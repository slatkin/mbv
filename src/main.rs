use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Cap glibc malloc arenas. libmpv/ffmpeg allocate through glibc directly
/// (mimalloc only covers Rust); with mpv's ~20 threads the default
/// 8*ncores private arenas fragment freed playback memory beyond glibc's
/// top-of-arena trim, pinning tens of MB of RSS (see issue #656). Two
/// shared arenas let free pages be reused and returned. Rust allocations
/// are unaffected — they never reach glibc.
fn cap_glibc_arenas() {
    // SAFETY: mallopt is thread-unsafe only after concurrent allocation
    // begins; this runs first in main, before any threads spawn.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }
}

mod app;
mod config;
mod local_daemon;
mod mpris;
mod single_instance;
mod tray;

use app::{capture_launch_window, current_launch_secs, App, Model};
use config::load_config;
use mbv_core::api::EmbyClient;
use mbv_core::{applog, player, remote_player};

/// Captures the launch window, initializes image pickers, and runs the TUI
/// with the launch window available to the model.
fn run_tui(mut app: App) {
    let launch_window = capture_launch_window(current_launch_secs());
    app.init_image_pickers();
    if let Err(e) = Model::new_with_launch_window(app, launch_window).run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Shared by both daemon-connection call sites in `main()` below: run the
/// TUI as a thin client of a connected daemon, exiting with an error if the
/// event loop itself fails. Callers still `return` after calling this so
/// control flow at each call site stays identical to before.
///
/// `App::new_remote` starts MPRIS itself (#160, moved there from here in
/// #175 so `App` owns the resulting handle and can `rebind` it later --
/// see `App::switch_to_direct_remote` / `App::restore_local_mode`). This is
/// still safe against a same-machine bus-name collision for the reason the
/// original comment here noted: `mbvd` has no D-Bus/zbus dependency and
/// never claims `org.mpris.MediaPlayer2.mbv` itself (`on_player_ready` is
/// wired to a no-op in `crates/mbvd/src/main.rs`), so this client is the
/// only thing that will ever own the name for a daemon-connected session,
/// whether the daemon is local or genuinely remote.
fn run_remote_app(
    client: Option<EmbyClient>,
    remote: remote_player::RemotePlayer,
    player_rx: std::sync::mpsc::Receiver<player::PlayerEvent>,
    endpoint: &remote_player::DaemonEndpoint,
    config: config::Config,
) {
    let app = App::new_remote_optional_with_config(client, remote, player_rx, endpoint, config);
    run_tui(app);
}

fn parse_log_level_arg(args: &[String]) -> Result<Option<applog::Level>, String> {
    let mut level = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--log-level" {
            i += 1;
            let Some(value) = args.get(i) else {
                return Err("mbv: --log-level requires error, warn, info, or debug".into());
            };
            level = Some(
                applog::Level::parse(value)
                    .ok_or_else(|| format!("mbv: invalid log level {value:?}"))?,
            );
        }
        i += 1;
    }
    Ok(level)
}

fn connect_daemon_arg(args: &[String]) -> Result<Option<String>, String> {
    let mut endpoint: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--connect-daemon=") {
            endpoint = Some(value.to_string());
        } else if arg == "--connect-daemon" {
            let Some(value) = iter.next() else {
                return Err("mbv: --connect-daemon requires an endpoint".to_string());
            };
            endpoint = Some(value.clone());
        }
    }
    Ok(endpoint)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn cached_emby_client(config: &config::Config) -> Option<EmbyClient> {
    let token = mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby)?;
    let setup = config.emby_setup.as_ref()?;
    let mut client = EmbyClient::new(config.clone());
    client.config.server_url.clone_from(&setup.server_url);
    client.user_id.clone_from(&setup.user_id);
    client.token = token;
    Some(client)
}

fn state_dir() -> std::path::PathBuf {
    std::env::var("XDG_STATE_HOME")
        .map_or_else(
            |_| {
                std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".local")
                    .join("state")
            },
            std::path::PathBuf::from,
        )
        .join("mbv")
}

fn crash_log_path() -> std::path::PathBuf {
    state_dir().join("mbv.log")
}

