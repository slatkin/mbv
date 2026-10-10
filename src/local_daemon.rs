//! Hidden local-daemon self-spawn subcommand (design.md decision 1).
//!
//! Ported from `relay::spawn_detached` / `run_relay_main`, minus everything
//! pty-related: no pty, no winsize, no byte-pipe multiplexing, no eviction.
//! The daemon here is just `crates/mbv-daemon/src/run.rs::run_with_options`
//! running headless in a detached process; clients reach it exactly the way
//! any other `DaemonEndpoint::Local` client does.
//!
//! Roles:
//! - The **launcher** (bare `mbv` with `stay_alive` set) calls
//!   [`spawn_detached`] to fork+detach a local daemon, then attaches to it
//!   as an ordinary client (`App::new_remote`).
//! - The **local daemon** is `mbv --__local-daemon`, entered via
//!   [`run_local_daemon_main`], which never returns.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
fn local_daemon_args(log_level: Option<mbv_core::applog::LogSpec>) -> Vec<String> {
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

/// Pure argv resolution for the swap-launch command (tray-pin-swap D5).
///
/// Pin runs `exe --pin` (a new pinned panel Client, no terminal involved).
/// Unpin prefixes `exe` with the `[panel] terminal` argv prefix when set,
/// otherwise `TERMINAL -e`; with neither set the swap cannot run, and the
/// error names both sources so the desktop notification does too
/// (spec pin-swap "Neither set").
fn swap_argv(
    direction: mbv_daemon::SwapDirection,
    exe: &OsStr,
    panel_terminal: Option<&[String]>,
    terminal_env: Option<&str>,
) -> Result<Vec<String>, String> {
    let exe = exe.to_string_lossy().into_owned();
    match direction {
        mbv_daemon::SwapDirection::Pin => Ok(vec![exe, "--pin".to_string()]),
        mbv_daemon::SwapDirection::Unpin => {
            let prefix = panel_terminal.filter(|argv| !argv.is_empty());
            let terminal = terminal_env.filter(|terminal| !terminal.trim().is_empty());
            match (prefix, terminal) {
                (Some(prefix), _) => {
                    let mut argv = prefix.to_vec();
                    argv.push(exe);
                    Ok(argv)
                }
                (None, Some(terminal)) => Ok(vec![terminal.to_string(), "-e".to_string(), exe]),
                (None, None) => Err(
                    "cannot start a terminal Client to unpin: neither [panel] terminal \
                     (config) nor TERMINAL (environment) is set"
                        .to_string(),
                ),
            }
        }
    }
}

/// Detached launch of the replacement Client (tray-pin-swap D5): stdin,
/// stdout and stderr null, and `setsid` in `pre_exec`, the same as
/// [`spawn_detached`]. The Owner's environment already carries the session
/// display variables.
fn detached_swap_command(argv: &[String]) -> Command {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    detach(&mut cmd, Stdio::null());
    cmd
}

/// Null stdin and stdout, the given stderr, and a new session via `setsid`
/// in `pre_exec`.
fn detach(cmd: &mut Command, stderr: Stdio) {
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

pub fn spawn_detached(
    socket_path: &str,
    log_level: Option<mbv_core::applog::LogSpec>,
) -> Result<(), mbv_remote_player::RemotePlayerError> {
    let exe = std::env::current_exe()
        .map_err(|e| io::Error::new(e.kind(), format!("cannot locate binary: {e}")))?;
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

/// Entered via the hidden `mbv --__local-daemon` self-spawn. Never returns.
pub fn run_local_daemon_main() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let log_spec = match crate::parse_log_level_arg(&args) {
        Ok(spec) => spec.unwrap_or_default(),
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
    mbv_core::applog::init(false, Some(state_dir.join("local-daemon.log")), &log_spec);
    tracing::info!(name: "local_daemon.process.starting", target: "local_daemon", "local daemon starting");

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
        tracing::warn!(
            name: "local_daemon.lock.write_failed",
            target: "local_daemon",
            error = %e,
            "failed to write pid into lock file"
        );
    }
    if let Err(e) = mbv_config::load_or_create_control_credential() {
        eprintln!("mbv: local daemon: cannot load Control credential: {e}");
        std::process::exit(1);
    }

    // Player ownership does not depend on Remote Service availability. The
    // Local role retains its existing Control-credential contract. Whether
    // the Tray is enabled is decided per call by the daemon loop from the
    // live Owner settings' `tray_enabled` (derived by `Config::tray_enabled`,
    // the single source of that rule); the hook is
    // callable more than once and clones the handle each time.
    let player_handle: std::sync::Arc<std::sync::Mutex<Option<mbv_daemon::DaemonPlayerHandle>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let player_handle_for_tray = std::sync::Arc::clone(&player_handle);

    match mbv_daemon::run_with_options(
        mbv_daemon::DaemonStartupContext::new(config, mbv_daemon::DaemonRole::Local),
        false,
        mbv_daemon::DaemonRuntimeHooks {
            on_player_ready: Box::new(move |handle| {
                *player_handle.lock().unwrap() = Some(handle);
            }),
            on_tray_ready: Box::new(move |shutdown_tx| {
                let handle = player_handle_for_tray.lock().unwrap().clone()?;
                mbv_desktop::tray::spawn(
                    shutdown_tx,
                    handle.status,
                    handle.transport_tx,
                    handle.pinned_client_attached,
                )
            }),
            swap_command: Box::new(move |direction| {
                let exe = std::env::current_exe()
                    .map_err(|e| format!("cannot locate the mbv binary: {e}"))?;
                // The `[panel] terminal` value is re-read from the config file
                // on every swap (design D5), like `owner_settings::reader`:
                // a config edit reaches the next Unpin without a restart.
                let panel_terminal = std::fs::read_to_string(mbv_config::config_path())
                    .ok()
                    .and_then(|text| mbv_config::parse_config(&text).ok())
                    .and_then(|config| config.panel.terminal);
                let terminal_env = std::env::var("TERMINAL").ok();
                let command_argv = swap_argv(
                    direction,
                    exe.as_os_str(),
                    panel_terminal.as_deref(),
                    terminal_env.as_deref(),
                )?;
                Ok(detached_swap_command(&command_argv))
            }),
            notify: Box::new(crate::pin::notify),
        },
    ) {
        // `Ok` is unreachable: the daemon loop never returns — shutdown
        // always ends in `process::exit`. Only a startup failure takes
        // the `Err` path, reported here like every other startup error.
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("mbv: local daemon: {error}");
            std::process::exit(1);
        }
    }
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
            local_daemon_args(Some(
                mbv_core::applog::LogSpec::parse("debug,player=trace").expect("valid spec"),
            )),
            ["--__local-daemon", "--log-level", "debug,player=trace"]
        );
    }

    /// tray-pin-swap spec pin-swap "Config value set" / "Only TERMINAL set" /
    /// "Neither set", and the Pin direction: the replacement-Client argv is
    /// `[panel] terminal` + exe, else `TERMINAL -e` + exe, else an error
    /// naming both; Pin never consults either source.
    #[rstest]
    #[case::pin_runs_exe_with_pin_flag(
        mbv_daemon::SwapDirection::Pin,
        Some(vec!["wezterm".to_string(), "start".to_string(), "--".to_string()]),
        Some("ghostty"),
        vec!["/usr/bin/mbv".to_string(), "--pin".to_string()]
    )]
    #[case::unpin_uses_panel_terminal(
        mbv_daemon::SwapDirection::Unpin,
        Some(vec!["wezterm".to_string(), "start".to_string(), "--".to_string()]),
        Some("ghostty"),
        vec![
            "wezterm".to_string(),
            "start".to_string(),
            "--".to_string(),
            "/usr/bin/mbv".to_string(),
        ]
    )]
    #[case::unpin_falls_back_to_terminal_env(
        mbv_daemon::SwapDirection::Unpin,
        None,
        Some("ghostty"),
        vec!["ghostty".to_string(), "-e".to_string(), "/usr/bin/mbv".to_string()]
    )]
    fn swap_argv_resolves_the_replacement_client_command(
        #[case] direction: mbv_daemon::SwapDirection,
        #[case] panel_terminal: Option<Vec<String>>,
        #[case] terminal_env: Option<&str>,
        #[case] expected: Vec<String>,
    ) {
        let resolved = swap_argv(
            direction,
            OsStr::new("/usr/bin/mbv"),
            panel_terminal.as_deref(),
            terminal_env,
        );
        assert_eq!(resolved, Ok(expected));
    }

    #[rstest]
    #[case::neither_set_names_both(None, None, "[panel] terminal")]
    #[case::empty_terminal_env_counts_as_unset(None, Some(""), "TERMINAL")]
    fn swap_argv_without_a_terminal_names_the_missing_source(
        #[case] panel_terminal: Option<Vec<String>>,
        #[case] terminal_env: Option<&str>,
        #[case] named: &str,
    ) {
        let error = swap_argv(
            mbv_daemon::SwapDirection::Unpin,
            OsStr::new("/usr/bin/mbv"),
            panel_terminal.as_deref(),
            terminal_env,
        )
        .expect_err("Unpin without a terminal must fail");
        assert!(error.contains(named), "error must name the source");
    }
}
