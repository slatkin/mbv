//! Owned pty relay for stay-alive mode (ADR 0005).
//!
//! Ported from the Phase 0 spike (`spikes/relay/src/{relay,lib}.rs`, issue
//! #161) — the primitives below (gate-on-first-attach, waitpid-driven
//! teardown, SIGHUP-ignore + setsid, the out-of-band socketpair control
//! channel, resize propagation, stale-socket-by-connectability) are carried
//! over verbatim; only packaging changed (spike had 3 throwaway binaries +
//! a dummy bash inferior for headless probing; here the relay is a hidden
//! self-spawn subcommand of the single `mbv` binary and the inferior is
//! always the real `mbv` app).
//!
//! Roles:
//! - The **launcher** (an ordinary `mbv -a` / `mbv` with `stay_alive` set)
//!   calls [`spawn_detached`] to fork+detach a relay, then becomes a
//!   terminal-client itself (see `src/terminal_client.rs`).
//! - The **relay** is `mbv --__relay <sock> -- <inferior argv...>`, entered
//!   via [`run_relay_main`], which never returns (it `process::exit`s on
//!   inferior teardown).
//! - The **inferior** is an ordinary `mbv` invocation (the real App::run()
//!   path) with its controlling terminal on the relay's pty slave and an
//!   out-of-band control-channel fd handed to it on fd 3 (see
//!   `MBV_STAYALIVE_CTRL_FD` / `src/app/stay_alive.rs`).

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// Env var the relay sets on the inferior so it knows the out-of-band
/// control-channel socket is on fd 3 (rather than guessing from argv shape).
pub const CTRL_FD_ENV: &str = "MBV_STAYALIVE_CTRL_FD";
pub const CTRL_FD: RawFd = 3;

pub const CTRL_ATTACH: &str = "ATTACH";
pub const CTRL_DETACH: &str = "DETACH";

/// Tag byte sent as the first byte of every socket connection to the relay,
/// so one UnixListener can multiplex the data pipe and the resize control
/// pipe without polluting the data pipe (which must stay a dumb byte pipe
/// for graphics transparency).
pub const TAG_DATA: u8 = 1;
pub const TAG_WINSZ: u8 = 2;
pub const TAG_DATA_READY: u8 = 3;

/// Full restore sequence written directly (no crossterm) so it is safe to
/// call from a signal handler: only raw writes, no allocation.
pub const RESTORE_SEQ: &[u8] =
    b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\x1b[?1049l\x1b[?25h";

fn to_io(e: nix::Error) -> io::Error {
    io::Error::from_raw_os_error(e as i32)
}

fn log(msg: &str) {
    log::info!(target: "relay", "{msg}");
}

/// Set by `on_relay_sigterm` (async-signal-safe: only an atomic store), and
/// polled by a plain thread in `run_relay_main` which does the actual
/// (non-signal-safe) work of forwarding SIGTERM to the inferior.
static TERM_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_relay_sigterm(_sig: libc::c_int) {
    TERM_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn open_pty() -> nix::Result<nix::pty::OpenptyResult> {
    nix::pty::openpty(None, None)
}

/// Post-fork, pre-exec: make `slave_fd` this process's controlling terminal
/// and wire it to fd 0/1/2. Must run in a freshly-forked, single-threaded
/// child (e.g. inside `Command::pre_exec`).
pub fn become_pty_slave(slave_fd: RawFd) -> io::Result<()> {
    nix::unistd::setsid().map_err(to_io)?;
    // Acquiring TIOCSCTTY can transiently race the kernel's disassociation
    // of a previous session leader from this same pty slave; retry briefly
    // rather than treating that race as a hard failure.
    let mut last_err = None;
    let mut ok = false;
    for _ in 0..50 {
        if unsafe { libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) } == 0 {
            ok = true;
            break;
        }
        last_err = Some(io::Error::last_os_error());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !ok {
        return Err(last_err.unwrap());
    }
    for fd in 0..3 {
        if fd != slave_fd {
            nix::unistd::dup2(slave_fd, fd).map_err(to_io)?;
        }
    }
    if slave_fd > 2 {
        let _ = nix::unistd::close(slave_fd);
    }
    Ok(())
}

/// Move `src` to file descriptor `dst`, closing `src` afterward if distinct.
pub fn dup2_fixed(src: RawFd, dst: RawFd) -> io::Result<()> {
    if src != dst {
        nix::unistd::dup2(src, dst).map_err(to_io)?;
        let _ = nix::unistd::close(src);
    }
    Ok(())
}

pub fn get_winsize(fd: RawFd) -> io::Result<libc::winsize> {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws as *mut libc::winsize) };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ws)
}

