//! Child-process spawn arguments (design D4 of `mpv-child-process`).
//!
//! `spawn_args` turns the option list that the embedded client used to set
//! through libmpv into `mpv` command-line arguments with the same values. The
//! child-process backend starts the child with this list.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, ErrorKind};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child as ProcessChild, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use crate::mpv_error::MpvError;
use crate::runtime::MpvRunConfig;

/// How many connect attempts one start makes before it fails, and how long it
/// waits between them (design D4). mpv creates its socket some time after it
/// starts, so the first tries can find no path yet. Production uses these; the
/// tests in `mpv_session/ipc/tests.rs` pass their own zero interval.
pub(crate) const CONNECT_ATTEMPTS: u32 = 250;
pub(crate) const CONNECT_INTERVAL: Duration = Duration::from_millis(20);

/// One child-start request: the program, its arguments, and the socket that
/// appears once it serves them.
pub(crate) struct StartRequest {
    /// The program to start. Production passes `"mpv"`, so a missing program
    /// names `mpv` in the start error (design D7).
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) socket_path: PathBuf,
    pub(crate) attempts: u32,
    pub(crate) interval: Duration,
}

/// The work one start request performs. Production starts the `mpv` program;
/// tests substitute a stub, so no mpv process runs (design D9).
pub(crate) type StartFn = Box<dyn Fn(StartRequest) -> Result<Started, MpvError> + Send + 'static>;

/// The one long-lived child spawner, owned by the player controller.
///
/// A child carries `PR_SET_PDEATHSIG`, which fires when the *thread* that
/// started it ends (design D5). Starting a child on the caller's thread would
/// tie it to a short-lived thread (for example the daemon start-up thread that
/// calls `pre_warm`). One long-lived thread starts every child instead, so a
/// child lives as long as the spawner and no longer.
///
/// `pre_warm` and the on-demand start in `run_player_thread` both start their
/// child through the same spawner (design D5).
pub(crate) struct Spawner {
    requests: mpsc::Sender<(StartRequest, mpsc::Sender<Result<Started, MpvError>>)>,
}

impl Spawner {
    /// The production spawner: the worker runs `Child::start` for `"mpv"`.
    pub(crate) fn new() -> Self {
        Self::with_start(Box::new(|request| {
            Child::start(
                &request.program,
                &request.args,
                &request.socket_path,
                request.attempts,
                request.interval,
            )
        }))
    }

    /// A spawner whose worker runs `start` instead of the `mpv` program (a
    /// test passes a stub; design D9).
    pub(crate) fn with_start(start: StartFn) -> Self {
        let (requests, receiver) =
            mpsc::channel::<(StartRequest, mpsc::Sender<Result<Started, MpvError>>)>();
        let _worker = thread::spawn(move || {
            while let Ok((request, reply)) = receiver.recv() {
                let _ = reply.send(start(request));
            }
        });
        Self { requests }
    }

    /// Starts one child on the spawner thread and returns it to the caller.
    ///
    /// # Errors
    ///
    /// Returns `MpvError::Spawn` only when the spawner thread is gone; a start
    /// fault comes back from `start` as its own `MpvError`.
    pub(crate) fn start(&self, request: StartRequest) -> Result<Started, MpvError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.requests
            .send((request, reply_tx))
            .map_err(|error| MpvError::Spawn(format!("mpv spawner is gone: {error}")))?;
        reply_rx
            .recv()
            .map_err(|error| MpvError::Spawn(format!("mpv spawner stopped: {error}")))?
    }
}

/// The path and script inputs that `init_mpv` resolves before it starts the
/// child. `spawn_args` stays a pure function of these values and the run
/// config: it reads no environment and touches no filesystem. `init_mpv`
/// keeps the filesystem work (`prepare_mpv_config_dir`,
/// `prepare_ipc_socket`) and hands the resolved values in.
pub(crate) struct SpawnPaths<'a> {
    /// The IPC socket path; also the value of `--input-ipc-server`, so user
    /// tools that talk to that socket keep working.
    pub(crate) ipc_path: &'a str,
    /// The mbv-owned config directory that `prepare_mpv_config_dir` filled.
    pub(crate) config_dir: &'a Path,
    /// The overlay script and its font directory, when mbv hands mpv its own
    /// overlay scripts (the `use_mpv_config` case with scripts enabled).
    pub(crate) overlay: Option<(&'a Path, &'a Path)>,
}

