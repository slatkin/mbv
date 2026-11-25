//! The per-user Owner process (design D8 of `3-add-owner-program`).
//!
//! Ported from `relay::spawn_detached` / `run_relay_main`, minus everything
//! pty-related: no pty, no winsize, no byte-pipe multiplexing, no eviction.
//! The Owner here is just `crates/mbv-daemon/src/run.rs::run_local_with_options`
//! running headless in a detached process; clients reach it exactly the way
//! any other `DaemonEndpoint::Local` client does.
//!
//! Roles:
//! - The **launcher** (a Client with `stay_alive` set) starts an Owner
//!   through `mbv_client_core::owner_link::start_owner`, then attaches to it as an
//!   ordinary client (`App::new_remote`).
//! - The **Owner process** is the `mbv-owner` program, entered via
//!   [`run_local_daemon_main`], which never returns.
//!
//! Only the Owner's own half lives here: the swap launch, the tray hooks, and
//! the run loop. The Client half — the argv, the display environment, and the
//! detached spawn that waits for the control socket — lives beside the Owner
//! link in `mbv_client_core::owner_link`, because a Client starts the Owner.

use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use mbv_desktop::launch::{detach, terminal_prefix, toggle_panel};

/// The TUI Client program: Pin swap and Unpin start it.
const TUI_PROGRAM: &str = "mbv-tui";

/// The GUI Client program: Open starts it.
const GUI_PROGRAM: &str = "mbv";

/// The sibling Client program one launch starts: the GUI Client for Open,
/// the TUI Client for Pin and Unpin (the pinned panel is a TUI Client).
fn launch_program_name(direction: mbv_daemon::SwapDirection) -> &'static str {
    match direction {
        mbv_daemon::SwapDirection::Open => GUI_PROGRAM,
        mbv_daemon::SwapDirection::Pin | mbv_daemon::SwapDirection::Unpin => TUI_PROGRAM,
    }
}

/// The reason a launch without its held program reports: no Client can run
/// the Owner's own build then (spec pin-swap "No Client program beside the
/// Owner program").
fn missing_program(name: &str) -> String {
    format!("cannot start a Client: no {name} program sits beside mbv-owner")
}

/// A sibling Client program the Owner holds from start (design D9).
///
/// A Client attaches only to an Owner of the same build, so a launch must
/// start the build the Owner itself runs, not whatever the installed path
/// names after a package upgrade. The open file is held for the Owner's
/// lifetime: the descriptor names the inode that was there at start.
struct HeldProgram {
    /// The open file, `O_CLOEXEC` as `File::open` gives it. It must outlive
    /// the Owner, because [`HeldProgram::launch`] names its descriptor.
    file: std::fs::File,
    /// The path the file was opened from: the program beside `mbv-owner`.
    path: PathBuf,
    /// The `(dev, ino)` the Owner opened.
    inode: (u64, u64),
}

/// The two sibling Client programs the Owner holds from start (design D9).
struct HeldPrograms {
    /// `mbv-tui`: Pin swap and Unpin start it.
    tui: Option<HeldProgram>,
    /// `mbv`, the GUI Client: Open starts it.
    gui: Option<HeldProgram>,
}

/// The held program as one launch sees it (design D9): the inode the Owner
/// opened, the path it opened, and the descriptor that still names it.
#[derive(Debug)]
struct LaunchProgram {
    /// The `(dev, ino)` the Owner opened at start.
    inode: (u64, u64),
    /// The path the Owner opened: the program beside `mbv-owner`.
    path: PathBuf,
    /// `/proc/<owner pid>/fd/<n>` for the held file. It names the held inode
    /// from any process of the same user, also from the terminal emulator an
    /// Unpin or Open launch starts.
    descriptor: PathBuf,
}

impl HeldProgram {
    /// Opens the program `name` beside the Owner's own program. `None` means
    /// no such program sat there at start, and no launch can start the
    /// Owner's own build of it.
    fn open_beside_owner(name: &str) -> Option<Self> {
        let owner = std::env::current_exe().ok()?;
        let path = owner.parent()?.join(name);
        let file = std::fs::File::open(&path).ok()?;
        let metadata = file.metadata().ok()?;
        Some(Self {
            file,
            path,
            inode: (metadata.dev(), metadata.ino()),
        })
    }

