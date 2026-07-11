//! Terminal-client: raw-mode the real terminal, connect to the relay's
//! socket, relay real-terminal<->socket, and restore the terminal on every
//! teardown path (Drop guard + panic hook + signal handlers).
//!
//! Ported from the Phase 0 spike (`spikes/relay/src/client.rs`, issue #161).
//! What bare `mbv` becomes when it attaches to an alive session (first
//! attach or reattach — same code path, see ADR 0005 "uniform attach
//! topology").
//!
//! Exit codes: 0 = clean detach; 3 = socket stale/absent/refused (NEVER a
//! panic — ADR 0006 "never trust socket-file existence"). Callers should
//! treat exit 3 as "stale socket, start a fresh session".

use crate::relay::{encode_winsize, RESTORE_SEQ, TAG_DATA, TAG_WINSZ};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
use nix::sys::termios::{tcgetattr, tcsetattr, SetArg, Termios};
use std::io::{Read, Write};
use std::os::fd::BorrowedFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::OnceLock;

pub const EXIT_CLEAN_DETACH: i32 = 0;
pub const EXIT_STALE_SOCKET: i32 = 3;

static ORIG_TERMIOS: OnceLock<libc::termios> = OnceLock::new();
static TTY_FD: AtomicI32 = AtomicI32::new(-1);
static RESTORED: AtomicBool = AtomicBool::new(false);
static RESIZE_PENDING: AtomicBool = AtomicBool::new(false);

/// Async-signal-safe-ish restore: only a termios ioctl + raw writes, no
/// allocation. Idempotent (safe to call from Drop, panic hook, AND a signal
/// handler; only the first caller does anything).
fn restore_terminal() {
    if RESTORED.swap(true, Ordering::SeqCst) {
        return;
    }
    let fd = TTY_FD.load(Ordering::SeqCst);
    if fd < 0 {
        return;
    }
    if let Some(t) = ORIG_TERMIOS.get() {
        let bfd = unsafe { BorrowedFd::borrow_raw(fd) };
        let t: Termios = (*t).into();
        let _ = tcsetattr(bfd, SetArg::TCSANOW, &t);
    }
    unsafe {
        libc::write(fd, RESTORE_SEQ.as_ptr() as *const _, RESTORE_SEQ.len());
    }
}

extern "C" fn on_fatal_signal(_sig: libc::c_int) {
    restore_terminal();
    unsafe { libc::_exit(1) };
}

extern "C" fn on_sigwinch(_sig: libc::c_int) {
    RESIZE_PENDING.store(true, Ordering::SeqCst);
}

struct RestoreGuard;
impl Drop for RestoreGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn install_hooks(tty_fd: i32, orig: Termios) {
    TTY_FD.store(tty_fd, Ordering::SeqCst);
    let _ = ORIG_TERMIOS.set(orig.into());

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    unsafe {
        let sa = SigAction::new(
            SigHandler::Handler(on_fatal_signal),
            SaFlags::empty(),
            SigSet::empty(),
        );
        let _ = sigaction(Signal::SIGTERM, &sa);
        let _ = sigaction(Signal::SIGHUP, &sa);

        // SA_RESTART: don't let SIGWINCH kick blocking reads (stdin, data
        // socket) out with EINTR.
        let sa_winch = SigAction::new(
            SigHandler::Handler(on_sigwinch),
            SaFlags::SA_RESTART,
            SigSet::empty(),
        );
        let _ = sigaction(Signal::SIGWINCH, &sa_winch);
    }
}

fn send_winsize(winsz: &mut UnixStream, fd: i32) {
    if let Ok(ws) = crate::relay::get_winsize(fd) {
        let _ = winsz.write_all(&encode_winsize(&ws));
    }
}

/// Attach to the relay at `socket_path` as a terminal-client, taking over
/// the real terminal for the duration of the session. Returns an exit code
/// (see `EXIT_*` constants above) — never panics on a stale/absent socket.
pub fn run_terminal_client(socket_path: &str) -> i32 {
    let tty_fd = 0;
    let orig = match tcgetattr(unsafe { BorrowedFd::borrow_raw(tty_fd) }) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("client: tcgetattr failed: {e}");
            return EXIT_STALE_SOCKET;
        }
    };
    install_hooks(tty_fd, orig);
    let _guard = RestoreGuard;

    // Connect BEFORE touching the terminal (raw mode / alt-screen). A
    // stale or refused socket then fails cleanly with the terminal
    // untouched, and NEVER panics (ADR 0006).
    let mut data = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mbv: cannot connect to relay at {socket_path}: {e}");
            eprintln!("mbv: socket appears stale or absent (no live session). Not attaching.");
            return EXIT_STALE_SOCKET;
        }
    };
    let mut hs = Vec::with_capacity(9);
    hs.push(TAG_DATA);
    let ws0 = crate::relay::get_winsize(tty_fd).unwrap_or(libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    });
    hs.extend_from_slice(&encode_winsize(&ws0));
    if let Err(e) = data.write_all(&hs) {
        eprintln!("mbv: relay closed during handshake: {e}");
        return EXIT_STALE_SOCKET;
    }

    let mut winsz = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mbv: cannot open resize channel to relay: {e}");
            return EXIT_STALE_SOCKET;
        }
    };
    if winsz.write_all(&[TAG_WINSZ]).is_err() {
        return EXIT_STALE_SOCKET;
    }
    send_winsize(&mut winsz, tty_fd);

    // Only NOW take over the terminal -- the relay is confirmed live.
    if crossterm::terminal::enable_raw_mode().is_err() {
        return EXIT_STALE_SOCKET;
    }
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen, Hide, EnableMouseCapture).ok();
    log::info!(target: "terminal_client", "attached to {socket_path}");

    // stdin -> data socket
    {
        let mut data_w = data.try_clone().expect("clone data");
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let stdin_fd = 0;
            loop {
                let n = unsafe { libc::read(stdin_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
                if n < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                {
                    continue; // EINTR (e.g. SIGWINCH landed on this thread): retry, not EOF.
                }
                if n <= 0 {
                    break;
                }
                if data_w.write_all(&buf[..n as usize]).is_err() {
                    break;
                }
            }
        });
    }

    // resize poller: SIGWINCH sets a flag; this thread notices and sends new size
    {
        let mut winsz = winsz;
        std::thread::spawn(move || loop {
            if RESIZE_PENDING.swap(false, Ordering::SeqCst) {
                send_winsize(&mut winsz, tty_fd);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        });
    }

    // data socket -> stdout (main thread; on EOF/error we're detached/evicted)
    let mut buf = [0u8; 4096];
    loop {
        match data.read(&mut buf) {
            Ok(0) => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
            Ok(n) => {
                let _ = out.write_all(&buf[..n]);
                let _ = out.flush();
            }
        }
    }

    log::info!(target: "terminal_client", "detached, restoring terminal");
    restore_terminal();
    execute!(
        std::io::stdout(),
        DisableMouseCapture,
        Show,
        LeaveAlternateScreen
    )
    .ok();
    let _ = crossterm::terminal::disable_raw_mode();
    EXIT_CLEAN_DETACH
}
