use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use crate::api::{EmbyClient, MediaItem};
use crate::ctrl::{CtrlCmd, CtrlCompatibility, PlaybackIntent, WireCommand};
use crate::playback_queue::{QueueRevision, QueueSlotId};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

#[derive(Clone)]
pub struct RemotePlayer {
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<crate::player::SubtitlePrefs>>,
    pub items: Arc<Mutex<Vec<MediaItem>>>,
    pub queue_source: Arc<Mutex<crate::config::QueueSource>>,
    pub(crate) cmd_tx: mpsc::Sender<CtrlCmd>,
    pub(crate) disconnected: Arc<AtomicBool>,
    /// Daemon-assigned slot IDs, parallel to `items`. Populated from v8
    /// CtrlState snapshots; empty for v7 peers.
    pub(crate) slot_ids: Arc<Mutex<Vec<QueueSlotId>>>,
    /// Daemon queue revision from the last-received CtrlState snapshot.
    pub(crate) queue_revision: Arc<Mutex<QueueRevision>>,
    /// True while a structural mutation (remove/move) has been sent but
    /// not yet acknowledged by a fresh snapshot from the daemon.
    pub(crate) pending_mutation: Arc<AtomicBool>,
    /// Set when the connection closed after the daemon announced a
    /// deliberate shutdown, as opposed to closing with no warning.
    pub(crate) shutdown_announced: Arc<AtomicBool>,
    pub(crate) ctrl_compatibility: CtrlCompatibility,
    /// A kept clone of the control socket, used only by `disconnect()`
    /// (#233) to shut the connection down on demand rather than relying
    /// on `Drop` -- which only closes this clone's own fd duplicate, not
    /// the reader/writer threads' separate duplicates of the same
    /// underlying socket. `Arc<Mutex<..>>` so every `RemotePlayer` clone
    /// shares one handle and `disconnect()` is safe to call from any of
    /// them; `Option` so a second call is a no-op instead of a double
    /// shutdown.
    pub(crate) control_stream: Arc<Mutex<Option<ControlStream>>>,
    pub(crate) next_playback_id: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) pending_playback: Arc<Mutex<HashMap<u64, PlaybackIntent>>>,
}

pub(crate) use crate::remote_player_connect::ControlStream;
pub use crate::remote_player_connect::DaemonEndpoint;

