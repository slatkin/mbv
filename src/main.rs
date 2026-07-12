mod app;
mod config;
mod login;
mod mpris;
mod relay;
mod single_instance;
mod terminal_client;
mod tray;

use app::App;
use config::load_config;
use mbv_core::api::EmbyClient;
use mbv_core::{applog, player, remote_player};

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
/// whether the daemon is local (`is_local_daemon`) or genuinely remote.
fn run_remote_app(
    client: EmbyClient,
    remote: remote_player::RemotePlayer,
    player_rx: std::sync::mpsc::Receiver<player::PlayerEvent>,
    is_local_daemon: bool,
) {
    if let Err(e) = App::new_remote(client, remote, player_rx, is_local_daemon).run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
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
            endpoint = Some(value.to_string());
        }
    }
    Ok(endpoint)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

/// Authenticate `client`, falling back to the interactive login TUI on
/// failure. Only called on the paths that actually go on to use the
/// resulting client (explicit daemon endpoint, and the branch that runs
/// `App::new(client).run()`) -- paths that discard `client` (Reattach, and
/// the stay-alive launcher-becomes-terminal-client branch) must not pay for
/// this network round-trip since the inferior/relay authenticates on its
/// own.
fn authenticate_or_login(mut client: EmbyClient, ui_config: &config::UiConfig) -> EmbyClient {
    let t0 = std::time::Instant::now();
    let auth_result = client.authenticate();
    log::info!(target: "startup", "authenticate: {}ms result={}", t0.elapsed().as_millis(), if auth_result.is_ok() { "ok" } else { "err" });
    if auth_result.is_err() {
        client = match login::run(client, ui_config) {
            Ok(c) => c,
            Err(_) => std::process::exit(0),
        };
    }
    client
}

fn state_dir() -> std::path::PathBuf {
    std::env::var("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".local")
                .join("state")
        })
        .join("mbv")
}

fn crash_log_path() -> std::path::PathBuf {
    state_dir().join("mbv.log")
}

