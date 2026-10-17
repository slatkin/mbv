//! The Emby Session the Owner plays through (emby-session-owner-output design
//! D1/D5/D6): the device identity that outlives the session id, the session
//! id the polls adopt, the connection the misses decide, the last observation
//! and its time, and the item the Owner loaded. Every `/Sessions` call for
//! this output runs on its own worker thread, in order; the job sender is the
//! only path to the device.

use std::sync::mpsc;
use std::time::Instant;

use mbv_ctrl::OutputConnection;
use mbv_emby::SessionInfo;
use mbv_queue::QueueSlotId;

use crate::event_loop::DaemonLoop;
use crate::output::advance::OutputAdvance;

pub(crate) mod connect;
pub(crate) mod poll;
pub(crate) mod worker;

#[cfg(test)]
pub(crate) mod fake;

#[cfg(test)]
mod tests;

use worker::EmbySessionJob;

/// One Emby Session worker outcome (design D4), carried to the owner's event
/// loop as `DaemonEvent::EmbySession`: the worker returns results instead of
/// the event loop waiting on the network.
#[derive(Debug)]
pub(crate) enum EmbySessionEvent {
    /// One selection's device lookup (design D4, task 3.2): the Session that
    /// matched the selected device, or the reason no Session matched it. The
    /// lookup runs on the output's worker, so its result arrives here.
    Selected {
        device_id: String,
        result: Result<SessionInfo, String>,
    },
    /// One `/Sessions` poll result (design D5), stamped with the poll id so a
    /// late result from a replaced worker cannot match the current poll. The
    /// error is the rendered `EmbyError`: a failed poll is not a missing
    /// device.
    Sessions {
        poll_id: u64,
        result: Result<Vec<SessionInfo>, String>,
    },
}

/// Builds the one worker an output owns (design D4). The Local daemon builds
/// it from the owner's Emby client at each selection; tests substitute one
/// whose `SessionApi` is a recording fake, so no test reaches the network.
pub(crate) type EmbySessionWorkerFactory = Box<dyn Fn() -> mpsc::Sender<EmbySessionJob>>;

/// Applies one worker outcome to the owner's output (design D4).
pub(crate) fn handle_event(daemon: &mut DaemonLoop, event: EmbySessionEvent) {
    match event {
        EmbySessionEvent::Selected { device_id, result } => {
            connect::handle_selected(daemon, device_id, result);
        }
        EmbySessionEvent::Sessions { poll_id, result } => {
            poll::handle_sessions(daemon, poll_id, result);
        }
    }
}

/// One Emby Session as the Owner's output (design D1). The Owner loads an item
/// by sending the device a `play` job; it never fetches the stream, so the
/// loaded item is an identity plus the runtime the end estimate is capped at
/// (design D6).
pub(crate) struct EmbySessionOutput {
    /// The only path to the device (design D4): one worker thread runs every
    /// `/Sessions` call for this output in order, and dropping the sender ends
    /// the worker.
    pub(crate) worker: mpsc::Sender<EmbySessionJob>,
    /// The device that owns the Session (design D1): it outlives the session
    /// id, so selections, poll matches, and the F3 mark key by it.
    pub(crate) device_id: String,
    /// The device's display name, as the selection and the ctrl state carry
    /// it.
    pub(crate) name: String,
    /// The device's current session id, adopted from each observation (design
    /// D5); it changes when the device app restarts.
    pub(crate) session_id: Option<String>,
    /// The media kinds the Session advertises as playable (design D1). An
    /// empty list is the Session's unknown capability, not an audio-only
    /// Session (spec "Playback admission").
    pub(crate) playable_media_types: Vec<String>,
    /// The connection the polls report (design D5).
    pub(crate) connection: OutputConnection,
    /// Consecutive polls that did not find the device; three of them set the
    /// connection to `Disconnected` (design D5).
    pub(crate) misses: u8,
    /// The poll in flight, by id and submission time (design D5), so a late
    /// result from a replaced worker cannot match the current poll.
    pub(crate) poll_sent: Option<(u64, Instant)>,
    /// When the last poll was submitted; it paces the 1 s cadence while no
    /// poll is in flight (design D5).
    pub(crate) last_poll: Option<Instant>,
    /// Mints the monotonic id of the next poll (design D5).
    pub(crate) next_poll_id: u64,
    /// The last `/Sessions` entry for this device and when it was observed
    /// (design D5); the position estimate, the end table, and the status
    /// projection read it.
    pub(crate) last_observation: Option<SessionInfo>,
    /// When `last_observation` was taken; the estimate adds the elapsed time
    /// while the Session plays (design D6).
    pub(crate) observed_at: Option<Instant>,
    /// The item the Owner loaded onto the device (design D1).
    pub(crate) loaded: Option<EmbySessionLoadedItem>,
    /// A pause the hand-off could not start with (design D9): the first
    /// observation that shows the loaded item sends `transport("Pause")`.
    pub(crate) pending_pause: bool,
    /// The shared advance a natural end started (design D6). It lives on the
    /// output, so a stop, an output change, or a replacing load ends it with
    /// the output.
    pub(crate) advance: Option<OutputAdvance>,
}