pub fn set_winsize(fd: RawFd, ws: &libc::winsize) -> io::Result<()> {
    let r = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, ws as *const libc::winsize) };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Wire format for a resize message on the TAG_WINSZ connection: 4x u16 BE
/// (rows, cols, xpixel, ypixel) — mirrors `libc::winsize`.
pub fn encode_winsize(ws: &libc::winsize) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..2].copy_from_slice(&ws.ws_row.to_be_bytes());
    buf[2..4].copy_from_slice(&ws.ws_col.to_be_bytes());
    buf[4..6].copy_from_slice(&ws.ws_xpixel.to_be_bytes());
    buf[6..8].copy_from_slice(&ws.ws_ypixel.to_be_bytes());
    buf
}

pub fn decode_winsize(buf: &[u8; 8]) -> libc::winsize {
    libc::winsize {
        ws_row: u16::from_be_bytes([buf[0], buf[1]]),
        ws_col: u16::from_be_bytes([buf[2], buf[3]]),
        ws_xpixel: u16::from_be_bytes([buf[4], buf[5]]),
        ws_ypixel: u16::from_be_bytes([buf[6], buf[7]]),
    }
}

/// Fork+exec the inferior. `slave`/`app_ctrl` are consumed (moved into the
/// child via pre_exec, then dropped in the parent so the relay holds no
/// extra copies of the slave — that way the pty master EOFs when the
/// inferior's own fds to it go away, though teardown itself is never keyed
/// off that EOF; see the master-reader thread below).
fn start_inferior(
    inferior: Vec<String>,
    slave: OwnedFd,
    app_ctrl: OwnedFd,
    current_data: Arc<Mutex<Option<UnixStream>>>,
    current_winsz: Arc<Mutex<Option<UnixStream>>>,
    socket_path: Arc<String>,
    inferior_pid: Arc<AtomicI32>,
) {
    let slave_raw = slave.as_raw_fd();
    let app_ctrl_raw = app_ctrl.as_raw_fd();

    let mut cmd = Command::new(&inferior[0]);
    cmd.args(&inferior[1..]);
    cmd.env(CTRL_FD_ENV, CTRL_FD.to_string());

    unsafe {
        cmd.pre_exec(move || {
            become_pty_slave(slave_raw)?;
            dup2_fixed(app_ctrl_raw, CTRL_FD)?;
            // The relay ignores SIGHUP (SIG_IGN), and SIG_IGN is inherited
            // across fork AND preserved across execve -- reset SIGHUP to
            // default here so the inferior (mbv) gets normal disposition.
            let sa_dfl = nix::sys::signal::SigAction::new(
                nix::sys::signal::SigHandler::SigDfl,
                nix::sys::signal::SaFlags::empty(),
                nix::sys::signal::SigSet::empty(),
            );
            let _ = nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGHUP, &sa_dfl);
            Ok(())
        });
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log(&format!(
                "FATAL: failed to spawn inferior {inferior:?}: {e}"
            ));
            std::process::exit(1);
        }
    };
    let pid = child.id();
    inferior_pid.store(pid as i32, Ordering::SeqCst);
    drop(slave);
    drop(app_ctrl);
    log(&format!("inferior forked: pid={pid} argv={inferior:?}"));

    // Reaper: the SOLE teardown authority. When the inferior process
    // actually exits (not merely closes its tty), tear the relay down:
    // close any attached client so it restores its terminal, unlink the
    // socket, exit. See ADR 0005 / issue #161 finding 2 — pty-master EOF
    // must NOT be treated as "inferior gone" (mbv closes/replaces its tty
    // fds while still running).
    std::thread::spawn(move || {
        let mut child = child;
        match child.wait() {
            Ok(status) => log(&format!(
                "inferior EXITED: pid={pid} status={status} -> relay teardown"
            )),
            Err(e) => log(&format!(
                "inferior wait() error: pid={pid} {e} -> relay teardown"
            )),
        }
        if let Some(s) = current_data.lock().unwrap().take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        if let Some(s) = current_winsz.lock().unwrap().take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        let _ = std::fs::remove_file(&*socket_path);
        std::process::exit(0);
    });
}

