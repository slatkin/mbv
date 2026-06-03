mod api;
mod app;
mod applog;
mod config;
mod daemon;
mod login;
mod mpris;
mod player;
mod ws;

use api::EmbyClient;
use app::App;
use config::load_config;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

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
                    println!("mby: daemon stopped (pid {pid})");
                } else {
                    eprintln!("mby: failed to stop daemon (pid {pid})");
                    std::process::exit(1);
                }
            }
            Err(_) => { eprintln!("mby: no daemon running"); std::process::exit(1); }
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
            eprintln!("mby daemon: no cached credentials — run mby interactively first");
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
            .expect("mby: failed to spawn daemon");
        println!("mby: daemon started");
        return;
    }

    if daemon_inner {
        daemon::run(client); // never returns
    }

    if let Err(e) = App::new(client).run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
