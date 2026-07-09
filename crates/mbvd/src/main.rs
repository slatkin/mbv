use mbv_core::api::EmbyClient;
use mbv_core::{applog, config, daemon};

fn print_usage() {
    eprintln!("Usage: mbvd [--audio-only] [-q|--quit] [--version]");
}

fn has_flag(args: &[String], long: &str, short: Option<&str>) -> bool {
    args.iter()
        .any(|arg| arg == long || short.is_some_and(|short| arg == short))
}

fn daemon_running() -> bool {
    let Ok(s) = std::fs::read_to_string(daemon::pid_file()) else {
        return false;
    };
    let Ok(pid) = s.trim().parse::<u32>() else {
        return false;
    };
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

fn stop_daemon() -> Result<String, String> {
    let path = daemon::pid_file();
    let pid = std::fs::read_to_string(&path)
        .map_err(|_| "mbvd: no daemon running".to_string())?
        .trim()
        .to_string();
    let ok = std::process::Command::new("kill")
        .arg(&pid)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        let _ = std::fs::remove_file(&path);
        Ok(format!("mbvd: daemon stopped (pid {pid})"))
    } else {
        Err(format!("mbvd: failed to stop daemon (pid {pid})"))
    }
}

fn log_path() -> std::path::PathBuf {
    config::data_dir_system_or_local().join("mbv.log")
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {info}");
        eprintln!("{msg}");
        log::error!(target: "crash", "{msg}");
    }));
}

fn install_signal_handlers() {
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

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if has_flag(&args, "--help", Some("-h")) {
        print_usage();
        return Ok(());
    }
    if has_flag(&args, "--version", Some("-V")) {
        println!("mbvd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if has_flag(&args, "--quit", Some("-q")) {
        println!("{}", stop_daemon()?);
        return Ok(());
    }
    for arg in &args {
        if arg != "--audio-only" {
            print_usage();
            return Err(format!("mbvd: unknown argument {arg:?}"));
        }
    }
    if daemon_running() {
        return Err("mbvd: a daemon is already running".to_string());
    }

    let config = config::load_config()?;
    applog::init(config::is_system_instance(), Some(log_path()));
    log::info!(target: "startup", "mbvd starting");

    let mut client = EmbyClient::new(config);
    if client.authenticate().is_err() {
        return Err("mbvd: no cached credentials; run mbv interactively first".to_string());
    }

    daemon::run_with_options(
        client,
        has_flag(&args, "--audio-only", None),
        daemon::DaemonRuntimeHooks {
            on_player_ready: Box::new(|_| {}),
            on_tray_ready: Box::new(|_| None),
        },
    );
}

fn main() {
    install_panic_hook();
    install_signal_handlers();
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