fn write_crash_log(msg: &str) {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    // Write directly to stderr (async-signal-safe, no mutex)
    use std::io::Write;
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
    unsafe {
        for &sig in &[libc::SIGSEGV, libc::SIGILL, libc::SIGBUS, libc::SIGFPE] {
            libc::signal(
                sig,
                signal_handler as extern "C" fn(libc::c_int) as libc::sighandler_t,
            );
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

    unsafe {
        libc::write(libc::STDERR_FILENO, msg.as_ptr().cast(), msg.len());
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

/// Parses the hidden `--__relay <sock> -- <inferior argv...>` self-spawn
/// form. Returns `None` if `--__relay` isn't present (the ordinary launch
/// path). This must be checked before any other CLI parsing since its argv
/// shape doesn't match the rest of mbv's flags.
fn parse_relay_args(args: &[String]) -> Option<(String, Vec<String>)> {
    let pos = args.iter().position(|a| a == "--__relay")?;
    let socket_path = args.get(pos + 1)?.clone();
    let sep = args.iter().position(|a| a == "--")?;
    let inferior: Vec<String> = args[sep + 1..].to_vec();
    Some((socket_path, inferior))
}

fn print_usage() {
    println!("mbv {}", env!("CARGO_PKG_VERSION"));
    println!("Usage: mbv [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -a, --alive               Stay alive: keep playing in the background after the");
    println!("                             terminal detaches, and reattach to it later by running");
    println!("                             `mbv` again in a terminal.");
    println!("  -q                        Quit a running mbv session (foreground or stay-alive).");
    println!("      --connect-daemon <endpoint>");
    println!("                             Connect as a thin client to a running mbvd daemon at");
    println!("                             <endpoint> instead of owning a local Player.");
    println!("  -V, --version              Print the version and exit.");
    println!("  -h, --help                 Print this help message and exit.");
}

fn main() {
    install_panic_hook();
    install_signal_handlers();

    let args: Vec<String> = std::env::args().skip(1).collect();

    if has_flag(&args, "-h") || has_flag(&args, "--help") {
        print_usage();
        return;
    }

    // Hidden relay self-spawn subcommand (T1, ADR 0005). Mirrors the
    // retired `--daemon-inner` self-spawn pattern: `mbv` re-execs itself as
    // `mbv --__relay <sock> -- <inferior argv...>` and never returns.
    if let Some((socket_path, inferior)) = parse_relay_args(&args) {
        applog::init(false, Some(state_dir().join("relay.log")));
        relay::run_relay_main(socket_path, inferior);
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
        return;
    }

    // `mbv -q`: quit a running alive session (ADR 0006). Repurposed from the
    // retired `-d` local daemon: reads the PID out of the single-instance
    // lock file and SIGTERMs it for a graceful, non-interactive shutdown
    // (T3) -- works for both a bare foreground session and a stay-alive
    // inferior (its tray Quit does the exact same thing).
    if has_flag(&args, "-q") {
        let lock = single_instance::lock_path();
        match single_instance::read_pid(&lock) {
            Some(pid) => {
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
            }
            None => {
                // Relay may be gated on first attach; check socket connectability
                // before reporting "no running instance".
                if std::os::unix::net::UnixStream::connect(single_instance::socket_path()).is_ok() {
                    eprintln!("mbv: stay-alive relay is starting up but not yet attachable; try again in a moment");
                    std::process::exit(1);
                }
                eprintln!("mbv: no running instance found");
                std::process::exit(1);
            }
        }
        return;
    }

    let alive_requested = has_flag(&args, "-a") || has_flag(&args, "--alive");

    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let ui_config = match config::load_ui_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

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

    let log_stderr = config::is_system_instance();
    let log_path = Some(state_dir().join("mbv.log"));
    applog::init(log_stderr, log_path);
    log::info!(target: "startup", "mbv starting");

    // Construction only (no network I/O) so `client.config` can be read by
    // the branches below regardless of whether this process ends up
    // authenticating at all (bug: double-authentication on paths that
    // discard `client` without ever needing a live session -- Reattach, and
    // the stay-alive launcher-becomes-terminal-client branch).
    let client = EmbyClient::new(config);

    // Explicit endpoint (`--connect-daemon` / config `daemon_client_endpoint`)
    // always wins: a thin client to `mbvd`, owning no Player and taking no
    // flock. Network/mbvd behavior is unchanged by stay-alive (issue #156).
    if let Some(endpoint) = explicit_daemon_endpoint {
        let client = authenticate_or_login(client, &ui_config);
        log::info!(target: "startup", "connecting to explicit daemon endpoint {endpoint}");
        let auth_token = client.token.clone();
        match remote_player::RemotePlayer::connect_endpoint(&endpoint, &auth_token) {
            Ok((remote, player_rx)) => {
                log::info!(target: "startup", "daemon endpoint connected");
                run_remote_app(client, remote, player_rx, endpoint.is_local());
                return;
            }
            Err(e) => {
                eprintln!("mbv: failed to connect to daemon endpoint {endpoint}: {e}");
                std::process::exit(1);
            }
        }
    }

    // Single-instance resolution (ADR 0006): advisory flock + relay-socket
    // connectability. Independent of stay-alive; always on.
    let lock_path = single_instance::lock_path();
    let socket_path = single_instance::socket_path();

    match single_instance::resolve(&socket_path, &lock_path) {
        Ok(single_instance::Resolution::Reattach) => {
            // A live alive session exists: attach as a terminal-client.
            // Same code path whether this is the very first attach after
            // `mbv -a` spawned the relay moments ago, or a later reattach
            // (ADR 0005 "uniform attach topology").
            log::info!(target: "startup", "alive session detected; attaching");
            let code = terminal_client::run_terminal_client(&socket_path.to_string_lossy());
            std::process::exit(code);
        }
        Ok(single_instance::Resolution::Refuse) => {
            eprintln!(
                "mbv: another mbv instance is already running in a foreground terminal (no relay to attach to)."
            );
            eprintln!("mbv: only one mbv instance may run at a time. Close it, or use `mbv -q` to stop it.");
            std::process::exit(1);
        }
        Ok(single_instance::Resolution::Fresh(mut guard)) => {
            // Are we already the gated inferior a relay is hosting on a pty
            // slave? Only `relay::start_inferior` ever sets this env var, on
            // the actual inferior it execs -- so its presence is a reliable
            // "I must not spawn a relay of my own" signal, independent of
            // what `client.config.stay_alive` says (bug: without this check,
            // an inferior launched because of a *config* `stay_alive = true`
            // -- as opposed to `-a`, which is stripped from its argv -- would
            // see the same config value, take this same branch again, and
            // spawn an unbounded chain of nested relays).
            let is_inferior = std::env::var(relay::CTRL_FD_ENV).is_ok();
            let stay_alive = !is_inferior && (alive_requested || client.config.stay_alive);
            if stay_alive {
                // This process was just a liveness probe: release the lock
                // immediately (the inferior below reacquires it for real,
                // becoming the actual Player-owning app) and become a
                // terminal-client ourselves. We never needed a live Emby
                // session for this -- `client` is discarded here, not
                // handed to the inferior -- so skip authentication entirely.
                drop(guard);
                let exe = match std::env::current_exe() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("mbv: cannot locate binary: {e}");
                        std::process::exit(1);
                    }
                };
                // Inferior argv = this same invocation, minus -a/--alive (it
                // must not try to spawn a second relay). No --connect-daemon
                // can be present here: that always returns above, before
                // this branch is reached, so stay-alive never applies to
                // thin clients.
                let inferior_argv: Vec<String> =
                    std::iter::once(exe.to_string_lossy().into_owned())
                        .chain(
                            args.iter()
                                .filter(|a| a.as_str() != "-a" && a.as_str() != "--alive")
                                .cloned(),
                        )
                        .collect();
                if let Err(e) = relay::spawn_detached(&socket_path.to_string_lossy(), inferior_argv)
                {
                    eprintln!("mbv: failed to start stay-alive session: {e}");
                    std::process::exit(1);
                }
                let code = terminal_client::run_terminal_client(&socket_path.to_string_lossy());
                std::process::exit(code);
            }

            // Only reached by a bare fresh launch, or by the inferior itself
            // (which must always fall through here rather than re-evaluate
            // the stay-alive branch above) -- both actually need a live,
            // Player-owning `EmbyClient`, so authenticate now.
            let client = authenticate_or_login(client, &ui_config);

            if let Err(e) = guard.write_pid() {
                log::warn!(target: "startup", "failed to write pid into lock file: {e}");
            }
            if let Err(e) = App::new(client).run() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
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
        assert!(connect_daemon_arg(&["--connect-daemon".into()]).is_err());
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

    #[test]
    fn parse_relay_args_extracts_socket_and_inferior() {
        let args: Vec<String> = ["--__relay", "/tmp/x.sock", "--", "mbv", "-a"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (sock, inferior) = parse_relay_args(&args).unwrap();
        assert_eq!(sock, "/tmp/x.sock");
        assert_eq!(inferior, vec!["mbv".to_string(), "-a".to_string()]);
    }

    #[test]
    fn parse_relay_args_none_without_flag() {
        let args: Vec<String> = vec!["-a".into()];
        assert!(parse_relay_args(&args).is_none());
    }
}