/// The command line for one `mpv` child process.
///
/// mpv's `mpv_create` (`player/client.c`) applies the `[libmpv]` profile from
/// `etc/builtin.conf` (checked at mpv 0.41) before `mpv_initialize`, and the
/// `mpv` program never applies that profile. The profile sets these defaults
/// for an embedded client, with the program default in parentheses:
///
/// - `config=no` (yes): mbv passes `--config=yes` with its own
///   `--config-dir`, so the child loads the same sanitized config that the
///   embedded client loaded. Nothing extra to restore.
/// - `idle=yes` (no): restored with `--idle=yes`. The child must stay alive
///   with no file and wait for IPC commands.
/// - `terminal=no` (yes): restored with `--no-terminal`.
/// - `input-terminal=no` (yes): restored, so the child never reads key
///   events from standard input.
/// - `osc=no` (yes): `init_mpv` passes `osc=no` whenever mbv manages the
///   scripts; the `use_mpv_config` case leaves scripts and the OSC to the
///   user's config on purpose, so `spawn_args` adds nothing there.
/// - `input-default-bindings=no` (yes) and `input-vo-keyboard=no` (yes):
///   `init_mpv` deliberately overrides both to `yes`, and `spawn_args` keeps
///   those values, so the child keeps the keyboard setup of today's
///   embedded session.
/// - `input-media-keys=no` and `media-controls=no`: macOS-only options. A
///   Linux `mpv` rejects them as unknown options, so `spawn_args` does not
///   pass them.
///
/// The remaining list below mirrors `init_mpv`'s `opt!` set in the same
/// order. The cache, `hwdec`, `vo`, audio-output, and `pause` settings stay
/// `set_property` calls after the start (design D4), so the order of effects
/// stays the same.
pub(crate) fn spawn_args(config: &MpvRunConfig, paths: &SpawnPaths<'_>) -> Vec<String> {
    let mut args = vec![
        "--config=yes".to_owned(),
        format!("--config-dir={}", paths.config_dir.to_str().unwrap_or("")),
        format!("--input-ipc-server={}", paths.ipc_path),
        "--input-default-bindings=yes".to_owned(),
        "--input-vo-keyboard=yes".to_owned(),
        "--wayland-app-id=mbv".to_owned(),
        "--gapless-audio=weak".to_owned(),
    ];
    if config.no_scripts || !config.use_mpv_config {
        args.push("--load-scripts=no".to_owned());
        args.push("--osc=no".to_owned());
        args.push("--osd-bar=no".to_owned());
    }
    if !config.no_scripts
        && !config.use_mpv_config
        && let Some((script, fonts)) = paths.overlay
    {
        args.push(format!("--scripts={}", script.to_str().unwrap_or("")));
        args.push(format!("--osd-fonts-dir={}", fonts.to_str().unwrap_or("")));
    }
    // Restores the embedded-client defaults that the `mpv` program lacks
    // (see the list above).
    args.push("--idle=yes".to_owned());
    args.push("--no-terminal".to_owned());
    args.push("--input-terminal=no".to_owned());
    args
}

/// Connects to the `mpv` IPC socket, retrying up to `attempts` times with
/// `interval` between tries. mpv creates the socket some time after it starts,
/// so the first tries can find no path yet (design D4). After every failed
/// attempt it checks whether the child already exited: a child that died at
/// start-up is the real fault, not the socket that never appeared (design D7).
fn connect_with_retry(
    process: &mut ProcessChild,
    stderr: &mut StderrLog,
    program: &str,
    socket_path: &Path,
    attempts: u32,
    interval: Duration,
) -> Result<UnixStream, MpvError> {
    let attempts = attempts.max(1);
    let mut attempt = 0;
    loop {
        attempt += 1;
        match connect_once(socket_path) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                check_child_alive(process, stderr, program)?;
                if attempt >= attempts {
                    return Err(error);
                }
            }
        }
        thread::sleep(interval);
    }
}

/// Returns `ChildExited` when the child is already gone, so a failed connect
/// reports the real cause instead of a missing socket (design D7).
fn check_child_alive(
    process: &mut ProcessChild,
    stderr: &mut StderrLog,
    program: &str,
) -> Result<(), MpvError> {
    if let Some(status) = process.try_wait().map_err(MpvError::Io)? {
        return Err(child_exited_error(program, status, stderr));
    }
    Ok(())
}

/// The message for a child that exited during start-up: the exit status and
/// the tail of what it wrote to stderr, so the real cause is visible.
fn child_exited_error(program: &str, status: ExitStatus, stderr: &mut StderrLog) -> MpvError {
    let mut message = format!("the {program} child process exited during start-up ({status})");
    let tail = stderr.finish();
    if !tail.is_empty() {
        message.push_str("; stderr: ");
        message.push_str(&tail.join(" | "));
    }
    MpvError::ChildExited(message)
}

/// One connect attempt. A path that does not exist yet is `SocketNotFound`,
/// so the retry loop above can tell it from a real socket fault.
fn connect_once(socket_path: &Path) -> Result<UnixStream, MpvError> {
    UnixStream::connect(socket_path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            MpvError::SocketNotFound(format!(
                "the mpv IPC socket {} was not found",
                socket_path.display()
            ))
        } else {
            MpvError::Io(error)
        }
    })
}