impl RemotePlayer {
    pub fn connect_endpoint(
        endpoint: &DaemonEndpoint,
        auth_token: &str,
    ) -> Result<(Self, mpsc::Receiver<PlayerEvent>), String> {
        super::remote_player_connect::connect_endpoint(endpoint, auth_token)
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    /// Whether the connection closed after the daemon announced a deliberate
    /// shutdown, as opposed to closing with no warning. Only meaningful once
    /// `is_disconnected()` is true.
    pub fn is_shutdown_announced(&self) -> bool {
        self.shutdown_announced.load(Ordering::SeqCst)
    }

    /// Shared handle to the disconnect flag, cloneable independent of the
    /// rest of `RemotePlayer` (#160): the root `mbv` crate's MPRIS polling
    /// loop holds this alongside `status` so it can stop advertising a
    /// live/active track the moment the daemon connection drops, even
    /// though `status` itself isn't guaranteed to be updated synchronously
    /// with the disconnect (an "expected" disconnect, e.g. an Emby Remote
    /// takeover, never sends a `Stopped` event -- see the reader thread in
    /// `connect_endpoint`).
    pub fn disconnected_flag(&self) -> Arc<AtomicBool> {
        self.disconnected.clone()
    }

    pub fn send_ctrl_cmd(&self, cmd: CtrlCmd) -> bool {
        self.cmd_tx.send(cmd).is_ok()
    }

    /// Send a guarded playback intent through its dedicated protocol
    /// envelope. There is deliberately no conversion to `PlayerCmd` here:
    /// callers that need lifecycle correlation must use this boundary.
    pub fn send_playback_intent(&self, intent: PlaybackIntent) -> bool {
        let request_id = intent.request_id;
        self.pending_playback
            .lock()
            .unwrap()
            .insert(request_id, intent.clone());
        if self.cmd_tx.send(CtrlCmd::PlaybackIntent(intent)).is_ok() {
            true
        } else {
            self.pending_playback.lock().unwrap().remove(&request_id);
            false
        }
    }

    pub fn new_playback_intent(&self, action: crate::ctrl::PlaybackIntentAction) -> PlaybackIntent {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed);
        PlaybackIntent {
            request_id: id,
            generation: id,
            action,
        }
    }

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        let wire_cmd = match cmd {
            PlayerCommand::QueueAppend { items }
                if !self.ctrl_compatibility.supports_queue_append =>
            {
                log::warn!(
                    target: "remote",
                    "remote ctrl peer protocol v{} does not support QueueAppend for {} item(s)",
                    self.ctrl_compatibility.peer_protocol_version,
                    items.len()
                );
                return false;
            }
            // ── v8 slot-aware translation ───────────────────────────
            // Structural mutations (remove/move) are gated by the one-
            // in-flight policy (task 6.4). JumpTo is not structural.
            _ if self.ctrl_compatibility.peer_protocol_version >= 8 => {
                match cmd {
                    PlayerCommand::QueueRemove(idx) => {
                        let wire = {
                            let slot_ids = self.slot_ids.lock().unwrap();
                            let revision = self.queue_revision.lock().unwrap();
                            match slot_ids.get(idx) {
                                Some(&slot_id) if revision.0 > 0 => {
                                    Some(WireCommand::QueueRemoveBySlot { slot_id, revision: *revision })
                                }
                                _ => {
                                    log::warn!(target: "remote", "v8 QueueRemove: no slot at idx={idx} or revision=0");
                                    None
                                }
                            }
                        };
                        match wire {
                            Some(w) => {
                                // One-in-flight check
                                if self.pending_mutation.swap(true, Ordering::SeqCst) {
                                    log::debug!(target: "remote", "v8 QueueRemove dropped: prior mutation in flight");
                                    return false;
                                }
                                if self.cmd_tx.send(CtrlCmd::PlayerCmd(w)).is_err() {
                                    self.pending_mutation.store(false, Ordering::SeqCst);
                                    return false;
                                }
                                return true;
                            }
                            None => return false,
                        }
                    }
                    PlayerCommand::QueueMove(from, to) => {
                        let wire = {
                            let slot_ids = self.slot_ids.lock().unwrap();
                            let revision = self.queue_revision.lock().unwrap();
                            match slot_ids.get(from) {
                                Some(&slot_id) if revision.0 > 0 => {
                                    Some(WireCommand::QueueMoveBySlot {
                                        slot_id,
                                        to_position: to,
                                        revision: *revision,
                                    })
                                }
                                _ => {
                                    log::warn!(target: "remote", "v8 QueueMove: no slot at idx={from} or revision=0");
                                    None
                                }
                            }
                        };
                        match wire {
                            Some(w) => {
                                if self.pending_mutation.swap(true, Ordering::SeqCst) {
                                    log::debug!(target: "remote", "v8 QueueMove dropped: prior mutation in flight");
                                    return false;
                                }
                                if self.cmd_tx.send(CtrlCmd::PlayerCmd(w)).is_err() {
                                    self.pending_mutation.store(false, Ordering::SeqCst);
                                    return false;
                                }
                                return true;
                            }
                            None => return false,
                        }
                    }
                    PlayerCommand::JumpTo(idx) => {
                        let slot_ids = self.slot_ids.lock().unwrap();
                        match slot_ids.get(idx) {
                            Some(&slot_id) => {
                                WireCommand::JumpToSlot { slot_id }
                            }
                            _ => {
                                log::warn!(target: "remote", "v8 JumpTo: no slot at idx={idx}");
                                return false;
                            }
                        }
                    }
                    PlayerCommand::ReplaceQueue { items: _, start_idx: _ }
                        if !self.ctrl_compatibility.supports_queue_append =>
                    {
                        log::warn!(target: "remote", "v8 ReplaceQueue not supported");
                        return false;
                    }
                    other => other.into(),
                }
            }
            cmd => cmd.into(),
        };
        self.cmd_tx.send(CtrlCmd::PlayerCmd(wire_cmd)).is_ok()
    }

    /// Send QueueRemoveActive to the daemon (task 7.1). The daemon
    /// transactionally removes the active slot and drives async finalization.
    pub fn send_queue_remove_active(&self, revision: QueueRevision) -> bool {
        if revision.0 == 0 {
            return false;
        }
        if self.pending_mutation.swap(true, Ordering::SeqCst) {
            log::debug!(target: "remote", "QueueRemoveActive dropped: prior mutation in flight");
            return false;
        }
        if self.cmd_tx.send(CtrlCmd::PlayerCmd(WireCommand::QueueRemoveActive { revision })).is_err() {
            self.pending_mutation.store(false, Ordering::SeqCst);
            false
        } else {
            true
        }
    }

    /// Send QueueInsertAt to the daemon for undo restoration (task 9.3).
    /// The daemon inserts the item at the given position and broadcasts
    /// the updated snapshot with a newly assigned slot ID.
    pub fn send_queue_insert_at(
        &self,
        item: MediaItem,
        position: usize,
        revision: QueueRevision,
    ) -> bool {
        if revision.0 == 0 {
            return false;
        }
        if self.pending_mutation.swap(true, Ordering::SeqCst) {
            log::debug!(target: "remote", "QueueInsertAt dropped: prior mutation in flight");
            return false;
        }
        if self.cmd_tx.send(CtrlCmd::PlayerCmd(WireCommand::QueueInsertAt { item, position, revision })).is_err() {
            self.pending_mutation.store(false, Ordering::SeqCst);
            false
        } else {
            true
        }
    }

    /// Called from apply_ctrl_event when a full v8 snapshot arrives;
    /// clears the in-flight flag so the next mutation can proceed.
    pub(crate) fn clear_pending_mutation(&self) {
        self.pending_mutation.store(false, Ordering::SeqCst);
    }

    /// Populate slot_ids and revision from a client-side PlayerTab
    /// (used by tests and bootstrap to seed the translation layer).
    pub(crate) fn set_slot_state(
        &self,
        slot_ids: Vec<QueueSlotId>,
        revision: QueueRevision,
    ) {
        *self.slot_ids.lock().unwrap() = slot_ids;
        *self.queue_revision.lock().unwrap() = revision;
    }

    pub fn adopt_queue(
        &self,
        items: Vec<MediaItem>,
        cursor: usize,
        source: crate::config::QueueSource,
    ) -> bool {
        let cursor = cursor.min(items.len().saturating_sub(1));
        {
            let mut status = self.status.lock().unwrap();
            status.current_idx = cursor;
            status.queue_len = items.len();
            status.active = false;
        }
        *self.items.lock().unwrap() = items.clone();
        *self.queue_source.lock().unwrap() = source.clone();
        self.cmd_tx
            .send(CtrlCmd::AdoptQueue {
                items,
                cursor,
                source,
            })
            .is_ok()
    }

    pub fn play(
        &self,
        item: &MediaItem,
        source: crate::config::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) {
        let _ = self.send_playback_intent(self.new_playback_intent(
            crate::ctrl::PlaybackIntentAction::Play {
                item_ids: vec![item.id.clone()],
                start_idx: 0,
                start_ticks: item.playback_position_ticks,
                source: source.clone(),
            },
        ));
        *self.items.lock().unwrap() = vec![item.clone()];
        *self.queue_source.lock().unwrap() = source;
    }

    pub fn play_queue(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) {
        let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        let start_ticks = items
            .get(start_idx)
            .map_or(0, |i| i.playback_position_ticks);
        let _ = self.send_playback_intent(self.new_playback_intent(
            crate::ctrl::PlaybackIntentAction::Play {
                item_ids,
                start_idx,
                start_ticks,
                source: source.clone(),
            },
        ));
        *self.items.lock().unwrap() = items;
        *self.queue_source.lock().unwrap() = source;
    }

    pub fn stop(&self) {
        let _ = self.send_playback_intent(
            self.new_playback_intent(crate::ctrl::PlaybackIntentAction::Stop),
        );
    }

    pub fn join(&self) {
        // No thread to join; daemon keeps running when TUI exits.
    }

    /// Actively tears down the control-socket connection (#233): shuts
    /// down the shared underlying socket so the reader thread's blocking
    /// `read()` (inside `reader.lines()` in `connect_endpoint`) observes
    /// EOF/an error and exits, instead of leaking forever the way it did
    /// when the only teardown was an implicit `Drop` of one fd duplicate.
    /// Idempotent: the stored handle is taken out on first use, so a
    /// second call is a no-op rather than a double `shutdown()`.
    pub fn disconnect(&self) {
        if let Some(stream) = self.control_stream.lock().unwrap().take() {
            if let Err(e) = stream.shutdown() {
                log::warn!(target: "remote", "control-socket shutdown failed: {e}");
            }
        }
    }

    pub fn supports_queue_append(&self) -> bool {
        self.ctrl_compatibility.supports_queue_append
    }

    pub(crate) fn stub_status(current_idx: usize, queue_len: usize) -> PlayerStatus {
        PlayerStatus {
            current_idx,
            queue_len,
            active: true,
            ..Default::default()
        }
    }

    /// Test helper for root-crate integration tests that need a remote-player
    /// stand-in without a live daemon connection.
    pub fn stub(items: Vec<MediaItem>, current_idx: usize) -> (Self, mpsc::Receiver<PlayerEvent>) {
        let (remote, event_rx, _cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        (remote, event_rx)
    }

    /// Test helper variant that also exposes commands sent to the daemon.
    pub fn stub_with_command_rx(
        items: Vec<MediaItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let queue_len = items.len();
        let status = Arc::new(Mutex::new(Self::stub_status(current_idx, queue_len)));
        let subtitle_prefs = Arc::new(Mutex::new(crate::player::SubtitlePrefs::default()));
        let items = Arc::new(Mutex::new(items));
        let queue_source = Arc::new(Mutex::new(crate::config::QueueSource::Unknown));
        let disconnected = Arc::new(AtomicBool::new(false));
        let shutdown_announced = Arc::new(AtomicBool::new(false));
        let next_playback_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let pending_playback = Arc::new(Mutex::new(HashMap::new()));
        let slot_ids = Arc::new(Mutex::new(Vec::new()));
        let queue_revision = Arc::new(Mutex::new(QueueRevision::default()));
        let pending_mutation = Arc::new(AtomicBool::new(false));
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();
        let (_event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let compat = CtrlCompatibility::current();
        (
            RemotePlayer {
                status,
                subtitle_prefs,
                items,
                queue_source,
                cmd_tx,
                disconnected,
                shutdown_announced,
                slot_ids,
                queue_revision,
                pending_mutation,
                ctrl_compatibility: compat,
                control_stream: Arc::new(Mutex::new(None)),
                next_playback_id,
                pending_playback,
            },
            event_rx,
            cmd_rx,
        )
    }
}
