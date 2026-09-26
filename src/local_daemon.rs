//! Hidden local-daemon self-spawn subcommand (design.md decision 1).
//!
//! Ported from `relay::spawn_detached` / `run_relay_main`, minus everything
//! pty-related: no pty, no winsize, no byte-pipe multiplexing, no eviction.
//! The daemon here is just `crates/mbv-core/src/daemon.rs::run_with_options`
//! running headless in a detached process; clients reach it exactly the way
//! any other `DaemonEndpoint::Local` client does.
//!
//! Roles:
//! - The **launcher** (bare `mbv` with `stay_alive` set) calls
//!   [`spawn_detached`] to fork+detach a local daemon, then attaches to it
//!   as an ordinary client (`App::new_remote`).
//! - The **local daemon** is `mbv --__local-daemon`, entered via
//!   [`run_local_daemon_main`], which never returns.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mbv_core::daemon;

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(20);

fn to_io(e: nix::Error) -> io::Error {
    io::Error::from_raw_os_error(e as i32)
}

/// Fork+detach a local daemon (`mbv --__local-daemon`) and wait until its
/// control socket is ready. On failure -- including the daemon exiting
/// early because the cached token is missing/invalid -- returns the reason
/// so the still-live launching terminal can report it, rather than the
/// daemon failing silently in the background.
fn local_daemon_args(log_level: Option<mbv_core::applog::Level>) -> Vec<String> {
    let mut args = vec!["--__local-daemon".to_string()];
    if let Some(level) = log_level {
        args.extend(["--log-level".to_string(), level.logfmt().to_string()]);
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
            log::warn!(target: "local_daemon", "systemctl --user show-environment failed: {}", output.status);
            return None;
        }
        Err(e) => {
            log::warn!(target: "local_daemon", "cannot run systemctl --user show-environment: {e}");
            return None;
        }
    };
    let env = display_env_from(&String::from_utf8_lossy(&output.stdout));
    if env.is_none() {
        log::warn!(target: "local_daemon", "systemd user manager exports no display; inheriting launcher's");
    }
    env
}

pub fn spawn_detached(
    socket_path: &str,
    log_level: Option<mbv_core::applog::Level>,
) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate binary: {e}"))?;
    let mut cmd = Command::new(exe);
    cmd.args(local_daemon_args(log_level));
    if let Some(display_env) = session_display_env() {
        for name in DISPLAY_VARS {
            cmd.env_remove(name);
        }
        cmd.envs(display_env);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());
    // Detach into its own session (equivalent to `setsid <cmd>`), and
    // ignore SIGHUP so closing the launching terminal can't kill it --
    // belt-and-suspenders with the daemon's own SIGHUP-ignore below.
    // SAFETY: The child-side hook only creates a new session before exec.
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(to_io)?;
            Ok(())
        })
    };
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to start local daemon: {e}"))?;
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
            return Err(if captured.is_empty() {
                format!("local daemon exited before starting (status {status})")
            } else {
                captured.to_string()
            });
        }
        if Instant::now() >= deadline {
            return Err("local daemon did not become ready in time".to_string());
        }
        std::thread::sleep(READY_POLL_INTERVAL);
    }
}

/// Entered via the hidden `mbv --__local-daemon` self-spawn. Never returns.
pub fn run_local_daemon_main() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let log_level = match crate::parse_log_level_arg(&args) {
        Ok(level) => level.unwrap_or(mbv_core::applog::Level::Info),
        Err(error) => {
            crate::print_usage();
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    // The daemon IS the SIGHUP firewall: closing the launching terminal
    // must not kill it (belt-and-suspenders with the setsid() done at
    // spawn time in `spawn_detached`).
    // SAFETY: Installing SIGHUP ignore is valid for this daemon process and
    // the action uses no handler pointer or uninitialized state.
    unsafe {
        let sa = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigIgn,
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        let _ = nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGHUP, &sa);
    }

    let state_dir = crate::state_dir();
    mbv_core::applog::init(false, Some(state_dir.join("local-daemon.log")), log_level);
    log::info!(target: "local_daemon", "local daemon starting");

    let config = match crate::config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mbv: local daemon: {e}");
            std::process::exit(1);
        }
    };

    // Acquire the single-instance lock and write our PID before binding the
    // control socket, so `mbv -q` can find us (the launcher already dropped
    // its own liveness-probe guard before spawning us).
    let lock_path = crate::single_instance::lock_path();
    let socket_path = crate::single_instance::socket_path();
    let mut guard = match crate::single_instance::resolve(&socket_path, &lock_path) {
        Ok(crate::single_instance::Resolution::Fresh(guard)) => guard,
        Ok(_) => {
            eprintln!("mbv: local daemon: another instance took ownership before startup");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("mbv: local daemon: single-instance check failed: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = guard.write_pid() {
        log::warn!(target: "local_daemon", "failed to write pid into lock file: {e}");
    }
    if let Err(e) = mbv_core::config::load_or_create_control_credential() {
        eprintln!("mbv: local daemon: cannot load Control credential: {e}");
        std::process::exit(1);
    }

    // Player ownership does not depend on Remote Service availability. The
    // Local role retains its existing Control-credential contract.
    let show_systray_icon = config.show_systray_icon;
    let player_handle: std::sync::Arc<std::sync::Mutex<Option<daemon::DaemonPlayerHandle>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let player_handle_for_tray = std::sync::Arc::clone(&player_handle);

    daemon::run_with_options(
        daemon::DaemonStartupContext::new(config, daemon::DaemonRole::Local),
        false,
        daemon::DaemonRuntimeHooks {
            on_player_ready: Box::new(move |handle| {
                *player_handle.lock().unwrap() = Some(handle);
            }),
            on_tray_ready: Box::new(move |shutdown_tx| {
                if !show_systray_icon {
                    return None;
                }
                let handle = player_handle_for_tray.lock().unwrap().take()?;
                crate::tray::spawn(shutdown_tx, handle.status, handle.command_tx)
            }),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::wayland_and_x11(
        "HOME=/h\nWAYLAND_DISPLAY=wayland-1\nDISPLAY=:0\nXAUTHORITY=/run/xauth\n",
        Some(vec![("WAYLAND_DISPLAY", "wayland-1"), ("DISPLAY", ":0"), ("XAUTHORITY", "/run/xauth")])
    )]
    #[case::no_graphical_session("HOME=/h\nPATH=/bin\n", None)]
    fn display_env_is_read_from_the_user_manager(
        #[case] show_environment: &str,
        #[case] expected: Option<Vec<(&'static str, &str)>>,
    ) {
        let expected = expected.map(|vars| {
            vars.into_iter()
                .map(|(name, value)| (name, value.to_string()))
                .collect::<Vec<_>>()
        });
        assert_eq!(display_env_from(show_environment), expected);
    }

    #[test]
    fn spawn_args_forward_only_an_explicit_log_level() {
        assert_eq!(local_daemon_args(None), ["--__local-daemon"]);
        assert_eq!(
            local_daemon_args(Some(mbv_core::applog::Level::Debug)),
            ["--__local-daemon", "--log-level", "debug"]
        );
    }
}
