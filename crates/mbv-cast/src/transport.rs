// The transport abstraction the Owner's cast worker runs against, moved
// out of the TUI so `mbv-daemon` can host the cast output too (design D1:
// the worker sender lives in the Owner's `CastOutput`).
//
// Threading model: `rust_cast::CastDevice` (which `CastClient` wraps) holds
// an `Rc<MessageManager<..>>` internally, so `CastClient` is not `Send` --
// it can never be moved to, or shared with, a thread other than the one it
// was constructed on. That rules out an `Arc<Mutex<CastClient>>` shape for
// cross-thread state. Instead, whoever connects a `CastClient` owns a
// single long-lived worker thread that constructs the client and never
// lets it leave that thread; callers only hold a `Sender<CastJob>` to that
// worker (`Send`+`Sync`+`Clone`, no `Rc` in sight). `spawn_cast_worker` is
// generic over `CastTransport` so it runs unmodified against a
// `FakeCastTransport` in tests -- constructing the (non-`Send`) transport
// inside the same closure that spawns the worker thread, rather than
// capturing an already-built one, is what keeps this compiling.

use crate::CastError;
use crate::client::{CastClient, CastMediaItem, CastStatus};
use std::sync::mpsc::Sender;

/// The cast operations a caller needs, decoupled from the concrete
/// `CastClient` so tests can substitute a recording fake instead of a live
/// receiver connection. No `Send` bound: see the threading-model note above
/// -- a `CastTransport` never crosses a thread boundary after construction.
pub trait CastTransport {
    fn load(&mut self, item: &CastMediaItem, start_position_seconds: f64) -> Result<(), CastError>;
    fn play(&mut self) -> Result<(), CastError>;
    fn pause(&mut self) -> Result<(), CastError>;
    fn stop(&mut self) -> Result<(), CastError>;
    /// Stops the launched receiver app (design D12: only a deliberate
    /// output change or detach calls it -- never mbv's own exit while
    /// attached, and never a media stop).
    fn teardown(&mut self) -> Result<(), CastError>;
    fn seek(&mut self, position_seconds: f32) -> Result<(), CastError>;
    fn set_volume(&self, level: f32) -> Result<(), CastError>;
    fn set_muted(&self, muted: bool) -> Result<(), CastError>;
    fn status(&mut self) -> Result<CastStatus, CastError>;
    fn keep_alive(&self) -> Result<(), CastError>;
}

impl CastTransport for CastClient {
    fn load(&mut self, item: &CastMediaItem, start_position_seconds: f64) -> Result<(), CastError> {
        CastClient::load(self, item, start_position_seconds)
    }
    fn play(&mut self) -> Result<(), CastError> {
        CastClient::play(self)
    }
    fn pause(&mut self) -> Result<(), CastError> {
        CastClient::pause(self)
    }
    fn stop(&mut self) -> Result<(), CastError> {
        CastClient::stop(self)
    }
    fn teardown(&mut self) -> Result<(), CastError> {
        CastClient::teardown(self)
    }
    fn seek(&mut self, position_seconds: f32) -> Result<(), CastError> {
        CastClient::seek(self, position_seconds)
    }
    fn set_volume(&self, level: f32) -> Result<(), CastError> {
        CastClient::set_volume(self, level)
    }
    fn set_muted(&self, muted: bool) -> Result<(), CastError> {
        CastClient::set_muted(self, muted)
    }
    fn status(&mut self) -> Result<CastStatus, CastError> {
        CastClient::status(self)
    }
    fn keep_alive(&self) -> Result<(), CastError> {
        CastClient::keep_alive(self)
    }
}

/// One unit of work submitted to a cast worker thread. Runs on that thread,
/// against the transport it owns, never the caller's -- see the
/// threading-model note above. Any response it needs to produce is sent
/// from inside the closure itself, since the job's return value is
/// discarded.
pub type CastJob = Box<dyn FnOnce(&mut dyn CastTransport) + Send>;