    /// The `(dev, ino)` the held path names now: a replaced file answers
    /// another inode, and a removed one answers nothing.
    fn current_inode(&self) -> Option<(u64, u64)> {
        let metadata = std::fs::metadata(&self.path).ok()?;
        Some((metadata.dev(), metadata.ino()))
    }

    /// The held program as a launch sees it.
    fn launch(&self) -> LaunchProgram {
        LaunchProgram {
            inode: self.inode,
            path: self.path.clone(),
            descriptor: PathBuf::from(format!(
                "/proc/{}/fd/{}",
                std::process::id(),
                self.file.as_raw_fd()
            )),
        }
    }
}

/// The path one launch starts (design D9): the held path while it still
/// names the held inode, else the Owner's descriptor for that inode. A path
/// that names another build must never start a Client, because the attach
/// gate refuses it.
fn launch_path(held: &LaunchProgram, current_inode: Option<(u64, u64)>) -> &Path {
    match current_inode {
        Some(current) if current == held.inode => &held.path,
        _ => &held.descriptor,
    }
}

/// Pure argv resolution for the swap-launch command.
///
/// Pin runs `mbv-tui --pin` (a new pinned panel Client, no terminal
/// involved). Unpin prefixes `mbv-tui` with the `[panel] terminal` argv
/// prefix when set, otherwise `TERMINAL -e`; with neither set the launch
/// cannot run, and the error names both sources so the desktop notification
/// does too (spec pin-swap "Neither set"). Open runs the GUI Client `mbv`, a
/// window that needs no terminal.
///
/// `held` is the program the Owner holds for `direction`, and
/// `current_inode` is the inode its path names now. No held program fails
/// before any other reason, because the launch cannot then start the Owner's
/// own build.
fn swap_argv(
    direction: mbv_daemon::SwapDirection,
    held: Option<&LaunchProgram>,
    current_inode: Option<(u64, u64)>,
    panel_terminal: Option<&[String]>,
    terminal_env: Option<&str>,
) -> Result<Vec<String>, String> {
    let exe = match held {
        Some(held) => launch_path(held, current_inode)
            .to_string_lossy()
            .into_owned(),
        None => return Err(missing_program(launch_program_name(direction))),
    };
    match direction {
        mbv_daemon::SwapDirection::Pin => Ok(vec![exe, "--pin".to_string()]),
        mbv_daemon::SwapDirection::Open => Ok(vec![exe]),
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
/// Client's Owner spawn (`mbv_client_core::owner_link::start_owner`). The Owner's
/// environment already carries the session display variables.
fn detached_swap_command(argv: &[String]) -> Command {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    detach(&mut cmd, Stdio::null());
    cmd
}

/// The Owner's Pin swap, Unpin, and Open launch (design D9): the replacement
/// command for `direction`, from the held program and the terminal sources.
/// The held path may name another build now (a package upgrade, or a removed
/// file), so its inode is read per launch.
fn launch_swap_command(
    direction: mbv_daemon::SwapDirection,
    held: &HeldPrograms,
) -> Result<Command, String> {
    let held = match direction {
        mbv_daemon::SwapDirection::Open => held.gui.as_ref(),
        mbv_daemon::SwapDirection::Pin | mbv_daemon::SwapDirection::Unpin => held.tui.as_ref(),
    };
    let current_inode = held.and_then(HeldProgram::current_inode);
    let program = held.map(HeldProgram::launch);
    // The `[panel] terminal` value is re-read from the config file on every
    // swap, like `owner_settings::reader`: a config edit reaches the next
    // Unpin without a restart.
    let panel_terminal = std::fs::read_to_string(mbv_config::config_path())
        .ok()
        .and_then(|text| mbv_config::parse_config(&text).ok())
        .and_then(|config| config.panel.terminal);
    let terminal_env = std::env::var("TERMINAL").ok();
    let argv = swap_argv(
        direction,
        program.as_ref(),
        current_inode,
        panel_terminal.as_deref(),
        terminal_env.as_deref(),
    )?;
    Ok(detached_swap_command(&argv))
}

/// The Local role's Player-ready hook: starts the Owner's MPRIS publisher for
/// local playback (owner-mpris-fallback design D5) and stores the handle the
/// Tray reads when it starts. The packaged `mbvd` passes a no-op hook, so it
/// starts no publisher, and test builds construct neither publisher.
fn owner_player_ready_hook(
    player_handle: std::sync::Arc<std::sync::Mutex<Option<mbv_daemon::DaemonPlayerHandle>>>,
) -> Box<dyn FnOnce(mbv_daemon::DaemonPlayerHandle)> {
    Box::new(move |handle| {
        #[cfg(not(test))]
        mbv_desktop::mpris::start_owner(
            std::sync::Arc::clone(&handle.status),
            handle.transport_tx.clone(),
            mbv_config::image_disk_cache_path,
        );
        *player_handle.lock().unwrap() = Some(handle);
    })
}

/// The `mbv-owner` entry: the Owner process run loop. Never returns.
pub fn run_local_daemon_main() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        crate::print_usage();
        std::process::exit(0);
    }
    let log_spec = match mbv_core::applog::parse_log_level_arg(&args) {
        Ok(spec) => spec.unwrap_or_default(),
        Err(error) => {
            crate::print_usage();
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

    let config = match mbv_config::load_config() {
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

    // Hold the sibling `mbv-tui` and `mbv` from start (design D9): a Pin
    // swap, Unpin, or Open after the installed file changes still starts the
    // build this Owner runs. The files stay open for the Owner's lifetime; the
    // launches read only their paths and inodes.
    let held_programs = HeldPrograms {
        tui: HeldProgram::open_beside_owner(TUI_PROGRAM),
        gui: HeldProgram::open_beside_owner(GUI_PROGRAM),
    };

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
            swap_command: Box::new(move |direction| launch_swap_command(direction, &held_programs)),
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
    use mbv_daemon::SwapDirection::{Open, Unpin};
    use rstest::rstest;

    /// The `(dev, ino)` the Owner opened at start, and the one the path
    /// answers after a package replaced the file.
    const HELD: (u64, u64) = (1, 2);
    const REPLACED: (u64, u64) = (3, 4);

    /// The held program for `direction`: a fixed path and its Owner
    /// descriptor, so a case varies only the inodes.
    fn held_program(direction: mbv_daemon::SwapDirection) -> LaunchProgram {
        LaunchProgram {
            inode: HELD,
            path: PathBuf::from(format!("/usr/bin/{}", launch_program_name(direction))),
            descriptor: PathBuf::from("/proc/4242/fd/7"),
        }
    }

    /// Resolves one launch the way the swap hook in
    /// [`run_local_daemon_main`] does. `held_inode` is `None` when the
    /// direction's program did not sit beside `mbv-owner` at start.
    fn resolve(
        direction: mbv_daemon::SwapDirection,
        held_inode: Option<(u64, u64)>,
        current_inode: Option<(u64, u64)>,
        panel_terminal: Option<&[&str]>,
        terminal_env: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let panel_terminal: Option<Vec<String>> =
            panel_terminal.map(|argv| argv.iter().map(|arg| (*arg).to_string()).collect());
        let program = held_inode.map(|inode| LaunchProgram {
            inode,
            ..held_program(direction)
        });
        swap_argv(
            direction,
            program.as_ref(),
            current_inode,
            panel_terminal.as_deref(),
            terminal_env,
        )
    }

    /// tray-pin-swap spec pin-swap "Config value set" / "Only TERMINAL set" /
    /// "Neither set", and the Pin direction; and 3-add-owner-program spec
    /// pin-swap "Launches start the Owner's own build after its binary file
    /// changes": a launch starts the held path while it still names the held
    /// inode, else the Owner's descriptor for that inode.
    #[rstest]
    #[case::pin_runs_the_held_program_with_the_pin_flag(
        mbv_daemon::SwapDirection::Pin,
        Some(HELD),
        Some(HELD),
        Some(&["wezterm", "start", "--"][..]),
        Some("ghostty"),
        vec!["/usr/bin/mbv-tui".to_string(), "--pin".to_string()]
    )]
    #[case::unpin_uses_panel_terminal(
        mbv_daemon::SwapDirection::Unpin,
        Some(HELD),
        Some(HELD),
        Some(&["wezterm", "start", "--"][..]),
        Some("ghostty"),
        vec![
            "wezterm".to_string(),
            "start".to_string(),
            "--".to_string(),
            "/usr/bin/mbv-tui".to_string(),
        ]
    )]
    #[case::unpin_falls_back_to_terminal_env(
        mbv_daemon::SwapDirection::Unpin,
        Some(HELD),
        Some(HELD),
        None,
        Some("ghostty"),
        vec!["ghostty".to_string(), "-e".to_string(), "/usr/bin/mbv-tui".to_string()]
    )]
    #[case::pin_after_the_tui_path_changed_starts_the_owner_descriptor(
        mbv_daemon::SwapDirection::Pin,
        Some(HELD),
        Some(REPLACED),
        None,
        None,
        vec!["/proc/4242/fd/7".to_string(), "--pin".to_string()]
    )]
    #[case::unpin_after_the_tui_path_was_removed_starts_the_owner_descriptor(
        mbv_daemon::SwapDirection::Unpin,
        Some(HELD),
        None,
        Some(&["wezterm", "start", "--"][..]),
        Some("ghostty"),
        vec![
            "wezterm".to_string(),
            "start".to_string(),
            "--".to_string(),
            "/proc/4242/fd/7".to_string(),
        ]
    )]
    #[case::open_starts_the_gui_with_no_terminal(
        mbv_daemon::SwapDirection::Open,
        Some(HELD),
        Some(HELD),
        Some(&["wezterm", "start", "--"][..]),
        Some("ghostty"),
        vec!["/usr/bin/mbv".to_string()]
    )]
    fn swap_argv_resolves_the_replacement_client_command(
        #[case] direction: mbv_daemon::SwapDirection,
        #[case] held_inode: Option<(u64, u64)>,
        #[case] current_inode: Option<(u64, u64)>,
        #[case] panel_terminal: Option<&[&str]>,
        #[case] terminal_env: Option<&str>,
        #[case] expected: Vec<String>,
    ) {
        let resolved = resolve(
            direction,
            held_inode,
            current_inode,
            panel_terminal,
            terminal_env,
        );
        assert_eq!(resolved, Ok(expected));
    }

    /// A launch with no terminal source, and a launch with no held program,
    /// both fail with a reason that names what is missing, so the desktop
    /// notification does too (spec pin-swap "Neither set" and "No Client
    /// program beside the Owner program").
    #[rstest]
    #[case::neither_set_names_both(Unpin, Some(HELD), None, None, "[panel] terminal")]
    #[case::empty_terminal_env_counts_as_unset(Unpin, Some(HELD), None, Some(""), "TERMINAL")]
    #[case::no_sibling_tui_at_start_names_it(Unpin, None, None, Some("ghostty"), "no mbv-tui ")]
    #[case::no_sibling_gui_at_start_names_it(Open, None, None, Some("ghostty"), "no mbv ")]
    fn swap_argv_without_a_launch_names_what_is_missing(
        #[case] direction: mbv_daemon::SwapDirection,
        #[case] held_inode: Option<(u64, u64)>,
        #[case] panel_terminal: Option<&[&str]>,
        #[case] terminal_env: Option<&str>,
        #[case] named: &str,
    ) {
        let error = resolve(
            direction,
            held_inode,
            Some(HELD),
            panel_terminal,
            terminal_env,
        )
        .expect_err("a launch without a program or a terminal must fail");
        assert!(error.contains(named), "error must name what is missing");
    }
}
