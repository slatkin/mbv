mod error;

pub use error::DaemonError;
use mbv_config as config;
use mbv_core::applog;
use mimalloc::MiMalloc;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

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

fn print_usage() {
    eprintln!(
        "Usage: mbvd [--audio-only] [--log-level <level[,target=level...]>] [-q|--quit] [--connect emby] [--connect abs] [--disconnect abs] [--version]"
    );
    eprintln!(
        "  --log-level accepts a level (error, warn, info, debug, trace; default info) optionally followed by target overrides, e.g. info,player=debug"
    );
}

fn daemon_running() -> bool {
    mbv_daemon::locked_owner_pid(&mbv_daemon::pid_file()).is_some()
}

fn stop_daemon() -> Result<String, DaemonError> {
    // The packaged command always uses the daemon's system-instance paths.
    // SAFETY: This CLI action runs synchronously before the daemon or worker threads start.
    unsafe { std::env::set_var("MBV_SYSTEM", "1") };
    let pid = mbv_daemon::signal_owner(&mbv_daemon::pid_file())
        .map_err(|error| DaemonError::failure_context("mbvd: failed to stop daemon", error))?;
    Ok(format!("mbvd: daemon stopped (pid {pid})"))
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Serve {
        audio_only: bool,
        log_level: applog::LogSpec,
    },
    ConnectEmby,
    ConnectAbs,
    DisconnectAbs,
    Quit,
    Help,
    Version,
}

fn parse_action(args: &[String]) -> Result<Action, DaemonError> {
    let mut audio_only = false;
    let mut log_level = applog::LogSpec::default();
    let mut action = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--audio-only" => audio_only = true,
            "--log-level" => log_level = parse_log_level(args, &mut i)?,
            "--help" | "-h" => select_action(&mut action, Action::Help)?,
            "--version" | "-V" => select_action(&mut action, Action::Version)?,
            "--quit" | "-q" => select_action(&mut action, Action::Quit)?,
            "--connect" => parse_connect_action(args, &mut i, &mut action)?,
            "--disconnect" => parse_disconnect_action(args, &mut i, &mut action)?,
            arg => {
                return Err(DaemonError::usage(format!(
                    "mbvd: unknown argument {arg:?}"
                )));
            }
        }
        i += 1;
    }
    if audio_only
        && matches!(
            action,
            Some(Action::ConnectEmby | Action::ConnectAbs | Action::DisconnectAbs)
        )
    {
        return Err(DaemonError::usage(
            "mbvd: service administration cannot be combined with daemon selectors",
        ));
    }
    Ok(action.unwrap_or(Action::Serve {
        audio_only,
        log_level,
    }))
}

fn parse_log_level(args: &[String], i: &mut usize) -> Result<applog::LogSpec, DaemonError> {
    *i += 1;
    let Some(value) = args.get(*i) else {
        return Err(DaemonError::usage(
            "mbvd: --log-level requires a level or level[,target=level...] list",
        ));
    };
    applog::LogSpec::parse(value).map_err(|error| DaemonError::usage(format!("mbvd: {error}")))
}

fn parse_connect_action(
    args: &[String],
    i: &mut usize,
    action: &mut Option<Action>,
) -> Result<(), DaemonError> {
    *i += 1;
    let Some(service) = args.get(*i) else {
        return Err(DaemonError::usage(
            "mbvd: --connect requires a Service (supported: emby, abs)",
        ));
    };
    let next = match service.as_str() {
        "emby" => Action::ConnectEmby,
        "abs" => Action::ConnectAbs,
        _ => {
            return Err(DaemonError::usage(
                "mbvd: unsupported Service; supported Services: emby, abs",
            ));
        }
    };
    select_action(action, next)
}

fn parse_disconnect_action(
    args: &[String],
    i: &mut usize,
    action: &mut Option<Action>,
) -> Result<(), DaemonError> {
    *i += 1;
    let Some(service) = args.get(*i) else {
        return Err(DaemonError::usage(
            "mbvd: --disconnect requires a Service (supported: abs)",
        ));
    };
    if service != "abs" {
        return Err(DaemonError::usage(
            "mbvd: unsupported Service; supported Services: abs",
        ));
    }
    select_action(action, Action::DisconnectAbs)
}

