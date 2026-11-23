//! Process launches: the executable a launcher starts, the detached spawn, the
//! terminal argv a Client is hosted in, the panel's `pinwin` requests, and the
//! desktop notification.
//!
//! Both the local-daemon spawn and the `--pin` launcher start a fresh mbv from
//! the build the running process came from (spec pin-swap "Launches start the
//! Owner's own build after its binary file changes"). The pinned panel is a
//! process-wide singleton: `pinwin` allows one panel at a time and a second
//! `--pin` shows the running panel. Every panel request leaves this process as
//! one `pinwin` child, and the panel socket is passed in by the caller: the
//! Owner host has none (its starting Client removed `PINWIN_SOCKET`), the TUI
//! and the core pass the claimed one.

use std::fmt;
use std::io::IsTerminal;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The panel name every pinned mbv runs under.
pub const PANEL_NAME: &str = "mbv";

/// The variable pinwin sets on the Client it runs inside a panel: the daemon
/// socket that serves that panel.
const PINWIN_SOCKET_ENV: &str = "PINWIN_SOCKET";

/// The binary path a launch starts from.
///
/// After the started file is unlinked, Linux `current_exe()` ends in
/// ` (deleted)` and spawn fails with ENOENT. Then start from the launcher's
/// `/proc/{launcher_pid}/exe`, which names the same build while the launcher
/// runs.
#[must_use]
pub fn launch_exe(current: &Path, launcher_pid: u32) -> PathBuf {
    if current.to_string_lossy().ends_with(" (deleted)") {
        PathBuf::from(format!("/proc/{launcher_pid}/exe"))
    } else {
        current.to_path_buf()
    }
}

fn to_io(e: nix::Error) -> std::io::Error {
    std::io::Error::from_raw_os_error(e as i32)
}

/// Null stdin and stdout, the given stderr, and a new session via `setsid`
/// in `pre_exec`.
pub fn detach(cmd: &mut Command, stderr: Stdio) {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(stderr);
    // SAFETY: The child-side hook only creates a new session before exec.
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(to_io)?;
            Ok(())
        })
    };
}

/// The terminal argv prefix that hosts a Client: the configured
/// prefix when set, else `TERMINAL -e`. With neither set the launch fails with
/// a reason that names both sources; `config_key` is the configuration key the
/// caller reads the prefix from.
pub fn terminal_prefix(
    configured: Option<&[String]>,
    terminal_env: Option<&str>,
    config_key: &str,
) -> Result<Vec<String>, String> {
    let prefix = configured.filter(|argv| !argv.is_empty());
    let terminal = terminal_env.filter(|terminal| !terminal.trim().is_empty());
    match (prefix, terminal) {
        (Some(prefix), _) => Ok(prefix.to_vec()),
        (None, Some(terminal)) => Ok(vec![terminal.to_string(), "-e".to_string()]),
        (None, None) => Err(format!(
            "neither [panel] {config_key} (config) nor TERMINAL (environment) is set"
        )),
    }
}

/// Why the `pinwin` program could not be spawned. A missing binary asks for an
/// install; any other failure names its cause.
#[derive(Debug)]
pub struct PinSpawnError(std::io::Error);

impl PinSpawnError {
    fn new(error: std::io::Error) -> Self {
        Self(error)
    }
}

impl From<std::io::Error> for PinSpawnError {
    fn from(error: std::io::Error) -> Self {
        Self(error)
    }
}

impl fmt::Display for PinSpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.kind() == std::io::ErrorKind::NotFound {
            f.write_str("pinwin is not installed")
        } else {
            write!(f, "pinwin could not start: {}", self.0)
        }
    }
}

impl std::error::Error for PinSpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Why one `pinwin` request did not finish.
#[derive(Debug)]
pub enum PinRequestError {
    /// The `pinwin` program could not be spawned.
    Spawn(PinSpawnError),
    /// `pinwin` ran and refused the request.
    Refused(String),
}

