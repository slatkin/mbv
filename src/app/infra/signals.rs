use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(in crate::app) static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
// Set only by SIGHUP or stdin POLLHUP (terminal vanished). Never set by q/SIGTERM.
// The watchdog's forced exit arms only on this flag so clean q-quits are never raced.
pub(in crate::app) static TERMINAL_GONE: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_quit_signal(signum: i32) {
    let msg: &[u8] = match signum {
        1 => b"mbv: received SIGHUP (signal 1), requesting quit\n",
        15 => b"mbv: received SIGTERM (signal 15), requesting quit\n",
        _ => b"mbv: received unknown quit signal\n",
    };

    // SAFETY: this signal handler uses only async-signal-safe libc calls
    // (a raw write of a static message) and atomic stores, so it never
    // allocates or takes locks and cannot deadlock on a held stderr lock.
    unsafe { libc::write(libc::STDERR_FILENO, msg.as_ptr().cast(), msg.len()) };
    QUIT_REQUESTED.store(true, Ordering::Relaxed);
    if signum == 1 {
        // SIGHUP — terminal closed
        TERMINAL_GONE.store(true, Ordering::Relaxed);
    }
}

pub(in crate::app) fn install_signal_handlers() {
    unsafe extern "C" {
        fn signal(signum: i32, handler: unsafe extern "C" fn(i32)) -> usize;
    }
    unsafe {
        // SAFETY: registering handlers for SIGHUP and SIGTERM whose bodies
        // only store to an atomic and never allocate or take locks, which is
        // async-signal-safe.
        signal(1, handle_quit_signal); // SIGHUP — terminal closed
        signal(15, handle_quit_signal); // SIGTERM — process termination
    }
}

// Returns true if stdin (fd 0) has POLLHUP — the PTY master was closed.
fn stdin_has_hup() -> bool {
    let mut pfd = libc::pollfd {
        fd: 0,
        events: 0,
        revents: 0,
    };
    // SAFETY: `pfd` is a valid, initialised `pollfd` whose address is valid
    // for the duration of the call; `poll` does not retain the pointer.
    unsafe { libc::poll(&raw mut pfd, 1, 0) > 0 && (pfd.revents & libc::POLLHUP) != 0 }
}

// Watchdog thread detects terminal close even if the TUI event loop is stuck.
pub(in crate::app) fn start_quit_watchdog() {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let hup = stdin_has_hup();
            if hup {
                TERMINAL_GONE.store(true, Ordering::Relaxed);
            }
            if TERMINAL_GONE.load(Ordering::Relaxed) || QUIT_REQUESTED.load(Ordering::Relaxed) {
                QUIT_REQUESTED.store(true, Ordering::Relaxed);
                if TERMINAL_GONE.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(15));
                    std::process::exit(0);
                }
                return; // clean quit — let the main thread finish report_stopped
            }
        }
    });
}