fn select_action(action: &mut Option<Action>, next: Action) -> Result<(), DaemonError> {
    if action.is_some() {
        return Err(DaemonError::usage(
            "mbvd: action selectors are mutually exclusive",
        ));
    }
    *action = Some(next);
    Ok(())
}

fn interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn prompt(label: &str) -> Result<String, DaemonError> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|error| DaemonError::failure_context("mbvd: prompt failed", error))?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .map_err(|error| DaemonError::failure_context("mbvd: input failed", error))?;
    Ok(value.trim().to_string())
}

fn prompt_secret(label: &str) -> Result<String, DaemonError> {
    let stdin = io::stdin();
    let mut termios = nix::sys::termios::tcgetattr(&stdin)
        .map_err(|error| DaemonError::failure_context("mbvd: prompt failed", error))?;
    let original = termios.clone();
    termios
        .local_flags
        .remove(nix::sys::termios::LocalFlags::ECHO);
    nix::sys::termios::tcsetattr(&stdin, nix::sys::termios::SetArg::TCSADRAIN, &termios)
        .map_err(|error| DaemonError::failure_context("mbvd: prompt failed", error))?;
    let result = prompt(label);
    let _ = nix::sys::termios::tcsetattr(&stdin, nix::sys::termios::SetArg::TCSADRAIN, &original);
    println!();
    result
}

fn administration_lock(stem: &str) -> Result<nix::fcntl::Flock<std::fs::File>, DaemonError> {
    let path = config::data_dir_system_or_local().join(format!("{stem}-connect.lock"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            DaemonError::failure_context("mbvd: cannot create administration lock", error)
        })?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|error| {
            DaemonError::failure_context("mbvd: cannot open administration lock", error)
        })?;
    nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock).map_err(
        |(_, error)| {
            DaemonError::failure(format!(
                "mbvd: another {stem} administration command is running: {error}"
            ))
        },
    )
}

fn classified_auth_error(is_auth: bool) -> &'static str {
    if is_auth {
        "mbvd: Emby authentication rejected"
    } else {
        "mbvd: Emby server unavailable or returned an invalid authentication response"
    }
}

fn exchange_emby_credentials(
    client: &mbv_emby::EmbyClient,
    server_url: &str,
    username: &str,
    password: &str,
) -> Result<mbv_emby::EmbyCredentialExchange, DaemonError> {
    Ok(client.exchange_credentials_bounded(
        server_url,
        username,
        password,
        Duration::from_secs(10),
    )?)
}

fn commit_emby_setup(
    existing: Option<&config::EmbySetup>,
    exchange: mbv_emby::EmbyCredentialExchange,
) -> Result<config::EmbySetup, DaemonError> {
    let mut setup = config::EmbySetup::new(exchange.server_url.clone(), exchange.user_id);
    setup.revision = match existing.as_ref() {
        None => 1,
        Some(old) => old
            .revision
            .checked_add(1)
            .ok_or_else(|| DaemonError::failure("mbvd: Emby setup revision exhausted"))?,
    };
    let same_server = existing
        .as_ref()
        .is_some_and(|old| old.server_url == setup.server_url);
    if existing.is_none() || same_server {
        config::persist_emby_setup_and_secret(&setup, &exchange.token).map_err(|error| {
            DaemonError::failure_context("mbvd: could not persist Emby setup", error)
        })?;
    } else {
        config::replace_emby_setup_and_secret(&setup, &exchange.token).map_err(|error| {
            DaemonError::failure_context("mbvd: could not replace Emby setup", error)
        })?;
    }
    Ok(setup)
}

fn connect_emby() -> Result<(), DaemonError> {
    if !interactive_terminal() {
        return Err(DaemonError::usage(
            "mbvd: --connect emby requires an interactive terminal",
        ));
    }
    // The packaged command always uses the daemon's system-instance paths.
    // SAFETY: This CLI action runs synchronously before the daemon or worker threads start.
    unsafe { std::env::set_var("MBV_SYSTEM", "1") };
    let _lock = administration_lock("emby")?;
    let server_url = prompt("Emby server URL")?;
    let username = prompt("Username")?;
    let password = prompt_secret("Password")?;
    let config = config::load_config().map_err(|error| {
        DaemonError::failure_context("mbvd: could not load owner configuration", error)
    })?;
    let existing = config.emby_setup.clone();
    let client = mbv_emby::EmbyClient::new(config);
    let exchange = exchange_emby_credentials(&client, &server_url, &username, &password)?;
    let setup = commit_emby_setup(existing.as_ref(), exchange)?;
    if daemon_running() {
        reconcile_running_owner(mbv_queue::ServiceKind::Emby, setup.revision)?;
        println!(
            "mbvd: Emby setup committed and active for {}",
            setup.server_url
        );
    } else {
        println!(
            "mbvd: Emby setup committed for {}; loaded on next startup",
            setup.server_url
        );
    }
    Ok(())
}