impl fmt::Display for PinRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => error.fmt(f),
            Self::Refused(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for PinRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) => Some(error),
            Self::Refused(_) => None,
        }
    }
}

/// `toggle <panel>`.
#[must_use]
pub fn toggle_argv(panel: &str) -> Vec<String> {
    vec!["toggle".to_string(), panel.to_string()]
}

/// The result a finished `pinwin` request reports: exit 0 is
/// success, exit 1 is pinwin's own reason from stderr, exit 2 is a usage
/// mismatch between mbv and pinwin, and any other ending is a failure that
/// names it.
pub fn request_result(code: Option<i32>, stderr: &str) -> Result<(), String> {
    match code {
        Some(0) => Ok(()),
        Some(1) => {
            let reason = stderr.trim();
            if reason.is_empty() {
                Err("pinwin refused the request".to_string())
            } else {
                Err(reason.to_string())
            }
        }
        Some(2) => Err(
            "pinwin rejected the command line: mbv and pinwin disagree on the pinwin CLI"
                .to_string(),
        ),
        Some(code) => Err(format!("pinwin exited with status {code}")),
        None => Err("pinwin ended by a signal".to_string()),
    }
}

/// Run one `pinwin` request against the panel daemon at `socket`, or against
/// pinwin's own default when `socket` is `None`. The child is the untested
/// boundary; the argv it runs and the result it maps to are pure.
pub fn run_request(argv: &[String], socket: Option<&str>) -> Result<(), PinRequestError> {
    let mut command = Command::new("pinwin");
    command.args(argv);
    if let Some(socket) = socket {
        command.env(PINWIN_SOCKET_ENV, socket);
    }
    let output = command
        .output()
        .map_err(|error| PinRequestError::Spawn(PinSpawnError::new(error)))?;
    request_result(
        output.status.code(),
        &String::from_utf8_lossy(&output.stderr),
    )
    .map_err(PinRequestError::Refused)
}

/// Ask pinwin to toggle the pinned panel: hide it if shown, show it if
/// hidden. This is the request the `--toggle` flag sends and the one the
/// Owner process sends on the Tray's behalf (tray-panel-toggle design D4).
///
/// The request runs on a worker thread, so the caller's thread never waits
/// for pinwin, and a failure is reported as a desktop notification: the
/// caller is a background process with no terminal to print on.
pub fn toggle_panel() {
    let argv = toggle_argv(PANEL_NAME);
    let worker = std::thread::Builder::new()
        .name("pinwin-toggle".to_string())
        .spawn(move || {
            if let Err(error) = run_request(&argv, None) {
                let reason = error.to_string();
                tracing::warn!(
                    name: "panel_toggle.failed",
                    target: "panel_toggle",
                    { error = %reason },
                    "pinned panel toggle failed"
                );
                notify(&format!("cannot toggle the pinned panel: {reason}"));
            }
        });
    if let Err(error) = worker {
        tracing::warn!(
            name: "panel_toggle.worker_failed",
            target: "panel_toggle",
            { error = %error },
            "pinned panel toggle worker could not start"
        );
    }
}

/// Best-effort `notify-send`, matching the TUI's system notifications; a
/// missing binary is only logged. Also the swap-failure notification path for
/// the daemon's `notify` hook.
pub fn notify(message: &str) {
    let result = Command::new("notify-send")
        .arg("--app-name=mbv")
        .arg("mbv")
        .arg(message)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match result {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(
            name: "startup.pin.notify.failed",
            target: "startup",
            code = status.code().unwrap_or(-1),
            "notify-send failed"
        ),
        Err(error) => tracing::warn!(
            name: "startup.pin.notify.failed",
            target: "startup",
            { error = %error },
            "notify-send could not run"
        ),
    }
}

/// Whether this process has an interactive stdin. A panel host reads it to
/// tell a terminal launch from a desktop launch before it reports a failure.
#[must_use]
pub fn stdin_is_terminal() -> bool {
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests;