/// Relay main loop. Entered via the hidden `mbv --__relay <sock> -- <argv...>`
/// self-spawn. Never returns.
pub fn run_relay_main(socket_path: String, inferior: Vec<String>) -> ! {
    // The relay IS the SIGHUP firewall (ADR 0005): closing the launching
    // terminal must not kill it. Ignore SIGHUP unconditionally (belt and
    // suspenders alongside the setsid() done at spawn time in
    // `spawn_detached`).
    unsafe {
        let sa = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigIgn,
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        let _ = nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGHUP, &sa);
    }

    // Best-effort graceful teardown on SIGTERM: rather than let the default
    // disposition abruptly kill the relay (orphaning/abruptly killing the
    // inferior), install a handler that just sets a flag (the only thing
    // safe to do inside a signal handler); a plain thread below polls it
    // and forwards SIGTERM to the inferior so it gets its own chance to run
    // its normal graceful-quit path (see `src/app/mod.rs` handle_quit_signal
    // for the app-level SIGTERM path this is deliberately mirroring the
    // intent of, though it's a different process). The reaper thread in
    // `start_inferior` remains the sole teardown authority for the relay
    // itself once the inferior actually exits.
    unsafe {
        let sa_term = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::Handler(on_relay_sigterm),
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        let _ = nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTERM, &sa_term);
    }

    let _ = std::fs::remove_file(&socket_path);
    log(&format!(
        "relay start: socket={socket_path} inferior={inferior:?}"
    ));

    let pty = open_pty().expect("openpty");

    let (relay_ctrl, app_ctrl) = nix::sys::socket::socketpair(
        nix::sys::socket::AddressFamily::Unix,
        nix::sys::socket::SockType::Stream,
        None,
        nix::sys::socket::SockFlag::empty(),
    )
    .expect("socketpair");

    // Two independent fds onto the SAME pty master: one dedicated fd for
    // the single reader thread (its read() blocks indefinitely, so it must
    // never be held behind a mutex shared with writers), one fd (behind a
    // mutex) for writers/ioctl.
    let master_read_raw = nix::unistd::dup(pty.master.as_raw_fd()).expect("dup master for reader");
    let mut master_file_read = unsafe { std::fs::File::from_raw_fd(master_read_raw) };
    let master_file = unsafe { std::fs::File::from_raw_fd(pty.master.into_raw_fd()) };
    let master_file = Arc::new(Mutex::new(master_file));

    let relay_ctrl_stream = unsafe { UnixStream::from_raw_fd(relay_ctrl.into_raw_fd()) };

    // Deferred fork: gated on first client attach (issue #161 finding 1 —
    // mbv's first paint + DA1/graphics query must have a live client to
    // reach, or album art never renders). Until the inferior forks, the
    // relay itself still holds `pty.slave`, so the master never EOFs and
    // the reader thread simply blocks.
    struct Pending {
        inferior: Vec<String>,
        slave: OwnedFd,
        app_ctrl: OwnedFd,
    }
    let pending = Arc::new(Mutex::new(Some(Pending {
        inferior,
        slave: pty.slave,
        app_ctrl,
    })));

    let current_data: Arc<Mutex<Option<UnixStream>>> = Arc::new(Mutex::new(None));
    // TAG_WINSZ mirror of `current_data`: tracks the incumbent resize
    // connection so it can be evicted the same way on newcomer-attach,
    // rather than staying live (and racing the newcomer to set winsize)
    // until the evicted client's process eventually exits.
    let current_winsz: Arc<Mutex<Option<UnixStream>>> = Arc::new(Mutex::new(None));
    let socket_path_arc = Arc::new(socket_path.clone());
    let generation = Arc::new(AtomicI32::new(0));
    let inferior_pid = Arc::new(AtomicI32::new(0));

    // --- thread: poll TERM_REQUESTED (set by the SIGTERM handler) and
    // forward SIGTERM to the inferior, if one is running. If no inferior
    // has been forked yet (still gated on first attach), there is nothing
    // to forward to -- just tear the relay itself down. ---
    {
        let inferior_pid = Arc::clone(&inferior_pid);
        let socket_path_arc = Arc::clone(&socket_path_arc);
        std::thread::spawn(move || loop {
            if TERM_REQUESTED.swap(false, Ordering::SeqCst) {
                let pid = inferior_pid.load(Ordering::SeqCst);
                if pid > 0 {
                    log(&format!(
                        "SIGTERM received: forwarding to inferior pid={pid}"
                    ));
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid),
                        nix::sys::signal::Signal::SIGTERM,
                    );
                } else {
                    log("SIGTERM received: no inferior running yet; relay exiting");
                    let _ = std::fs::remove_file(&*socket_path_arc);
                    std::process::exit(0);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        });
    }

    // --- thread: read relay_ctrl for "DETACH" lines from the app ---
    {
        let current_data = Arc::clone(&current_data);
        let ctrl_reader = relay_ctrl_stream.try_clone().expect("clone ctrl");
        std::thread::spawn(move || {
            use std::io::BufRead;
            let buf = std::io::BufReader::new(ctrl_reader.try_clone().unwrap());
            for line in buf.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                if line.trim() == CTRL_DETACH {
                    if let Some(s) = current_data.lock().unwrap().take() {
                        let _ = s.shutdown(std::net::Shutdown::Both);
                        log("DETACH: closed current data client");
                    }
                }
            }
            let _ = ctrl_reader.shutdown(std::net::Shutdown::Both);
        });
    }

    // --- thread: pty master -> current client (dumb pump; NOT a teardown trigger) ---
    //
    // CRITICAL (issue #161 finding 2): pty-master EOF must NOT tear down
    // the relay — an unmodified mbv can close/replace its controlling-tty
    // fds while STILL RUNNING. The ONLY teardown authority is the reaper's
    // waitpid in `start_inferior`. This thread just pumps bytes and, on
    // master EOF, stops reading and lets the relay keep serving reattaches.
    {
        let current_data = Arc::clone(&current_data);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match master_file_read.read(&mut buf) {
                    Ok(0) => {
                        log("pty master EOF (inferior closed its tty; relay stays alive)");
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        log(&format!("pty master read error: {e} (relay stays alive)"));
                        break;
                    }
                    Ok(n) => {
                        let mut guard = current_data.lock().unwrap();
                        if let Some(s) = guard.as_mut() {
                            if s.write_all(&buf[..n]).is_err() {
                                *guard = None;
                            }
                        }
                        // else: no client attached -> discard (keeps the
                        // pty drained so the inferior never blocks).
                    }
                }
            }
        });
    }

    let listener = UnixListener::bind(&socket_path).expect("bind socket");
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
    log(&format!("socket ready: {socket_path} (gated-on-attach)"));

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let master_file = Arc::clone(&master_file);
        let current_data = Arc::clone(&current_data);
        let current_winsz = Arc::clone(&current_winsz);
        let pending = Arc::clone(&pending);
        let socket_path_arc = Arc::clone(&socket_path_arc);
        let mut ctrl_writer = relay_ctrl_stream.try_clone().expect("clone ctrl");
        let gen = Arc::clone(&generation);
        let inferior_pid = Arc::clone(&inferior_pid);
        std::thread::spawn(move || {
            let mut stream = stream;
            let mut tag = [0u8; 1];
            if stream.read_exact(&mut tag).is_err() {
                return;
            }
            match tag[0] {
                TAG_DATA => {
                    // TAG_DATA is followed by an 8-byte initial winsize so
                    // the relay can size the pty BEFORE forking the (gated)
                    // inferior: matters because mbv reads its size at
                    // startup.
                    let mut wsbuf = [0u8; 8];
                    if stream.read_exact(&mut wsbuf).is_err() {
                        return;
                    }
                    let ws = decode_winsize(&wsbuf);
                    {
                        let f = master_file.lock().unwrap();
                        let _ = set_winsize(f.as_raw_fd(), &ws);
                    }

                    // Evict any incumbent client (newcomer-evicts). Bump the
                    // shared generation FIRST so the TAG_WINSZ handler below
                    // (keyed off the same counter) can tell it's now stale
                    // even if its eviction-shutdown races with its own
                    // blocking read.
                    let my_gen = gen.fetch_add(1, Ordering::SeqCst) + 1;
                    if let Some(old) = current_data.lock().unwrap().take() {
                        let _ = old.shutdown(std::net::Shutdown::Both);
                        log("evicted incumbent data client");
                    }
                    if let Some(old) = current_winsz.lock().unwrap().take() {
                        let _ = old.shutdown(std::net::Shutdown::Both);
                        log("evicted incumbent winsz connection");
                    }
                    if stream.write_all(&[TAG_DATA_READY]).is_err() {
                        return;
                    }
                    *current_data.lock().unwrap() = Some(stream.try_clone().unwrap());
                    log(&format!("client ATTACHED (gen {my_gen})"));

                    // Fork the inferior now if this is the first attach.
                    if let Some(p) = pending.lock().unwrap().take() {
                        start_inferior(
                            p.inferior,
                            p.slave,
                            p.app_ctrl,
                            Arc::clone(&current_data),
                            Arc::clone(&current_winsz),
                            Arc::clone(&socket_path_arc),
                            Arc::clone(&inferior_pid),
                        );
                    }
                    // Tell the app a client attached (T5 reattach-refresh trigger).
                    let _ = writeln!(ctrl_writer, "{CTRL_ATTACH}");

                    // client -> master
                    let mut buf = [0u8; 4096];
                    loop {
                        let n = match stream.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => n,
                        };
                        let mut f = master_file.lock().unwrap();
                        if f.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                    if gen.load(Ordering::SeqCst) == my_gen {
                        *current_data.lock().unwrap() = None;
                    }
                    log(&format!("client DETACHED (gen {my_gen})"));
                }
                TAG_WINSZ => {
                    // Staleness is handled solely by newcomer-eviction
                    // shutting down the incumbent winsz stream (which makes
                    // the blocking read below error out). `my_gen` is used
                    // ONLY to guard the end-of-loop cleanup, never to break
                    // the loop -- so a capture that races the TAG_DATA gen
                    // bump can at worst skip a harmless cleanup, and can
                    // never clobber a newcomer's `current_winsz` entry.
                    let my_gen = gen.load(Ordering::SeqCst);
                    if let Some(old) = current_winsz
                        .lock()
                        .unwrap()
                        .replace(stream.try_clone().unwrap())
                    {
                        let _ = old.shutdown(std::net::Shutdown::Both);
                    }
                    let mut buf = [0u8; 8];
                    loop {
                        if stream.read_exact(&mut buf).is_err() {
                            break;
                        }
                        let ws = decode_winsize(&buf);
                        let f = master_file.lock().unwrap();
                        let _ = set_winsize(f.as_raw_fd(), &ws);
                    }
                    // Clear our slot on a normal (non-evicted) disconnect,
                    // but only if it's still ours -- a newcomer that already
                    // replaced it owns the entry now.
                    if gen.load(Ordering::SeqCst) == my_gen {
                        *current_winsz.lock().unwrap() = None;
                    }
                }
                _ => {}
            }
        });
    }
    unreachable!("UnixListener::incoming() never ends");
}

/// Fork+detach a relay for `inferior` (the full argv of the real mbv
/// invocation, argv[0] = the mbv executable path) and wait until its
/// socket is ready. Called by the ordinary launch path (`mbv -a` / bare
/// with `stay_alive` set) before becoming a terminal-client itself.
pub fn spawn_detached(socket_path: &str, inferior: Vec<String>) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.arg("--__relay")
        .arg(socket_path)
        .arg("--")
        .args(&inferior);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    // Detach into its own session (equivalent to `setsid <cmd>`) so closing
    // the launching shell can't SIGHUP the relay (belt-and-suspenders with
    // the relay's own SIGHUP-ignore in `run_relay_main`).
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(to_io)?;
            Ok(())
        });
    }
    let _child = cmd.spawn()?;

    // Wait for the relay to announce readiness by probing socket
    // connectability (never file existence — ADR 0006).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if UnixStream::connect(socket_path).is_ok() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "relay did not become ready in time",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
