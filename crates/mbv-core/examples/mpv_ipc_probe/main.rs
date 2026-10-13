//! Manual mpv event probe (`openspec/changes/mpv-ipc-event-probe`).
//!
//! Runs each scenario through in-process libmpv and through an `mpv` child
//! process over JSON IPC, then compares the two event sequences after the
//! design D3 normalizations. It starts real mpv instances with synthetic
//! `av://lavfi:` media, so run it only by hand. It never runs in CI.

mod backends;
mod compare;
mod scenarios;

use backends::{Backend, Ipc, Libmpv, Next};
use compare::{Outcome, RULES};
use scenarios::{MISSING, SCENARIOS, Scenario, Step};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};
use std::{env, fmt, fs};

const RUNS: usize = 5;
const WAIT_BOUND: Duration = Duration::from_secs(10);
const SETTLE: Duration = Duration::from_millis(300);

#[derive(Debug)]
struct ProbeError(String);

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn run_scenario(backend: &mut dyn Backend, scenario: &Scenario, missing: &str) -> Vec<Value> {
    let mut trace = Vec::new();
    // Take the initial observed values before the first step, so their
    // position does not depend on when each client first polls.
    drain(backend, &mut trace);
    for step in scenario.steps {
        match step {
            Step::Cmd(args) => {
                let args: Vec<&str> = args
                    .iter()
                    .map(|arg| if *arg == MISSING { missing } else { *arg })
                    .collect();
                backend.command(&args);
            }
            Step::Set(name, value) => backend.set(name, *value),
            Step::Wait(event, count) => wait_for(backend, &mut trace, event, *count),
            Step::Quit => {
                backend.command(&["quit"]);
                wait_for(backend, &mut trace, "shutdown", 1);
            }
        }
    }
    drain(backend, &mut trace);
    trace
}

fn drain(backend: &mut dyn Backend, trace: &mut Vec<Value>) {
    while let Next::Event(event) = backend.next(SETTLE) {
        trace.push(event);
    }
}

fn wait_for(backend: &mut dyn Backend, trace: &mut Vec<Value>, event: &str, count: usize) {
    let deadline = Instant::now() + WAIT_BOUND;
    let mut seen = 0;
    while seen < count {
        match backend.next(deadline.saturating_duration_since(Instant::now())) {
            Next::Event(value) => {
                seen += usize::from(value["event"] == event);
                trace.push(value);
            }
            Next::Timeout => {
                eprintln!("  timed out waiting for {event} ({seen}/{count})");
                return;
            }
            Next::Closed => return,
        }
    }
}

struct Options {
    out: Option<PathBuf>,
}

fn parse_args() -> Result<Option<Options>, ProbeError> {
    let mut out = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--out" => {
                let dir = args
                    .next()
                    .ok_or_else(|| ProbeError("--out needs a directory".to_owned()))?;
                out = Some(PathBuf::from(dir));
            }
            other => return Err(ProbeError(format!("unknown argument: {other}"))),
        }
    }
    Ok(Some(Options { out }))
}

fn print_help() {
    println!("mpv_ipc_probe: compare libmpv and JSON IPC mpv event sequences");
    println!();
    println!("Usage: mpv_ipc_probe [--out <dir>]");
    println!("  --out <dir>  write the first IPC trace of each scenario to <dir>/<id>.jsonl");
    println!("  --help       show this text");
    println!();
    println!("Each scenario runs {RUNS} times per backend. Scenarios:");
    for scenario in &SCENARIOS {
        println!("  {:<4} {}", scenario.id, scenario.summary);
    }
    println!();
    println!("Normalization rules:");
    for rule in RULES {
        println!("  {rule}");
    }
}

fn mpv_binary_version() -> String {
    Command::new("mpv")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

fn libmpv_version() -> Result<String, ProbeError> {
    let libmpv = Libmpv::start(&[])?;
    libmpv
        .mpv
        .get_property::<String>("mpv-version")
        .map_err(|e| ProbeError(format!("cannot read libmpv version: {e:?}")))
}

struct Probe {
    scratch: PathBuf,
    missing: String,
    header: Value,
    out: Option<PathBuf>,
}

impl Probe {
    fn run(&self, scenario: &Scenario) -> Result<(), ProbeError> {
        let mut libmpv_runs = Vec::new();
        let mut ipc_runs = Vec::new();
        let mut fixture = None;
        for run in 0..RUNS {
            let mut libmpv = Libmpv::start(scenario.options)?;
            libmpv_runs.push(run_scenario(&mut libmpv, scenario, &self.missing));
            let socket = self.scratch.join(format!("{}-{run}.sock", scenario.id));
            let mut ipc = Ipc::start(scenario.options, socket)?;
            ipc_runs.push(run_scenario(&mut ipc, scenario, &self.missing));
            fixture.get_or_insert_with(|| ipc.raw.clone());
        }
        Outcome::new(&libmpv_runs, &ipc_runs).print(scenario);
        match (&self.out, fixture) {
            (Some(dir), Some(lines)) => self.write_fixture(dir, scenario, &lines),
            _ => Ok(()),
        }
    }

    fn write_fixture(
        &self,
        dir: &Path,
        scenario: &Scenario,
        lines: &[String],
    ) -> Result<(), ProbeError> {
        let mut header = self.header.clone();
        header["mbv_probe"]["scenario"] = json!(scenario.id);
        let mut text = format!("{header}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        let path = dir.join(format!("{}.jsonl", scenario.id));
        fs::write(&path, text)
            .map_err(|e| ProbeError(format!("cannot write {}: {e}", path.display())))
    }
}

fn run(options: Options) -> Result<(), ProbeError> {
    let scratch = env::temp_dir().join(format!("mbv-mpv-ipc-probe-{}", std::process::id()));
    fs::create_dir_all(&scratch)
        .map_err(|e| ProbeError(format!("cannot create {}: {e}", scratch.display())))?;
    if let Some(dir) = &options.out {
        fs::create_dir_all(dir)
            .map_err(|e| ProbeError(format!("cannot create {}: {e}", dir.display())))?;
    }
    let mpv_version = mpv_binary_version();
    let libmpv_version = libmpv_version()?;
    println!("mpv binary: {mpv_version}");
    println!("libmpv:     {libmpv_version}");
    let probe = Probe {
        missing: scratch
            .join("missing")
            .join("none.flac")
            .display()
            .to_string(),
        header: json!({"mbv_probe": {
            "mpv_version": mpv_version,
            "date": time::OffsetDateTime::now_utc().date().to_string(),
        }}),
        scratch,
        out: options.out,
    };
    for scenario in &SCENARIOS {
        probe.run(scenario)?;
    }
    let _ = fs::remove_dir_all(&probe.scratch);
    Ok(())
}

fn main() -> ExitCode {
    let result = parse_args().and_then(|options| {
        let Some(options) = options else {
            print_help();
            return Ok(());
        };
        run(options)
    });
    if let Err(e) = result {
        eprintln!("mpv_ipc_probe: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
