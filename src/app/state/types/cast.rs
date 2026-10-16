// Cast attachment state, mirroring the shape `connected_session_id` /
// `connected_session_state` give an attached Emby session (see
// `remote_slot_state.rs`).
//
// The transport abstraction, the worker thread, and the test fake live in
// `mbv_cast::transport` now (they moved there so the Owner can host the
// cast output too); App state only ever holds the worker's
// `Sender<CastJob>`, never the (non-`Send`) transport itself.

use mbv_cast::discovery::CastReceiver;
use mbv_cast::{CastError, CastJob, CastStatus};
use mbv_queue::QueueItemContentId;
use std::sync::mpsc::Sender;
use std::time::Instant;

// The progress target moved to the Owner's resolver (cast-owner-output task
// 4.1); the TUI's Cast runtime keeps using it until row 8.3 deletes that
// runtime.
pub(in crate::app) use mbv_daemon::CastProgressTarget;

/// One item dispatched to the receiver's queue: enough identity to match a
/// reported `playing_content_id` back to it (6.5) and to report its
/// provider progress (6.6).
pub(in crate::app) struct DispatchedCastItem {
    pub(in crate::app) url: String,
    pub(in crate::app) content_id: QueueItemContentId,
    pub(in crate::app) title: String,
    pub(in crate::app) report: CastProgressTarget,
}

/// mbv's attachment to a Google Cast receiver. `client` starts `None` on
/// attach (5.1) and is filled in once an async connect completes (stage 4,
/// task 8.3) -- see `App::attach_cast`/`App::set_cast_client`.
pub(in crate::app) struct CastAttachment {
    pub(in crate::app) receiver_id: String,
    pub(in crate::app) client: Option<Sender<CastJob>>,
    pub(in crate::app) dispatched: Vec<DispatchedCastItem>,
    pub(in crate::app) status: Option<CastStatus>,
    pub(in crate::app) status_at: Instant,
    /// `CastClient` exposes only `set_volume`/`set_muted`, never a getter --
    /// `rust_cast` 0.21 has no volume-status read separate from the receiver
    /// status channel this stage doesn't parse. Tracked optimistically here,
    /// the same way local playback tracks `ui_volume` while inactive.
    pub(in crate::app) volume: u8,
    pub(in crate::app) muted: bool,
    /// Set once a status poll reports a connection failure (7.6). `client`
    /// is cleared in the same step so no further poll jobs are submitted;
    /// this flag drives presenting the target as disconnected without
    /// discarding `dispatched` or touching mbv's queue.
    pub(in crate::app) disconnected: bool,
}

impl CastAttachment {
    /// Presents the target as disconnected and stops submitting poll jobs:
    /// both fields change together, so no caller can set one without the other.
    pub(in crate::app) fn mark_disconnected(&mut self) {
        self.disconnected = true;
        self.client = None;
    }
}

/// Results of background work against the attached receiver, reported back
/// to the main thread over `App::cast_tx`/`cast_rx`.
pub(in crate::app) enum CastEvent {
    Dispatched {
        receiver_id: String,
        outcome: Result<Vec<DispatchedCastItem>, CastError>,
        uncastable: Vec<(String, String)>,
    },
    StatusUpdated {
        receiver_id: String,
        status: Result<CastStatus, CastError>,
    },
    TransportError(CastError),
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
        error: CastError,
    },
    /// A background cast discovery browse (`cast_actions::spawn_cast_discovery`)
    /// completed -- possibly empty, since `browse_cast_receivers` never
    /// errors (see `cast_discovery.rs`). Drives `App::rebuild_panel_targets`
    /// (8.1/8.2).
    DiscoveryCompleted(Vec<CastReceiver>),
}