fn classified_abs_error(error: &mbv_audiobookshelf::AudiobookshelfError) -> String {
    if error.is_authentication_rejected() {
        "mbvd: Audiobookshelf authentication rejected".into()
    } else {
        "mbvd: Audiobookshelf server unavailable or returned an invalid response".into()
    }
}

fn clear_audiobookshelf_owned_state() -> Result<(), DaemonError> {
    match config::load_queue_state() {
        Some(state) if !state.items.is_empty() => {
            Ok(config::save_queue_state(&state.without_audiobookshelf())?)
        }
        _ => Ok(config::clear_queue_state()?),
    }
}

fn connect_abs() -> Result<(), DaemonError> {
    if !interactive_terminal() {
        return Err(DaemonError::usage(
            "mbvd: --connect abs requires an interactive terminal",
        ));
    }
    // The packaged command always uses the daemon's system-instance paths.
    // SAFETY: This CLI action runs synchronously before the daemon or worker threads start.
    unsafe { std::env::set_var("MBV_SYSTEM", "1") };
    let _lock = administration_lock("abs")?;
    let server_url = prompt("Audiobookshelf server URL")?;
    let api_key = prompt_secret("Audiobookshelf API key")?;
    let config = config::load_config().map_err(|error| {
        DaemonError::failure_context("mbvd: could not load owner configuration", error)
    })?;
    let existing = config.audiobookshelf_setup.clone();
    let old_queue = config::load_queue_state();
    let validated = mbv_audiobookshelf::AudiobookshelfClient::validate_setup_bounded(
        &server_url,
        &api_key,
        Duration::from_secs(10),
    )?;
    let (setup, _user, api_key) = validated.into_parts();
    let same_server = existing
        .as_ref()
        .is_some_and(|old| old.server_url == setup.server_url);
    let revision = if existing.is_none() || same_server {
        config::persist_audiobookshelf_setup_and_secret(&setup, &api_key).map_err(|error| {
            DaemonError::failure_context("mbvd: could not persist Audiobookshelf setup", error)
        })?
    } else {
        config::replace_audiobookshelf_setup_and_secret(
            &setup,
            &api_key,
            clear_audiobookshelf_owned_state,
            move || {
                if let Some(old) = old_queue.as_ref() {
                    let _ = config::save_queue_state(old);
                }
            },
        )
        .map_err(|error| {
            DaemonError::failure_context("mbvd: could not replace Audiobookshelf setup", error)
        })?
    };
    if daemon_running() {
        reconcile_running_owner(mbv_queue::ServiceKind::Audiobookshelf, revision)?;
        println!(
            "mbvd: Audiobookshelf setup committed and active for {}",
            setup.server_url
        );
    } else {
        println!(
            "mbvd: Audiobookshelf setup committed for {}; loaded on next startup",
            setup.server_url
        );
    }
    Ok(())
}

fn disconnect_abs() -> Result<(), DaemonError> {
    if !interactive_terminal() {
        return Err(DaemonError::usage(
            "mbvd: --disconnect abs requires an interactive terminal",
        ));
    }
    // SAFETY: This CLI action runs synchronously before the daemon or worker threads start.
    unsafe { std::env::set_var("MBV_SYSTEM", "1") };
    let _lock = administration_lock("abs")?;
    let config = config::load_config().map_err(|error| {
        DaemonError::failure_context("mbvd: could not load owner configuration", error)
    })?;
    let was_installed = config.audiobookshelf_setup.is_some();
    config::remove_audiobookshelf_setup_and_secret_with_owned_state(
        clear_audiobookshelf_owned_state,
        || {},
    )
    .map_err(|error| {
        DaemonError::failure_context("mbvd: could not remove Audiobookshelf setup", error)
    })?;
    if was_installed {
        println!("mbvd: Audiobookshelf credential removed");
    } else {
        println!("mbvd: no Audiobookshelf setup was installed");
    }
    if daemon_running() {
        // A revision of 0 signals removal: the running owner rereads its own
        // storage, sees no setup, and drops its context.
        if let Err(error) = reconcile_running_owner(mbv_queue::ServiceKind::Audiobookshelf, 0) {
            return Err(DaemonError::restart_required(format!(
                "{error}; the running process may retain the deleted key in memory"
            )));
        }
        println!("mbvd: Audiobookshelf setup removed and active");
    } else {
        println!("mbvd: Audiobookshelf setup removed; cleared on next startup");
    }
    Ok(())
}

