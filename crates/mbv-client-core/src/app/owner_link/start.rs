//! Starting this machine's Owner process (design D3 of
//! `split-tui-gui-clients`): the argv the Owner takes, the graphical session's
//! display environment, and the detached spawn that waits for the Owner's
//! control socket.
//!
//! Both Clients reach this through [`super::start_owner`]. The Owner itself
//! is `mbv --__local-daemon`, whose host half stays in the TUI Client's
//! `crate::local_daemon` until `3-add-owner-program` moves it to `mbv-owner`.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mbv_desktop::launch::detach;

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// The argv that makes a fresh `mbv` the Owner process, forwarding only an
/// explicit log level.
pub(in crate::app) fn local_daemon_args(
    log_level: Option<mbv_core::applog::LogSpec>,
) -> Vec<String> {
    let mut args = vec!["--__local-daemon".to_string()];
    if let Some(spec) = log_level {
        args.extend(["--log-level".to_string(), spec.to_string()]);
    }
    args
}

const DISPLAY_VARS: [&str; 3] = ["WAYLAND_DISPLAY", "DISPLAY", "XAUTHORITY"];

/// Display variables from `systemctl --user show-environment` output; `None`
/// unless it names a Wayland or X11 display.
fn display_env_from(show_environment: &str) -> Option<Vec<(&'static str, String)>> {
    let vars: Vec<_> = DISPLAY_VARS
        .into_iter()
        .filter_map(|name| {
            show_environment
                .lines()
                .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
                .map(|value| (name, value.to_string()))
        })
        .collect();
    vars.iter()
        .any(|(name, _)| *name != "XAUTHORITY")
        .then_some(vars)
}

/// The graphical session's display, as exported to the systemd user manager.
/// The launcher's own env can't be trusted: an SSH login has no display (or a
/// forwarded one), and every later client attaches to this daemon's mpv.
fn session_display_env() -> Option<Vec<(&'static str, String)>> {
    let output = match Command::new("systemctl")
        .args(["--user", "show-environment"])
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            tracing::warn!(
                name: "local_daemon.environment.query_failed",
                target: "local_daemon",
                status = %output.status,
                "systemctl --user show-environment failed"
            );
            return None;
        }
        Err(e) => {
            tracing::warn!(
                name: "local_daemon.environment.query_failed",
                target: "local_daemon",
                error = %e,
                "cannot run systemctl --user show-environment"
            );
            return None;
        }
    };
    let env = display_env_from(&String::from_utf8_lossy(&output.stdout));
    if env.is_none() {
        tracing::warn!(
            name: "local_daemon.environment.display_missing",
            target: "local_daemon",
            "systemd user manager exports no display; inheriting launcher's"
        );
    }
    env
}

/// Fork+detach a local daemon (`mbv --__local-daemon`) and wait until its
/// control socket is ready. `exe` is the `mbv` program that becomes the
/// Owner; the TUI Client passes its own executable and the GUI Client passes
/// the `mbv` program it resolved. On failure -- including the daemon exiting
/// early because the cached token is missing/invalid -- returns the reason
/// so the still-live launching Client can report it, rather than the daemon
/// failing silently in the background.
pub(in crate::app) fn spawn_detached(
    exe: &Path,
    socket_path: &str,
    log_level: Option<mbv_core::applog::LogSpec>,
) -> Result<(), mbv_remote_player::RemotePlayerError> {
    let mut cmd = Command::new(exe);
    cmd.args(local_daemon_args(log_level));
    if let Some(display_env) = session_display_env() {
        for name in DISPLAY_VARS {
            cmd.env_remove(name);
        }
        cmd.envs(display_env);
    }
    // Detach into its own session (equivalent to `setsid <cmd>`), and
    // ignore SIGHUP so closing the launching terminal can't kill it --
    // belt-and-suspenders with the daemon's own SIGHUP-ignore below.
    detach(&mut cmd, Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| io::Error::new(e.kind(), format!("failed to start local daemon: {e}")))?;
    let mut stderr = child.stderr.take().expect("piped stderr");

    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if UnixStream::connect(socket_path).is_ok() {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            let mut captured = String::new();
            let _ = stderr.read_to_string(&mut captured);
            let captured = captured.trim();
            return Err(io::Error::other(if captured.is_empty() {
                format!("local daemon exited before starting (status {status})")
            } else {
                captured.to_string()
            })
            .into());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("local daemon did not become ready in time").into());
        }
        std::thread::sleep(READY_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests;
