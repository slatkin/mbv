// Cast attachment state, mirroring the shape `connected_session_id` /
// `connected_session_state` give an attached Emby session (see
// `remote_slot_state.rs`).
//
// Threading model: `rust_cast::CastDevice` (which `CastClient` wraps) holds
// an `Rc<MessageManager<..>>` internally, so `CastClient` is not `Send` --
// it can never be moved to, or shared with, a thread other than the one it
// was constructed on. That rules out the `Arc<Mutex<CastClient>>` shape used
// elsewhere in this codebase for cross-thread state (e.g. `EmbyClient`
// snapshots). Instead, whoever connects a `CastClient` (`App::connect_cast_receiver`,
// shared by task 7.3's reattach and task 8.3's attach-on-selection) owns a
// single long-lived worker thread
// that constructs the client and never lets it leave that thread; App state
// only ever holds a `Sender<CastCommand>` to that worker (`Send`+`Sync`+
// `Clone`, no `Rc` in sight). `run_cast_worker` below is the worker's command
// loop, generic over `CastTransport` so it runs unmodified against a
// `FakeCastTransport` in tests -- constructing the (non-`Send`) transport
// inside the same closure that spawns the worker thread, rather than
// capturing an already-built one, is what keeps this compiling.

use mbv_core::cast_client::{CastClient, CastMediaItem, CastStatus};
use mbv_core::cast_discovery::CastReceiver;
use mbv_core::playback_queue::QueueItemContentId;
use mbv_core::{EmbySessionId, MediaSourceId};
use std::sync::mpsc::Sender;
#[cfg(test)]
use std::sync::Arc;
use std::time::Instant;

/// The cast operations App state needs, decoupled from the concrete
/// `CastClient` so tests can substitute a recording fake instead of a live
/// receiver connection. No `Send` bound: see the threading-model note above
/// -- a `CastTransport` never crosses a thread boundary after construction.
pub(super) trait CastTransport {
    fn load_queue(&mut self, items: &[CastMediaItem], start_index: u16) -> Result<(), String>;
    fn play(&mut self) -> Result<(), String>;
    fn pause(&mut self) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    fn seek(&mut self, position_seconds: f32) -> Result<(), String>;
    fn skip_next(&mut self) -> Result<(), String>;
    fn skip_previous(&mut self) -> Result<(), String>;
    fn set_volume(&self, level: f32) -> Result<(), String>;
    fn set_muted(&self, muted: bool) -> Result<(), String>;
    fn status(&mut self) -> Result<CastStatus, String>;
    fn keep_alive(&self) -> Result<(), String>;
}

impl CastTransport for CastClient {
    fn load_queue(&mut self, items: &[CastMediaItem], start_index: u16) -> Result<(), String> {
        CastClient::load_queue(self, items, start_index)
    }
    fn play(&mut self) -> Result<(), String> {
        CastClient::play(self)
    }
    fn pause(&mut self) -> Result<(), String> {
        CastClient::pause(self)
    }
    fn stop(&mut self) -> Result<(), String> {
        CastClient::stop(self)
    }
    fn seek(&mut self, position_seconds: f32) -> Result<(), String> {
        CastClient::seek(self, position_seconds)
    }
    fn skip_next(&mut self) -> Result<(), String> {
        CastClient::skip_next(self)
    }
    fn skip_previous(&mut self) -> Result<(), String> {
        CastClient::skip_previous(self)
    }
    fn set_volume(&self, level: f32) -> Result<(), String> {
        CastClient::set_volume(self, level)
    }
    fn set_muted(&self, muted: bool) -> Result<(), String> {
        CastClient::set_muted(self, muted)
    }
    fn status(&mut self) -> Result<CastStatus, String> {
        CastClient::status(self)
    }
    fn keep_alive(&self) -> Result<(), String> {
        CastClient::keep_alive(self)
    }
}

