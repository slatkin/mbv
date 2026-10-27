//! The Emby Session job worker (emby-session-owner-output design D4): one
//! thread owns the `SessionApi` for the lifetime of one output and runs every
//! `/Sessions` call in the order the Owner submits it. Results re-enter the
//! owner's event loop as `DaemonEvent::EmbySession`, so the event loop never
//! waits on HTTP and commands reach the device in the order the user sent
//! them.

use std::sync::mpsc;

use mbv_emby::{EmbyClient, SessionInfo};
use mbv_emby_model::EmbyItem;

/// The `/Sessions` operations the worker needs, decoupled from the concrete
/// `EmbyClient` so tests can substitute a recording fake (design D4). No
/// method reports progress, so design D8 holds by construction.
///
/// Errors cross the worker thread as rendered strings: the owner's events
/// carry a client-facing message, as the Cast worker's do.
pub(crate) trait SessionApi {
    /// Every Session the server lists, unfiltered (design D4): the filtered
    /// list would drop a device that is idle but still connected (#236).
    fn get_sessions(&self) -> Result<Vec<SessionInfo>, String>;
    /// Loads `item_ids` on the Session as one list, in that order, starting at
    /// the first id from `start_ticks` (design D14).
    fn play(&self, session_id: &str, item_ids: &[&str], start_ticks: i64) -> Result<(), String>;
    /// Appends `item_ids` to the Session's queue without interrupting the item
    /// it plays now (design D14, D15).
    fn play_last(&self, session_id: &str, item_ids: &[&str]) -> Result<(), String>;
    /// Sends one `/Playing/{command}` transport command.
    fn transport(&self, session_id: &str, command: &str) -> Result<(), String>;
    fn seek(&self, session_id: &str, ticks: i64) -> Result<(), String>;
    fn set_volume(&self, session_id: &str, volume: i64) -> Result<(), String>;
    fn set_mute(&self, session_id: &str, muted: bool) -> Result<(), String>;
    fn set_audio_index(&self, session_id: &str, index: i64) -> Result<(), String>;
    fn set_subtitle_index(&self, session_id: &str, index: i64) -> Result<(), String>;
    /// Looks one Emby item up by id (emby-session-output-followups design
    /// D4): the queue-end continuation's lookup runs on the output's worker
    /// like every other call.
    fn get_item(&self, item_id: &str) -> Result<EmbyItem, String>;
}

impl SessionApi for EmbyClient {
    fn get_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_unfiltered()
            .map_err(|error| error.to_string())
    }

    fn play(&self, session_id: &str, item_ids: &[&str], start_ticks: i64) -> Result<(), String> {
        self.session_play(session_id, item_ids, start_ticks)
            .map_err(|error| error.to_string())
    }

    fn play_last(&self, session_id: &str, item_ids: &[&str]) -> Result<(), String> {
        self.session_play_last(session_id, item_ids)
            .map_err(|error| error.to_string())
    }

    fn transport(&self, session_id: &str, command: &str) -> Result<(), String> {
        self.session_transport(session_id, command)
            .map_err(|error| error.to_string())
    }

    fn seek(&self, session_id: &str, ticks: i64) -> Result<(), String> {
        self.session_seek(session_id, ticks)
            .map_err(|error| error.to_string())
    }

    fn set_volume(&self, session_id: &str, volume: i64) -> Result<(), String> {
        self.session_set_volume(session_id, volume)
            .map_err(|error| error.to_string())
    }

    fn set_mute(&self, session_id: &str, muted: bool) -> Result<(), String> {
        self.session_set_mute(session_id, muted)
            .map_err(|error| error.to_string())
    }

    fn set_audio_index(&self, session_id: &str, index: i64) -> Result<(), String> {
        self.session_set_audio_index(session_id, index)
            .map_err(|error| error.to_string())
    }

    fn set_subtitle_index(&self, session_id: &str, index: i64) -> Result<(), String> {
        self.session_set_subtitle_index(session_id, index)
            .map_err(|error| error.to_string())
    }

    fn get_item(&self, item_id: &str) -> Result<EmbyItem, String> {
        let mut items = self
            .get_items_by_ids(&[item_id.to_owned()])
            .map_err(|error| error.to_string())?;
        items
            .pop()
            .ok_or_else(|| format!("the server returned no item for {item_id}"))
    }
}

/// One unit of work for the worker thread: it runs on the thread with the
/// `SessionApi` in hand and sends its outcome through the captured event
/// sender. The closure is `Send`; the API itself never leaves its thread.
pub(crate) type EmbySessionJob = Box<dyn FnOnce(&dyn SessionApi) + Send>;

/// Wraps one job so it acks on `ack` after it ran (design D15): Owner exit
/// submits the queue hand-over and waits on a `sync_channel(0)` within the
/// shutdown deadline, and that wait also covers every job queued ahead of it. A
/// receiver that already gave up drops the channel, so the send fails instead
/// of stalling the worker.
pub(crate) fn acknowledged(ack: mpsc::SyncSender<()>, job: EmbySessionJob) -> EmbySessionJob {
    Box::new(move |api: &dyn SessionApi| {
        job(api);
        let _ = ack.send(());
    })
}

/// Spawns the worker thread that owns the `SessionApi` for one output, and
/// returns the `Sender` the output holds instead of the API (design D4).
/// `build` runs on the new thread, so the API never needs to be `Send`; each
/// submitted job does, because it crosses the channel.
pub(crate) fn spawn_emby_session_worker<T, F>(build: F) -> mpsc::Sender<EmbySessionJob>
where
    T: SessionApi + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<EmbySessionJob>();
    std::thread::spawn(move || {
        let api = build();
        for job in rx {
            job(&api);
        }
    });
    tx
}
