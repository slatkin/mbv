mod api;
mod app;
mod applog;
mod config;
mod ctrl;
mod daemon;
mod login;
mod mpris;
mod player;
mod remote_player;
mod ws;

use api::EmbyClient;
use app::App;
use config::load_config;

fn prompt_line(label: &str) -> String {
    use std::io::Write;
    print!("{label}");
    let _ = std::io::stdout().flush();
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
    buf.trim().to_string()
}

fn prompt_password(label: &str) -> String {
    use crossterm::event::{Event, KeyCode, KeyEventKind};
    use std::io::Write;
    print!("{label}");
    let _ = std::io::stdout().flush();
    let _ = crossterm::terminal::enable_raw_mode();
    let mut pass = String::new();
    loop {
        if let Ok(Event::Key(key)) = crossterm::event::read() {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Enter => break,
                KeyCode::Backspace => {
                    pass.pop();
                }
                KeyCode::Char(c) => pass.push(c),
                _ => {}
            }
        }
    }
    let _ = crossterm::terminal::disable_raw_mode();
    println!();
    pass
}

fn daemon_running() -> bool {
    let Ok(s) = std::fs::read_to_string(daemon::pid_file()) else {
        return false;
    };
    let Ok(pid) = s.trim().parse::<u32>() else {
        return false;
    };
    // Check if the process is alive via /proc (Linux-specific, no extra deps)
    std::path::Path::new(&format!("/proc/{pid}")).exists()
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

fn crash_log_path() -> std::path::PathBuf {
    std::env::var("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".local")
                .join("state")
        })
        .join("mbv")
        .join("mbv.log")
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

fn main() {
    install_panic_hook();
    install_signal_handlers();

    let args: Vec<String> = std::env::args().skip(1).collect();
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

    // Kill running daemon
    if has_flag(&args, "-q") {
        let path = daemon::pid_file();
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let pid = s.trim().to_string();
                let ok = std::process::Command::new("kill")
                    .arg(&pid)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if ok {
                    let _ = std::fs::remove_file(&path);
                    println!("mbv: daemon stopped (pid {pid})");
                } else {
                    eprintln!("mbv: failed to stop daemon (pid {pid})");
                    std::process::exit(1);
                }
            }
            Err(_) => {
                eprintln!("mbv: no daemon running");
                std::process::exit(1);
            }
        }
        return;
    }

    let daemon_mode = has_flag(&args, "-d");
    let daemon_inner = has_flag(&args, "--daemon-inner");
    let daemon_audio_only = has_flag(&args, "--audio-only");

    let config = match load_config() {
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

    if daemon_mode && explicit_daemon_endpoint.is_some() {
        eprintln!("mbv: daemon client endpoint cannot be used with -d");
        std::process::exit(1);
    }
    if daemon_audio_only && !daemon_mode && !daemon_inner {
        eprintln!("mbv: --audio-only is only supported with -d or --daemon-inner");
        std::process::exit(1);
    }

    let log_capacity = if daemon_inner || config::is_system_instance() {
        0
    } else if config.show_log_tab {
        5000
    } else {
        0
    };
    let log_stderr = config::is_system_instance();
    let log_path = {
        let state_dir = std::env::var("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".local")
                    .join("state")
            })
            .join("mbv");
        Some(state_dir.join("mbv.log"))
    };
    applog::init(log_capacity, log_stderr, log_path);
    log::info!(target: "startup", "mbv starting");

    let mut client = EmbyClient::new(config);

    let t0 = std::time::Instant::now();
    let auth_result = client.authenticate();
    log::info!(target: "startup", "authenticate: {}ms result={}", t0.elapsed().as_millis(), if auth_result.is_ok() { "ok" } else { "err" });
    if auth_result.is_err() {
        if daemon_inner {
            eprintln!("mbv daemon: no cached credentials — run mbv interactively first");
            std::process::exit(1);
        }
        if daemon_mode {
            if client.config.server_url.is_empty() {
                eprintln!("mbv: set server_url in your config file before starting the daemon");
                std::process::exit(1);
            }
            client.config.username = prompt_line("Username: ");
            client.config.password = prompt_password("Password: ");
            if let Err(e) = client.authenticate_credentials() {
                eprintln!("mbv: {e}");
                std::process::exit(1);
            }
        } else {
            client = match login::run(client) {
                Ok(c) => c,
                Err(_) => std::process::exit(0),
            };
        }
    }

    if daemon_mode {
        if daemon_running() {
            eprintln!("a daemon is already running, nacho");
            std::process::exit(1);
        }
        // Spawn a detached copy of ourselves and exit
        let exe = std::env::current_exe().expect("cannot locate binary");
        #[allow(clippy::zombie_processes)]
        let _child = std::process::Command::new(exe)
            .arg("--daemon-inner")
            .args(daemon_audio_only.then_some("--audio-only"))
            .env_remove("MBV_SYSTEM")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("mbv: failed to spawn daemon");
        println!("mbv: daemon started");
        return;
    }

    if daemon_inner {
        daemon::run_with_options(client, daemon_audio_only); // never returns
    }

    if let Some(endpoint) = explicit_daemon_endpoint {
        log::info!(target: "startup", "connecting to explicit daemon endpoint {endpoint}");
        let auth_token = client.token.clone();
        match remote_player::RemotePlayer::connect_endpoint(&endpoint, &auth_token) {
            Ok((remote, player_rx)) => {
                log::info!(target: "startup", "daemon endpoint connected");
                if let Err(e) =
                    App::new_remote(client, remote, player_rx, endpoint.is_local()).run()
                {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                return;
            }
            Err(e) => {
                eprintln!("mbv: failed to connect to daemon endpoint {endpoint}: {e}");
                std::process::exit(1);
            }
        }
    }

    // If a local daemon is running, try to connect to it instead of standalone mode.
    let daemon_existed = daemon_running();
    if daemon_existed {
        log::info!(target: "startup", "daemon detected; connecting to control socket");
        let auth_token = client.token.clone();
        let endpoint = remote_player::DaemonEndpoint::Local;
        match remote_player::RemotePlayer::connect_endpoint(&endpoint, &auth_token) {
            Ok((remote, player_rx)) => {
                log::info!(target: "startup", "daemon socket connected");
                if let Err(e) =
                    App::new_remote(client, remote, player_rx, endpoint.is_local()).run()
                {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                return;
            }
            Err(e) => {
                eprintln!(
                    "mbv: daemon found but control socket unavailable ({e}), starting standalone"
                );
            }
        }
    }

    if let Err(e) = App::new(client).run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    if let Ok(cfg) = load_config() {
        if cfg.daemon_mode_on_exit && !daemon_existed && !daemon_running() {
            let exe = std::env::current_exe().expect("cannot locate binary");
            #[allow(clippy::zombie_processes)]
            let _ = std::process::Command::new(exe)
                .arg("--daemon-inner")
                .env_remove("MBV_SYSTEM")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            println!("mbv: daemon started");
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
            &["-d".into(), "--audio-only".into()],
            "--audio-only"
        ));
        assert!(!has_flag(
            &["--audio-only=false".into(), "--audio".into()],
            "--audio-only"
        ));
    }
}