fn config_diagnostic_summary(config: &config::Config) -> String {
    let mut routes: Vec<_> = config.library_routes.iter().collect();
    routes.sort_by(|a, b| a.0.cmp(b.0));
    let entries = routes
        .into_iter()
        .map(|(library, endpoint)| format!("{library}={endpoint}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "config loaded: auto_reconnect={} library_routes={} entries=[{}]",
        config.auto_reconnect,
        config.library_routes.len(),
        entries
    )
}

fn write_crash_log(msg: &str) {
    use std::io::Write;

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    // Write directly to stderr (async-signal-safe, no mutex)
    let _ = std::io::stderr().write_all(msg.as_bytes());
    let _ = std::io::stderr().write_all(b"\n");
    log::error!(target: "crash", "{msg}");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crash_log_path())
    {
        let _ = writeln!(f, "{msg}");
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {info}");
        write_crash_log(&msg);
        eprintln!("{msg}");
    }));
}

fn install_signal_handlers() {
    // Write a crash log entry for fatal signals before the process dies.
    // SAFETY: install handlers before the application starts concurrent work.
    unsafe {
        for &sig in &[libc::SIGSEGV, libc::SIGILL, libc::SIGBUS, libc::SIGFPE] {
            libc::signal(sig, signal_handler as *const () as libc::sighandler_t);
        }
    }
}