/// Where a dispatched item's progress is reported (6.6), one variant per
/// provider `cast_dispatch` resolves items for.
pub(super) enum CastProgressTarget {
    Emby {
        item_id: String,
        media_source_id: MediaSourceId,
        session_id: EmbySessionId,
        runtime_ticks: i64,
    },
    Feed {
        feed_id: Option<String>,
        guid: String,
    },
    /// `session_id` is the Audiobookshelf playback session opened to resolve
    /// this episode's cast URL (`create_playback_session_bounded`) -- it must
    /// be opened at dispatch time regardless of progress reporting, since
    /// `cast_dispatch::resolve_audiobookshelf_episode_dispatch` requires an
    /// already-resolved `AudiobookshelfAudioSource`.
    AudiobookshelfEpisode {
        session_id: String,
        duration_seconds: f64,
    },
}

/// One item dispatched to the receiver's queue: enough identity to match a
/// reported `playing_content_id` back to it (6.5) and to report its
/// provider progress (6.6).
pub(super) struct DispatchedCastItem {
    pub(super) url: String,
    pub(super) content_id: QueueItemContentId,
    pub(super) title: String,
    pub(super) report: CastProgressTarget,
}

/// One unit of work submitted to a cast worker thread. Runs on that thread,
/// against the transport it owns, never the caller's -- see the
/// threading-model note above. Any response it needs to produce (typically a
/// `CastEvent`) is sent from inside the closure itself, since the job's
/// return value is discarded.
pub(super) type CastJob = Box<dyn FnOnce(&mut dyn CastTransport) + Send>;

/// Spawns the worker thread that owns a live `CastTransport` for the
/// lifetime of one cast attachment, and returns the `Sender` App state holds
/// instead of the transport itself. `build` runs on the new thread, not the
/// caller's: a `CastTransport` is constructed and consumed entirely within
/// that thread and never itself needs to be `Send`, only `build` (its
/// inputs, e.g. a device address) and each `CastJob` (plain-data closures)
/// do.
///
/// `build` is fallible: connecting a real receiver (7.3's reattach, task
/// 8.3's attach-on-selection) can genuinely fail, and `CastClient` isn't
/// `Send` (see the threading-model note above), so the connect attempt
/// can't happen on the caller's thread and then be handed off -- it must
/// happen on this function's own new thread. This function blocks the
/// calling thread until that thread reports whether `build` succeeded, so
/// every caller that must not block its own thread (all of them so far)
/// invokes this from a background thread of its own, the way
/// `App::connect_cast_receiver` does.
pub(super) fn spawn_cast_worker<T, F>(build: F) -> Result<Sender<CastJob>, String>
where
    T: CastTransport,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<CastJob>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
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
        Err(_) => Err("cast worker thread exited before reporting readiness".to_string()),
    }
}

/// mbv's attachment to a Google Cast receiver. `client` starts `None` on
/// attach (5.1) and is filled in once an async connect completes (stage 4,
/// task 8.3) -- see `App::attach_cast`/`App::set_cast_client`.
pub(super) struct CastAttachment {
    pub(super) receiver_id: String,
    pub(super) client: Option<Sender<CastJob>>,
    pub(super) dispatched: Vec<DispatchedCastItem>,
    pub(super) status: Option<CastStatus>,
    pub(super) status_at: Instant,
    /// `CastClient` exposes only `set_volume`/`set_muted`, never a getter --
    /// rust_cast 0.21 has no volume-status read separate from the receiver
    /// status channel this stage doesn't parse. Tracked optimistically here,
    /// the same way local playback tracks `ui_volume` while inactive.
    pub(super) volume: u8,
    pub(super) muted: bool,
    /// Set once a status poll reports a connection failure (7.6). `client`
    /// is cleared in the same step so no further poll jobs are submitted;
    /// this flag drives presenting the target as disconnected without
    /// discarding `dispatched` or touching mbv's queue.
    pub(super) disconnected: bool,
}