fn reconcile_event_outcome(event: &mbv_ctrl::CtrlEvent) -> Option<Result<(), DaemonError>> {
    match event {
        mbv_ctrl::CtrlEvent::ServiceSetupApplied { .. } => Some(Ok(())),
        mbv_ctrl::CtrlEvent::ServiceSetupRejected { reason, .. } => {
            Some(Err(DaemonError::restart_required(format!(
                "mbvd: restart required (live setup rejected: {reason:?})"
            ))))
        }
        _ => None,
    }
}

fn wait_for_reconcile_outcome(reader: impl BufRead) -> Result<(), DaemonError> {
    for next in reader.lines() {
        let line = next.map_err(|error| {
            DaemonError::restart_context(
                "mbvd: restart required (setup acknowledgement unavailable)",
                error,
            )
        })?;
        let event = serde_json::from_str::<mbv_ctrl::CtrlEvent>(&line).map_err(|error| {
            DaemonError::restart_context(
                "mbvd: restart required (invalid setup acknowledgement)",
                error,
            )
        })?;
        if let Some(outcome) = reconcile_event_outcome(&event) {
            return outcome;
        }
    }
    Err(DaemonError::restart_required(
        "mbvd: restart required (setup acknowledgement unavailable)",
    ))
}

fn connect_running_owner() -> Result<(UnixStream, BufReader<UnixStream>), DaemonError> {
    let stream = UnixStream::connect(config::control_socket_path()).map_err(|error| {
        DaemonError::restart_context(
            "mbvd: restart required (packaged daemon ctrl unavailable)",
            error,
        )
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .map_err(|error| {
            DaemonError::restart_context(
                "mbvd: restart required (cannot read packaged daemon ctrl)",
                error,
            )
        })?;
    let writer = stream.try_clone().map_err(|error| {
        DaemonError::restart_context(
            "mbvd: restart required (cannot write packaged daemon ctrl)",
            error,
        )
    })?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|error| {
        DaemonError::restart_context(
            "mbvd: restart required (packaged daemon did not acknowledge)",
            error,
        )
    })?;
    match serde_json::from_str::<mbv_ctrl::CtrlEvent>(&line) {
        Ok(mbv_ctrl::CtrlEvent::Hello(hello)) => hello.validate_peer().map_err(|error| {
            DaemonError::restart_context("mbvd: restart required (ctrl protocol mismatch)", error)
        })?,
        _ => {
            return Err(DaemonError::restart_required(
                "mbvd: restart required (invalid packaged daemon ctrl hello)",
            ));
        }
    }
    Ok((writer, reader))
}

fn send_reconcile_request(
    writer: &mut UnixStream,
    kind: mbv_queue::ServiceKind,
    revision: u64,
) -> Result<(), DaemonError> {
    let hello = serde_json::to_string(&mbv_ctrl::CtrlCmd::Hello(mbv_ctrl::CtrlHello::current()))
        .map_err(|error| {
            DaemonError::restart_context(
                "mbvd: restart required (cannot serialize ctrl hello)",
                error,
            )
        })?;
    writeln!(writer, "{hello}")
        .and_then(|()| {
            serde_json::to_string(&mbv_ctrl::CtrlCmd::ApplyServiceSetup { kind, revision })
                .map_err(|error| {
                    io::Error::other(format!("cannot serialize setup request: {error}"))
                })
                .and_then(|request| writeln!(writer, "{request}"))
        })
        .map_err(|error| {
            DaemonError::restart_context(
                "mbvd: restart required (cannot send setup request)",
                error,
            )
        })?;
    writer.flush().map_err(|error| {
        DaemonError::restart_required(format!(
            "mbvd: restart required (cannot flush setup request): {error}"
        ))
    })
}

fn reconcile_running_owner(kind: mbv_queue::ServiceKind, revision: u64) -> Result<(), DaemonError> {
    let (mut writer, reader) = connect_running_owner()?;
    send_reconcile_request(&mut writer, kind, revision)?;
    wait_for_reconcile_outcome(reader)
}

fn log_path() -> std::path::PathBuf {
    config::data_dir_system_or_local().join("mbv.log")
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {info}");
        eprintln!("{msg}");
        tracing::error!(name: "mbvd.panic.captured", target: "crash", error = %msg, "panic captured");
    }));
}

