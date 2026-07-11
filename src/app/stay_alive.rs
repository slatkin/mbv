//! App-side half of the relay's out-of-band control channel (ADR 0005).
//!
//! When mbv is running as the inferior under a stay-alive relay, the relay
//! hands it a control-channel fd (see `MBV_STAYALIVE_CTRL_FD` /
//! `crate::relay`) carrying two messages: `client attached` (relay -> app,
//! on every attach) and `detach now` (app -> relay, sent here on `q`).

use std::io::{BufRead, Write};
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Set by the control-channel reader thread on every `ATTACH` line; polled
/// (and cleared) by the run loop to fire the reattach-refresh (T5).
static ATTACH_PENDING: AtomicBool = AtomicBool::new(false);

/// Handle to the app's end of the relay control channel. Present only when
/// mbv is running as a stay-alive inferior (i.e. under a relay); `None` in
/// bare mode.
pub struct StayAliveCtrl {
    writer: Arc<Mutex<UnixStream>>,
}

impl StayAliveCtrl {
    /// If `MBV_STAYALIVE_CTRL_FD` is set (the relay handed us a control
    /// fd), take it over: spawn a reader thread that marks
    /// `attach_pending` on every `ATTACH` line, and return a handle that
    /// can send `DETACH` back on `q`. Returns `None` in bare mode.
    pub fn from_env() -> Option<Self> {
        let fd: i32 = std::env::var(crate::relay::CTRL_FD_ENV)
            .ok()?
            .parse()
            .ok()?;
        // SAFETY: the relay dup2'd its app_ctrl socketpair end onto this fd
        // before exec, per `relay::start_inferior`; it is ours to own.
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        let reader = stream.try_clone().ok()?;
        std::thread::spawn(move || {
            let buf = std::io::BufReader::new(reader);
            for line in buf.lines() {
                let Ok(line) = line else { break };
                if line.trim() == crate::relay::CTRL_ATTACH {
                    ATTACH_PENDING.store(true, Ordering::SeqCst);
                    log::info!(target: "stay_alive", "client attached (control channel)");
                }
            }
        });
        Some(Self {
            writer: Arc::new(Mutex::new(stream)),
        })
    }

    /// Send `detach now` to the relay: it closes the current terminal-client
    /// connection (which restores that terminal) but keeps serving the pty
    /// — the app itself must NOT stop the player or exit its run loop.
    ///
    /// Returns an error if the write to the control channel fails (e.g. the
    /// relay's ctrl-reader thread has already died and the socket is
    /// wedged/closed) so callers can tell the user detach didn't actually
    /// happen, instead of claiming success unconditionally.
    pub fn send_detach(&self) -> std::io::Result<()> {
        let mut w = self.writer.lock().unwrap();
        writeln!(w, "{}", crate::relay::CTRL_DETACH)
    }

    /// True at most once per `ATTACH` line received; clears on read.
    pub fn take_attach_pending() -> bool {
        ATTACH_PENDING.swap(false, Ordering::SeqCst)
    }
}