/// Spawns the worker thread that owns a live `CastTransport` for the
/// lifetime of one cast connection, and returns the `Sender` the caller
/// holds instead of the transport itself. `build` runs on the new thread,
/// not the caller's: a `CastTransport` is constructed and consumed entirely
/// within that thread and never itself needs to be `Send`, only `build`
/// (its inputs, e.g. a device address) and each `CastJob` (plain-data
/// closures) do.
///
/// `build` is fallible: connecting a real receiver can genuinely fail, and
/// `CastClient` isn't `Send` (see the threading-model note above), so the
/// connect attempt can't happen on the caller's thread and then be handed
/// off -- it must happen on this function's own new thread. This function
/// blocks the calling thread until that thread reports whether `build`
/// succeeded, so every caller that must not block its own thread invokes
/// this from a background thread of its own.
pub fn spawn_cast_worker<T, F>(build: F) -> Result<Sender<CastJob>, CastError>
where
    T: CastTransport,
    F: FnOnce() -> Result<T, CastError> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<CastJob>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), CastError>>();
    std::thread::spawn(move || match build() {
        Ok(mut transport) => {
            if ready_tx.send(Ok(())).is_err() {
                return; // Caller gave up waiting; nothing left to serve.
            }
            for job in rx {
                job(&mut transport);
            }
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e));
        }
    });
    match ready_rx.recv() {
        Ok(result) => result.map(|()| tx),
        Err(_) => Err(CastError::worker_not_ready()),
    }
}

/// A recording `CastTransport` double: no network, no live receiver. Used
/// by tests to verify which command was sent without a real Chromecast.
#[cfg(any(test, feature = "test"))]
#[derive(Debug, Default)]
pub struct FakeCastTransport {
    // Shared (not owned) so a test can keep reading `calls` after the
    // transport itself has been moved into a worker thread by
    // `spawn_fake_cast_worker` -- matching the real `CastTransport`'s
    // never-leaves-its-thread shape.
    pub calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    pub status: Option<CastStatus>,
}

#[cfg(any(test, feature = "test"))]
impl CastTransport for FakeCastTransport {
    fn load(&mut self, item: &CastMediaItem, start_position_seconds: f64) -> Result<(), CastError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("load({}, {start_position_seconds})", item.url));
        Ok(())
    }
    fn play(&mut self) -> Result<(), CastError> {
        self.calls.lock().unwrap().push("play".to_string());
        Ok(())
    }
    fn pause(&mut self) -> Result<(), CastError> {
        self.calls.lock().unwrap().push("pause".to_string());
        Ok(())
    }
    fn stop(&mut self) -> Result<(), CastError> {
        self.calls.lock().unwrap().push("stop".to_string());
        Ok(())
    }
    fn teardown(&mut self) -> Result<(), CastError> {
        self.calls.lock().unwrap().push("teardown".to_string());
        Ok(())
    }
    fn seek(&mut self, position_seconds: f32) -> Result<(), CastError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("seek({position_seconds})"));
        Ok(())
    }
    fn set_volume(&self, level: f32) -> Result<(), CastError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("set_volume({level})"));
        Ok(())
    }
    fn set_muted(&self, muted: bool) -> Result<(), CastError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("set_muted({muted})"));
        Ok(())
    }
    fn status(&mut self) -> Result<CastStatus, CastError> {
        self.status
            .clone()
            .ok_or_else(CastError::receiver_not_found)
    }
    fn keep_alive(&self) -> Result<(), CastError> {
        self.calls.lock().unwrap().push("keep_alive".to_string());
        Ok(())
    }
}

/// Spawns a worker thread owning `transport` and returns the job `Sender`
/// plus a handle to the call log, still readable from the test thread after
/// `transport` itself has moved.
///
/// # Panics
///
/// Panics if the worker thread exits before reporting readiness, which the
/// fake transport's build cannot do; the panic only fires on a broken channel.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn spawn_fake_cast_worker(
    transport: FakeCastTransport,
) -> (
    Sender<CastJob>,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    let calls = std::sync::Arc::clone(&transport.calls);
    let tx = spawn_cast_worker(move || Ok(transport)).expect("fake transport build never fails");
    (tx, calls)
}