extern "C" fn signal_handler(sig: libc::c_int) {
    let msg: &[u8] = match sig {
        libc::SIGSEGV => b"CRASH: signal SIGSEGV\n",
        libc::SIGILL => b"CRASH: signal SIGILL\n",
        libc::SIGBUS => b"CRASH: signal SIGBUS\n",
        libc::SIGFPE => b"CRASH: signal SIGFPE\n",
        _ => b"CRASH: fatal signal\n",
    };

    // SAFETY: this signal handler uses only async-signal-safe libc calls and a static message.
    unsafe {
        libc::write(libc::STDERR_FILENO, msg.as_ptr().cast(), msg.len());
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

fn print_usage() {
    println!("mbv {}", env!("CARGO_PKG_VERSION"));
    println!("Usage: mbv [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "      --log-level <level>   Set the log level: error, warn, info (default), or debug."
    );
    println!("  -q                        Stop the running Player owner (bare mbv, or the local");
    println!("                             daemon in stay-alive mode).");
    println!("      --connect-daemon <endpoint>");
    println!("                             Attach as a client to a running mbvd daemon at");
    println!("                             <endpoint> instead of owning a local Player.");
    println!("  -V, --version              Print the version and exit.");
    println!("  -h, --help                 Print this help message and exit.");
}

fn pre_config_startup() -> Option<(Option<applog::Level>, Option<String>)> {
    cap_glibc_arenas();
    install_panic_hook();
    install_signal_handlers();

    let args: Vec<String> = std::env::args().skip(1).collect();

    if has_flag(&args, "-h") || has_flag(&args, "--help") {
        print_usage();
        return None;
    }

    let log_level = match parse_log_level_arg(&args) {
        Ok(level) => level,
        Err(error) => {
            print_usage();
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    // Hidden local-daemon self-spawn is checked before other CLI parsing.
    if has_flag(&args, "--__local-daemon") {
        local_daemon::run_local_daemon_main();
    }

    let cli_daemon_endpoint = match connect_daemon_arg(&args) {
        Ok(endpoint) => endpoint,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    if has_flag(&args, "--version") || has_flag(&args, "-V") {
        println!("mbv {}", env!("CARGO_PKG_VERSION"));
        return None;
    }

    if has_flag(&args, "-q") {
        stop_running_instance();
        return None;
    }

    // Reject the removed -d argument before startup side effects.
    if has_flag(&args, "-d") {
        eprintln!("mbv: the `-d` flag has been removed.");
        eprintln!("mbv: to keep the local daemon running after quit, enable `stay_alive` in config or the settings overlay.");
        std::process::exit(1);
    }

    Some((log_level, cli_daemon_endpoint))
}

fn stop_running_instance() {
    let lock = single_instance::lock_path();
    if let Some(pid) = single_instance::read_pid(&lock) {
        // SAFETY: sending SIGTERM to the PID read from the single-instance lock is intentional.
        let ok = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0;
        if ok {
            println!("mbv: quit signal sent (pid {pid})");
        } else {
            eprintln!(
                "mbv: failed to signal pid {pid}: {}",
                std::io::Error::last_os_error()
            );
            std::process::exit(1);
        }
    } else {
        eprintln!("mbv: no running instance found; if one just started, try again in a moment");
        std::process::exit(1);
    }
}

fn main() {
    let Some((log_level, cli_daemon_endpoint)) = pre_config_startup() else {
        return;
    };

    applog::init(
        config::is_system_instance(),
        Some(state_dir().join("mbv.log")),
        log_level.unwrap_or(applog::Level::Info),
    );

    if let Err(e) = config::migrate_legacy_emby_token() {
        eprintln!("mbv: Emby setup migration failed: {e}");
    }
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    log::info!(target: "startup", "{}", config_diagnostic_summary(&config));
    run_configured_startup(log_level, cli_daemon_endpoint, &config);
}

fn run_configured_startup(
    log_level: Option<applog::Level>,
    cli_daemon_endpoint: Option<String>,
    config: &config::Config,
) {
    let explicit_daemon_endpoint = cli_daemon_endpoint
        .or_else(|| {
            let endpoint = config.daemon_client_endpoint.trim();
            (!endpoint.is_empty()).then(|| endpoint.to_string())
        })
        .map(|endpoint| {
            remote_player::DaemonEndpoint::parse(&endpoint).unwrap_or_else(|e| {
                eprintln!("mbv: invalid daemon endpoint {endpoint:?}: {e}");
                std::process::exit(1);
            })
        });

    log::info!(target: "startup", "mbv starting");

    // Explicit endpoint (`--connect-daemon` / config `daemon_client_endpoint`)
    // always wins: a thin client to `mbvd`, owning no Player and taking no
    // flock. Network/mbvd behavior is unchanged by stay-alive (issue #156).
    if let Some(endpoint) = explicit_daemon_endpoint {
        let client = cached_emby_client(config);
        log::info!(target: "startup", "connecting to explicit daemon endpoint {endpoint}");
        println!("Connecting to daemon at {endpoint}...");
        match remote_player::RemotePlayer::connect_endpoint(&endpoint) {
            Ok((remote, player_rx)) => {
                log::info!(target: "startup", "daemon endpoint connected");
                run_remote_app(client, remote, player_rx, &endpoint, config.clone());
                return;
            }
            Err(e) => {
                eprintln!("mbv: failed to connect to daemon endpoint {endpoint}: {e}");
                std::process::exit(1);
            }
        }
    }

    run_local_instance(config, log_level);
}

fn run_local_instance(config: &config::Config, log_level: Option<applog::Level>) {
    // Single-instance resolution (ADR 0006): advisory flock + control-socket
    // connectability. Independent of stay-alive; always on.
    let lock_path = single_instance::lock_path();
    let socket_path = single_instance::socket_path();

    match single_instance::resolve(&socket_path, &lock_path) {
        Ok(single_instance::Resolution::Attach) => {
            // A live local daemon exists: attach as a client alongside any
            // others already attached. Clients take no lock -- that is what
            // permits any number of them.
            log::info!(target: "startup", "local daemon detected; attaching");
            let client = cached_emby_client(config);
            match remote_player::RemotePlayer::connect_endpoint(
                &remote_player::DaemonEndpoint::Local,
            ) {
                Ok((remote, player_rx)) => {
                    run_remote_app(
                        client,
                        remote,
                        player_rx,
                        &remote_player::DaemonEndpoint::Local,
                        config.clone(),
                    );
                }
                Err(e) => {
                    eprintln!("mbv: failed to attach to local daemon: {e}");
                    std::process::exit(1);
                }
            }
        }
        Ok(single_instance::Resolution::Refuse) => {
            eprintln!("mbv: another mbv instance already owns playback in a foreground terminal.");
            match single_instance::read_pid(&lock_path) {
                Some(pid) => eprintln!(
                    "mbv: that instance's PID is {pid} (per {}).",
                    lock_path.display()
                ),
                None => {
                    eprintln!(
                        "mbv: could not determine that instance's PID from {}.",
                        lock_path.display()
                    );
                }
            }
            eprintln!(
                "mbv: only one process can own playback at a time. Close it, stop it with \
                 `mbv -q`, or enable `stay_alive` in config to run several terminals against a local daemon."
            );
            std::process::exit(1);
        }
        Ok(single_instance::Resolution::Fresh(mut guard)) => {
            let stay_alive = config.stay_alive;

            if stay_alive {
                let client = cached_emby_client(config);
                // This process was just a liveness probe: release the lock
                // immediately (the local daemon reacquires it for real,
                // becoming the actual Player-owning process) and attach to
                // it as a client ourselves.
                drop(guard);
                if let Err(e) =
                    local_daemon::spawn_detached(&socket_path.to_string_lossy(), log_level)
                {
                    eprintln!("mbv: failed to start local daemon: {e}");
                    std::process::exit(1);
                }
                match remote_player::RemotePlayer::connect_endpoint(
                    &remote_player::DaemonEndpoint::Local,
                ) {
                    Ok((remote, player_rx)) => {
                        run_remote_app(
                            client,
                            remote,
                            player_rx,
                            &remote_player::DaemonEndpoint::Local,
                            config.clone(),
                        );
                        return;
                    }
                    Err(e) => {
                        eprintln!("mbv: failed to attach to local daemon: {e}");
                        std::process::exit(1);
                    }
                }
            }

            if let Err(e) = guard.write_pid() {
                log::warn!(target: "startup", "failed to write pid into lock file: {e}");
            }
            let app = App::new_independent(config);
            run_tui(app);
            // `guard` drops here (end of scope) at real process exit,
            // releasing the flock -- also happens automatically on any
            // process death (ADR 0006).
        }
        Err(e) => {
            eprintln!("mbv: single-instance check failed: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_arg_accepts_supported_values_and_rejects_invalid_values() {
        for (value, expected) in [
            ("error", applog::Level::Error),
            ("warn", applog::Level::Warn),
            ("info", applog::Level::Info),
            ("debug", applog::Level::Debug),
        ] {
            assert_eq!(
                parse_log_level_arg(&["--log-level".into(), value.into()]).unwrap(),
                Some(expected)
            );
        }
        parse_log_level_arg(&["--log-level".into(), "trace".into()]).unwrap_err();
        parse_log_level_arg(&["--log-level".into()]).unwrap_err();
        assert_eq!(parse_log_level_arg(&[]).unwrap(), None);
    }

    #[test]
    fn connect_daemon_arg_accepts_split_and_equals_forms() {
        assert_eq!(
            connect_daemon_arg(&["--connect-daemon".into(), "local".into()]).unwrap(),
            Some("local".to_string())
        );
        assert_eq!(
            connect_daemon_arg(&["--connect-daemon=unix:///tmp/mbv.sock".into()]).unwrap(),
            Some("unix:///tmp/mbv.sock".to_string())
        );
    }

    #[test]
    fn connect_daemon_arg_requires_value() {
        connect_daemon_arg(&["--connect-daemon".into()]).unwrap_err();
    }

    #[test]
    fn has_flag_matches_exact_flag() {
        assert!(has_flag(
            &["-a".into(), "--audio-only".into()],
            "--audio-only"
        ));
        assert!(!has_flag(
            &["--audio-only=false".into(), "--audio".into()],
            "--audio-only"
        ));
    }

    /// A client missing `user_id` builds `/Users//Views`-shaped paths --
    /// Emby throws on the empty GUID segment rather than rejecting cleanly.
    /// `construct.rs` marks a cached client `Ready` immediately without
    /// re-authenticating, so a blank `user_id` here reaches real requests.
    #[test]
    fn cached_emby_client_carries_user_id_from_setup() {
        let _state_dir = mbv_core::config::TestStateDirGuard::new();
        mbv_core::config::save_service_secret(mbv_core::config::ServiceKind::Emby, "tok").unwrap();
        let config = config::Config {
            emby_setup: Some(mbv_core::config::EmbySetup::new(
                "http://emby.example:8096",
                "the-user-id",
            )),
            ..config::Config::default()
        };
        let client = cached_emby_client(&config).expect("cached client");
        assert_eq!(client.user_id, "the-user-id");
    }

    #[test]
    fn config_diagnostic_summary_is_sorted_and_sanitized() {
        let mut config = config::Config {
            auto_reconnect: true,
            password: "secret-token".to_string(),
            ..config::Config::default()
        };
        config
            .library_routes
            .insert("music".to_string(), "tcp://192.0.2.10:17831".to_string());
        config.library_routes.insert(
            "audiobooks".to_string(),
            "tcp://192.0.2.11:17831".to_string(),
        );
        let summary = config_diagnostic_summary(&config);
        assert_eq!(summary, "config loaded: auto_reconnect=true library_routes=2 entries=[audiobooks=tcp://192.0.2.11:17831, music=tcp://192.0.2.10:17831]");
        assert!(!summary.contains("secret-token"));
    }
}
