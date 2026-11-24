//! Hidden local-daemon self-spawn subcommand (design.md decision 1).
//!
//! Ported from `relay::spawn_detached` / `run_relay_main`, minus everything
//! pty-related: no pty, no winsize, no byte-pipe multiplexing, no eviction.
//! The daemon here is just `crates/mbv-daemon/src/run.rs::run_local_with_options`
//! running headless in a detached process; clients reach it exactly the way
//! any other `DaemonEndpoint::Local` client does.
//!
//! Roles:
//! - The **launcher** (a Client with `stay_alive` set) starts a local daemon
//!   through `crate::app::owner_link::start_owner`, then attaches to it as an
//!   ordinary client (`App::new_remote`).
//! - The **local daemon** is `mbv --__local-daemon`, entered via
//!   [`run_local_daemon_main`], which never returns.
//!
//! Only the Owner's own half stays here (design D3 of
//! `split-tui-gui-clients`): the swap launch, the tray hooks, and the run
//! loop. The Client half — the argv, the display environment, and the
//! detached spawn that waits for the control socket — lives beside the Owner
//! link in `crate::app::owner_link`, because a Client starts the Owner.

use std::ffi::OsStr;
use std::process::{Command, Stdio};

use mbv_desktop::launch::{detach, launch_exe, terminal_prefix, toggle_panel};

/// Pure argv resolution for the swap-launch command.
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
            let mut argv = terminal_prefix(panel_terminal, terminal_env, "terminal")
                .map_err(|reason| format!("cannot start a terminal Client: {reason}"))?;
            argv.push(exe);
            Ok(argv)
        }
    }
}

/// Detached launch of the replacement Client: stdin,
/// stdout and stderr null, and `setsid` in `pre_exec`, the same as the
/// Client's Owner spawn (`crate::app::owner_link::start_owner`). The Owner's
/// environment already carries the session display variables.
fn detached_swap_command(argv: &[String]) -> Command {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    detach(&mut cmd, Stdio::null());
    cmd
}

/// The Local role's Player-ready hook: starts the Owner's MPRIS publisher for
/// local playback (owner-mpris-fallback design D5) and stores the handle the
/// Tray reads when it starts. The packaged `mbvd` passes a no-op hook, so it
/// starts no publisher, and test builds construct neither publisher.
fn owner_player_ready_hook(
    player_handle: std::sync::Arc<std::sync::Mutex<Option<mbv_daemon::DaemonPlayerHandle>>>,
) -> Box<dyn FnOnce(mbv_daemon::DaemonPlayerHandle)> {
    Box::new(move |handle| {
        #[cfg(not(any(test, feature = "test")))]
        mbv_desktop::mpris::start_owner(
            std::sync::Arc::clone(&handle.status),
            handle.transport_tx.clone(),
            mbv_config::image_disk_cache_path,
        );
        *player_handle.lock().unwrap() = Some(handle);
    })
}

/// Entered via the hidden `mbv --__local-daemon` self-spawn. Never returns.
pub fn run_local_daemon_main() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let log_spec = match mbv_core::applog::parse_log_level_arg(&args) {
        Ok(spec) => spec.unwrap_or_default(),
        Err(error) => {
            crate::startup::print_usage();
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    // The daemon IS the SIGHUP firewall: closing the launching terminal
    // must not kill it (belt-and-suspenders with the setsid() done at
    // spawn time in the Client's Owner spawn).
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

    let state_dir = mbv_config::state_dir();
    mbv_core::applog::init(false, Some(state_dir.join("local-daemon.log")), &log_spec);
    tracing::info!(name: "local_daemon.process.starting", target: "local_daemon", "local daemon starting");

    let config = match crate::app::config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mbv: local daemon: {e}");
            std::process::exit(1);
        }
    };

    // Acquire the single-instance lock and write our PID before binding the
    // control socket, so `mbv -q` can find us (the launcher already dropped
    // its own liveness-probe guard before spawning us).
    let lock_path = mbv_config::owner_lock_path();
    let socket_path = mbv_daemon::socket_path();
    let mut guard = match mbv_daemon::resolve(&socket_path, &lock_path) {
        Ok(mbv_daemon::Resolution::Fresh(guard)) => guard,
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

    // `Ok` is unreachable: the daemon loop never returns — shutdown always
    // ends in `process::exit`. Only a startup failure reaches this point.
    let Err(error) = mbv_daemon::run_local_with_options(
        mbv_daemon::DaemonStartupContext::new(config, mbv_daemon::DaemonRole::Local),
        false,
        mbv_daemon::DaemonRuntimeHooks {
            on_player_ready: owner_player_ready_hook(player_handle),
            on_tray_ready: Box::new(move |shutdown_tx| {
                let handle = player_handle_for_tray.lock().unwrap().clone()?;
                mbv_desktop::tray::spawn(
                    shutdown_tx,
                    handle.status,
                    handle.transport_tx,
                    handle.attached_clients,
                )
            }),
            swap_command: Box::new(move |direction| {
                let current = std::env::current_exe()
                    .map_err(|e| format!("cannot locate the mbv binary: {e}"))?;
                let exe = launch_exe(&current, std::process::id());
                // The `[panel] terminal` value is re-read from the config file
                // on every swap, like `owner_settings::reader`:
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
            notify: Box::new(mbv_desktop::launch::notify),
            // The Tray's icon click asks the Owner process to toggle the
            // pinned panel; the request runs on a worker thread, so the
            // Owner's event loop never waits for pinwin.
            toggle_panel: Box::new(toggle_panel),
        },
        // The Local daemon owns the Cast connector and hosts an Emby
        // Session output; see `LocalOutputHooks` for the entry-point rule.
        mbv_daemon::LocalOutputHooks {
            cast_connector: mbv_daemon::local_cast_connector(),
            emby_session_output: true,
        },
    );
    eprintln!("mbv: local daemon: {error}");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

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