/// Results of background work against the attached receiver, reported back
/// to the main thread over `App::cast_tx`/`cast_rx`.
pub(super) enum CastEvent {
    Dispatched {
        receiver_id: String,
        outcome: Result<Vec<DispatchedCastItem>, String>,
        uncastable: Vec<(String, String)>,
    },
    StatusUpdated {
        receiver_id: String,
        status: Result<CastStatus, String>,
    },
    TransportError(String),
    /// A background resolve-and-connect (`App::connect_cast_receiver`)
    /// succeeded: `client` is the worker's job sender, ready for
    /// `App::attach_cast`/`set_cast_client` (7.3, and task 8.3's reuse).
    Connected {
        receiver_id: String,
        client: Sender<CastJob>,
    },
    /// A background resolve-and-connect failed or the receiver could not be
    /// found by a fresh discovery browse (7.5's "unavailable" case).
    ConnectFailed {
        receiver_id: String,
        error: String,
    },
    /// A background cast discovery browse (`cast_actions::spawn_cast_discovery`)
    /// completed -- possibly empty, since `browse_cast_receivers` never
    /// errors (see cast_discovery.rs). Drives `App::rebuild_panel_targets`
    /// (8.1/8.2).
    DiscoveryCompleted(Vec<CastReceiver>),
}

/// A recording `CastTransport` double: no network, no live receiver. Used by
/// `cast_actions`/`playback_target_cast`/`cast_status_actions` tests to
/// verify which command was sent without a real Chromecast.
#[cfg(test)]
pub(super) struct FakeCastTransport {
    // Shared (not owned) so a test can keep reading `calls` after the
    // transport itself has been moved into a worker thread by
    // `spawn_fake_cast_worker` -- matching the real `CastTransport`'s
    // never-leaves-its-thread shape.
    pub(super) calls: Arc<std::sync::Mutex<Vec<String>>>,
    pub(super) status: Result<CastStatus, String>,
}

#[cfg(test)]
impl Default for FakeCastTransport {
    fn default() -> Self {
        Self {
            calls: Arc::new(std::sync::Mutex::new(Vec::new())),
            status: Err("no status set".to_string()),
        }
    }
}

#[cfg(test)]
impl CastTransport for FakeCastTransport {
    fn load_queue(&mut self, items: &[CastMediaItem], start_index: u16) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("load_queue({}, {start_index})", items.len()));
        Ok(())
    }
    fn play(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("play".to_string());
        Ok(())
    }
    fn pause(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("pause".to_string());
        Ok(())
    }
    fn stop(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("stop".to_string());
        Ok(())
    }
    fn seek(&mut self, position_seconds: f32) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("seek({position_seconds})"));
        Ok(())
    }
    fn skip_next(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("skip_next".to_string());
        Ok(())
    }
    fn skip_previous(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("skip_previous".to_string());
        Ok(())
    }
    fn set_volume(&self, level: f32) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("set_volume({level})"));
        Ok(())
    }
    fn set_muted(&self, muted: bool) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("set_muted({muted})"));
        Ok(())
    }
    fn status(&mut self) -> Result<CastStatus, String> {
        self.status.clone()
    }
    fn keep_alive(&self) -> Result<(), String> {
        self.calls.lock().unwrap().push("keep_alive".to_string());
        Ok(())
    }
}

/// Spawns a worker thread owning `transport` and returns the job `Sender`
/// App state holds plus a handle to the call log, still readable from the
/// test thread after `transport` itself has moved.
#[cfg(test)]
pub(super) fn spawn_fake_cast_worker(
    transport: FakeCastTransport,
) -> (Sender<CastJob>, Arc<std::sync::Mutex<Vec<String>>>) {
    let calls = transport.calls.clone();
    let tx = spawn_cast_worker(move || Ok(transport)).expect("fake transport build never fails");
    (tx, calls)
}
