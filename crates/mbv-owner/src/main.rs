//! The `mbv-owner` program: the per-user Owner process, the only local host
//! of a Player owner.
//!
//! A Client starts it from beside its own program, else from `PATH`, and
//! attaches to it through the core's Owner link. It links no Client crate, so
//! no terminal UI library and no GUI toolkit enters its dependency graph. The
//! `local_daemon` module holds the run loop, the Pin swap launch, and the tray
//! hooks.

mod local_daemon;

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
    // begins; this runs first in `main`, before any threads spawn.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }
}

fn crash_log_path() -> std::path::PathBuf {
    mbv_config::state_dir().join("mbv.log")
}

/// Write a fatal error to stderr and to the crash log. The Owner holds no
/// terminal, so this writer makes no terminal call.
fn write_crash_log(msg: &str) {
    use std::io::Write;

    // Write directly to stderr (async-signal-safe, no mutex)
    let _ = std::io::stderr().write_all(msg.as_bytes());
    let _ = std::io::stderr().write_all(b"\n");
    tracing::error!(
        name: "crash.log.failed",
        target: "crash",
        { error = %msg },
        "fatal error"
    );
    if let Ok(mut f) = open_crash_log() {
        let _ = writeln!(f, "{msg}");
    }
}

/// Open the crash log for append. The state directory may not exist yet at
/// crash time, so it is created first; a failure there is left for the open
/// to report.
fn open_crash_log() -> std::io::Result<std::fs::File> {
    let path = crash_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
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

/// The Owner program's usage. It has one option: the log level, the same
/// option the TUI program takes, because the launching Client forwards it.
fn print_usage() {
    println!("mbv-owner {}", env!("CARGO_PKG_VERSION"));
    println!("Usage: mbv-owner [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "      --log-level <level[,target=level...]>\n\n     Set the log level, e.g. info or info,player=debug.\n     Levels: error, warn, info (default), debug, trace."
    );
    println!("  -h, --help                 Print this help message and exit.");
}

fn main() {
    cap_glibc_arenas();
    install_panic_hook();
    install_signal_handlers();

    // The runtime directory is checked once at the process boundary, before
    // the Owner lock is taken, so the single-instance resolution always sees
    // a valid private directory.
    if let Err(error) = mbv_config::ensure_runtime_dir() {
        eprintln!("mbv-owner: {error}");
        std::process::exit(1);
    }

    local_daemon::run_local_daemon_main();
}