impl EmbySessionOutput {
    /// Whether the Session advertises audio as its only playable media kind
    /// (ctrl `OutputState::EmbySession::audio_only`, design D3). An empty list
    /// is the Session's unknown capability, which counts as able to play every
    /// Emby item, so it is not audio-only.
    pub(crate) fn audio_only(&self) -> bool {
        !self.playable_media_types.is_empty()
            && self
                .playable_media_types
                .iter()
                .all(|kind| kind.eq_ignore_ascii_case("audio"))
    }
}

impl std::fmt::Debug for EmbySessionOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbySessionOutput")
            .field("worker", &self.worker)
            .field("device_id", &self.device_id)
            .field("name", &self.name)
            .field("session_id", &self.session_id)
            .field("playable_media_types", &self.playable_media_types)
            .field("connection", &self.connection)
            .field("misses", &self.misses)
            .field("poll_sent", &self.poll_sent)
            .field("last_poll", &self.last_poll)
            .field("next_poll_id", &self.next_poll_id)
            .field("last_observation", &self.last_observation)
            .field("observed_at", &self.observed_at)
            .field("loaded", &self.loaded)
            .field("pending_pause", &self.pending_pause)
            .field("advance", &self.advance)
            .finish()
    }
}

/// The item the Owner loaded onto an Emby Session (design D1, D6). Emby
/// reports no end reason, so the Owner infers both the end and a load that
/// never started: `seen_playing` separates the two, `end_handled` keeps one
/// end from advancing twice, and `failure_reported` names a load the Session
/// never showed exactly once.
pub(crate) struct EmbySessionLoadedItem {
    /// The queue slot this item is the occurrence of.
    pub(crate) slot: QueueSlotId,
    /// The Emby item id the device was told to play.
    pub(crate) item_id: String,
    /// The loaded slot's runtime in ticks (design D6); the end estimate is
    /// capped at it, and the Session's own runtime stands in when it is absent.
    pub(crate) runtime_ticks: i64,
    /// When the play job was sent: the 30 s failure timeout counts from it.
    pub(crate) loaded_at: Instant,
    /// Whether any observation has presented this item yet (design D6).
    pub(crate) seen_playing: bool,
    /// Whether the inferred end of this item was already acted on (design D6).
    pub(crate) end_handled: bool,
    /// Whether the load that never started was already reported (design D6).
    pub(crate) failure_reported: bool,
}

impl std::fmt::Debug for EmbySessionLoadedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbySessionLoadedItem")
            .field("slot", &self.slot)
            .field("item_id", &self.item_id)
            .field("runtime_ticks", &self.runtime_ticks)
            .field("loaded_at", &self.loaded_at)
            .field("seen_playing", &self.seen_playing)
            .field("end_handled", &self.end_handled)
            .field("failure_reported", &self.failure_reported)
            .finish()
    }
}
