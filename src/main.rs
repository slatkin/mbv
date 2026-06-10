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

fn daemon_running() -> bool {
    let Ok(s) = std::fs::read_to_string(daemon::pid_file()) else { return false };
    let Ok(pid) = s.trim().parse::<u32>() else { return false };
    // Check if the process is alive via /proc (Linux-specific, no extra deps)
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.contains(&"--version".to_string()) || args.contains(&"-V".to_string()) {
        println!("mbv {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Kill running daemon
    if args.contains(&"-q".to_string()) {
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
            Err(_) => { eprintln!("mbv: no daemon running"); std::process::exit(1); }
        }
        return;
    }

    let daemon_mode  = args.contains(&"-d".to_string());
    let daemon_inner = args.contains(&"--daemon-inner".to_string());

    let config = match load_config() {
        Ok(c) => c,
        Err(e) => { eprintln!("{e}"); std::process::exit(1); }
    };

    let mut client = EmbyClient::new(config);

    if client.authenticate().is_err() {
        if daemon_inner {
            eprintln!("mbv daemon: no cached credentials — run mbv interactively first");
            std::process::exit(1);
        }
        client = match login::run(client) {
            Ok(c) => c,
            Err(_) => std::process::exit(0),
        };
    }

    if daemon_mode {
        // Spawn a detached copy of ourselves and exit
        let exe = std::env::current_exe().expect("cannot locate binary");
        #[allow(clippy::zombie_processes)]
        let _child = std::process::Command::new(exe)
            .arg("--daemon-inner")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("mbv: failed to spawn daemon");
        println!("mbv: daemon started");
        return;
    }

    if daemon_inner {
        daemon::run(client); // never returns
    }

    // If a daemon is running, try to connect to it instead of standalone mode.
    if daemon_running() {
        match remote_player::RemotePlayer::connect() {
            Ok((remote, player_rx)) => {
                if let Err(e) = App::new_remote(client, remote, player_rx).run() {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                return;
            }
            Err(e) => {
                eprintln!("mbv: daemon found but control socket unavailable ({e}), starting standalone");
            }
        }
    }

    if let Err(e) = App::new(client).run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