fn install_signal_handlers() {
    let handler = signal_handler as *const () as libc::sighandler_t;
    // SAFETY: each signal is a valid fatal signal, and handler has the C ABI
    // signature expected by libc::signal.
    unsafe {
        for &sig in &[libc::SIGSEGV, libc::SIGILL, libc::SIGBUS, libc::SIGFPE] {
            libc::signal(sig, handler);
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

    // SAFETY: `msg` points to a static byte string valid for `msg.len()`;
    // write is async-signal-safe, and sig is the signal currently handled.
    unsafe {
        libc::write(libc::STDERR_FILENO, msg.as_ptr().cast(), msg.len());
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

fn run() -> Result<(), DaemonError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let action = match parse_action(&args) {
        Ok(action) => action,
        Err(error) => {
            print_usage();
            return Err(error);
        }
    };
    let (audio_only, log_level) = match action {
        Action::Help => {
            print_usage();
            return Ok(());
        }
        Action::Version => {
            println!("mbvd {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Action::ConnectEmby => return connect_emby(),
        Action::ConnectAbs => return connect_abs(),
        Action::DisconnectAbs => return disconnect_abs(),
        Action::Quit => {
            println!("{}", stop_daemon()?);
            return Ok(());
        }
        Action::Serve {
            audio_only,
            log_level,
        } => (audio_only, log_level),
    };
    // The runtime directory is checked once at the process boundary before
    // the owner lock or control socket is taken (harden-owner-process-
    // boundaries design D4). Nothing to do on a system instance.
    if let Err(error) = config::ensure_runtime_dir() {
        return Err(DaemonError::failure(format!("mbvd: {error}")));
    }
    if daemon_running() {
        return Err(DaemonError::failure("mbvd: a daemon is already running"));
    }

    let config = config::load_config()?;
    let is_system = config::is_system_instance();
    let log_path = (!is_system).then(log_path);
    applog::init(is_system, log_path, &log_level);
    tracing::info!(name: "mbvd.startup.started", target: "startup", "mbvd starting");

    mbv_daemon::run_with_options(
        mbv_daemon::DaemonStartupContext::new(config, mbv_daemon::DaemonRole::Packaged),
        audio_only,
        mbv_daemon::DaemonRuntimeHooks {
            on_player_ready: Box::new(|_| {}),
            // Deliberately a stub: mbvd runs as a system service with no
            // user session, so there's no tray to spawn into.
            on_tray_ready: Box::new(|_| None),
            // mbvd is a headless server daemon with no desktop session, so
            // it never starts a Pin swap (design D5): the hook reports that
            // and no notification could ever reach a user.
            swap_command: Box::new(|_| {
                Err("mbvd has no desktop session and cannot swap panels".to_string())
            }),
            notify: Box::new(|_| {}),
        },
    )
    // `Ok` is unreachable: the daemon loop never returns — shutdown always
    // ends in `process::exit`. Only a startup failure yields `Err`; the
    // lock-held refusal is an already-running failure (harden-owner-process-
    // boundaries 1.3).
    .map(|never| match never {})
    .map_err(|error| DaemonError::failure(format!("mbvd: {error}")))
}

fn main() {
    cap_glibc_arenas();
    install_panic_hook();
    install_signal_handlers();
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(exit_code_for_error(&e));
    }
}

fn exit_code_for_error(error: &DaemonError) -> i32 {
    if error.is_restart_required() {
        3
    } else if error.is_usage_error() {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests;