/// A started `mpv` child process and its connected IPC socket (design D4 and
/// D5). One playback run owns one `Child`.
pub(crate) struct Child {
    process: ProcessChild,
}

/// A started child and the connected IPC socket it serves.
pub(crate) type Started = (Child, UnixStream);

impl Child {
    /// Starts `program` with `args` and connects to the IPC socket it serves.
    /// Standard input and output go to the null device; stderr is drained to
    /// the log and kept as a tail, so a start failure names the real cause
    /// instead of the socket that never appeared (design D4 and D7).
    pub(crate) fn start(
        program: &str,
        args: &[String],
        socket_path: &Path,
        attempts: u32,
        interval: Duration,
    ) -> Result<Started, MpvError> {
        let (mut process, mut stderr) = start_process(program, args)?;
        match connect_with_retry(
            &mut process,
            &mut stderr,
            program,
            socket_path,
            attempts,
            interval,
        ) {
            Ok(stream) => Ok((Self { process }, stream)),
            // `std::process::Child` has no `Drop`, so a failed connect must
            // reap the process itself. Otherwise mpv stays alive (it runs with
            // `--idle=yes`) or becomes a zombie (design D5). A child that
            // `try_wait` already reaped is not touched again.
            Err(error) => {
                if !matches!(error, MpvError::ChildExited(_)) {
                    reap(&mut process);
                }
                Err(error)
            }
        }
    }
}

impl Drop for Child {
    /// Kills and reaps the process. The run loop already waited its own
    /// shutdown grace period after it sent `quit` (design D5).
    fn drop(&mut self) {
        reap(&mut self.process);
    }
}

/// Kills and reaps the child. Killing an exited child is harmless, and the
/// wait reaps it so no zombie stays.
fn reap(process: &mut ProcessChild) {
    let _ = process.kill();
    let _ = process.wait();
}

/// Starts one `mpv` child process with the null device for standard input and
/// output, a drained pipe for standard error, and a `SIGKILL` on the parent
/// thread's death.
fn start_process(program: &str, args: &[String]) -> Result<(ProcessChild, StderrLog), MpvError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // SAFETY: `pre_exec` runs in the forked child before `exec`. The closure
    // calls only `prctl`, which is async-signal-safe and touches no shared
    // state. `PR_SET_PDEATHSIG` makes the kernel send `SIGKILL` to this child
    // when the thread that started it ends, so an `mpv` child never outlives
    // the long-lived spawner thread that owns it (design D5).
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        })
    };
    let mut process = command
        .spawn()
        .map_err(|error| spawn_error(program, &error))?;
    let stderr = StderrLog::drain(process.stderr.take());
    Ok((process, stderr))
}

/// How many trailing stderr lines `child_exited_error` keeps.
const STDERR_TAIL_LINES: usize = 20;

/// The kept tail of a child's stderr and the reader thread that drains the
/// pipe, so a chatty child never blocks on a full stderr buffer.
struct StderrLog {
    tail: Arc<Mutex<VecDeque<String>>>,
    reader: Option<thread::JoinHandle<()>>,
}

impl StderrLog {
    /// Drains `stderr` on its own thread: each line is logged, and the last
    /// `STDERR_TAIL_LINES` lines are kept for a start-failure message.
    fn drain(stderr: Option<ChildStderr>) -> Self {
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let Some(stderr) = stderr else {
            return Self { tail, reader: None };
        };
        let sink = Arc::clone(&tail);
        let reader = thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let line = line.trim_end().to_owned();
                if line.is_empty() {
                    continue;
                }
                tracing::warn!(name: "player.mpv.stderr", target: "mpv", message = %line, "mpv wrote to stderr");
                let mut tail = sink.lock().unwrap();
                if tail.len() == STDERR_TAIL_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        });
        Self {
            tail,
            reader: Some(reader),
        }
    }

    /// Waits for the reader to reach end-of-file and returns the kept lines.
    /// The child has exited, so the pipe is closed and this cannot block.
    fn finish(&mut self) -> Vec<String> {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.tail.lock().unwrap().iter().cloned().collect()
    }
}

/// Maps a spawn fault. A program that is not on the search path names `mpv`
/// for the start error (the spec says the error names the missing program);
/// every other fault is a general spawn error.
fn spawn_error(program: &str, error: &std::io::Error) -> MpvError {
    if error.kind() == ErrorKind::NotFound {
        MpvError::ProgramNotFound(format!(
            "the {program} program was not found on the search path"
        ))
    } else {
        MpvError::Spawn(format!("cannot start the {program} program: {error}"))
    }
}

#[cfg(test)]
mod tests;
