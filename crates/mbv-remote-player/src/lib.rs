use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use mbv_ctrl::player::{PlayerCommand, PlayerEvent, PlayerStatus};
use mbv_ctrl::{CtrlCmd, CtrlCompatibility, PlaybackIntent, QueueOpId, WireCommand};
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_queue::{QueueItem, QueueLineage, QueueSource};

/// An owner-authoritative queue operation sent over ctrl.
#[derive(Clone, Debug)]
pub enum QueueOp {
    Replace {
        items: Vec<mbv_ctrl::UnifiedQueueSlot>,
        start_idx: Option<usize>,
        source: QueueSource,
    },
    Append {
        items: Vec<QueueItem>,
        before: Option<u64>,
    },
    RemoveSlot {
        slot_id: u64,
    },
    RemoveSlots {
        slot_ids: Vec<u64>,
    },
    MoveSlot {
        slot_id: u64,
        to_index: usize,
    },
    PlaySlot {
        slot_id: u64,
    },
    Clear,
    SourceUpdate {
        source: QueueSource,
        lineage: QueueLineage,
    },
    Refresh,
    ApplyProgress {
        updates: Vec<mbv_ctrl::ProgressUpdate>,
    },
}

/// Response from a bounded shutdown request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShutdownResponse {
    /// The daemon accepted the request after persisting its queue.
    Accepted,
    /// The daemon rejected the request (e.g. TCP transport, persistence failure).
    Rejected { reason: String },
    /// The connection closed before a response arrived.
    Disconnected,
    /// The bounded wait timed out without receiving a response.
    TimedOut,
    /// The peer daemon does not advertise the lifecycle-shutdown capability.
    Unsupported,
}

#[derive(Clone, Debug)]
pub struct RemotePlayer {
    status: Arc<Mutex<PlayerStatus>>,
    subtitle_prefs: Arc<Mutex<mbv_ctrl::player::SubtitlePrefs>>,
    unified_queue: Arc<Mutex<Option<mbv_ctrl::UnifiedQueueStateData>>>,
    pub(crate) cmd_tx: mpsc::Sender<CtrlCmd>,
    pub(crate) disconnected: Arc<AtomicBool>,
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
    pub(crate) control_stream: Arc<Mutex<Option<SocketStream>>>,
    pub(crate) next_playback_id: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) next_queue_op_id: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) pending_playback: Arc<Mutex<HashMap<u64, PlaybackIntent>>>,
    /// Completer for a pending shutdown request.
    pub(crate) shutdown_request_tx: Arc<Mutex<Option<mpsc::Sender<ShutdownResponse>>>>,
}

pub(crate) mod connect;
mod error;
pub use error::RemotePlayerError;

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test"))]
pub use connect::connect_stub_daemon_pair;
pub use connect::signal_local_daemon_service_setup;
pub use connect::{DaemonEndpoint, resolve_library_route};
pub(crate) use mbv_net::stream::SocketStream;

impl RemotePlayer {
    pub fn connect_endpoint(
        endpoint: &DaemonEndpoint,
    ) -> Result<(Self, mpsc::Receiver<PlayerEvent>), RemotePlayerError> {
        connect::connect_endpoint(endpoint)
    }

    #[must_use]
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    /// Whether the connection closed after the daemon announced a deliberate
    /// shutdown, as opposed to closing with no warning. Only meaningful once
    /// `is_disconnected()` is true.
    #[must_use]
    pub fn is_shutdown_announced(&self) -> bool {
        self.shutdown_announced.load(Ordering::SeqCst)
    }

    /// Test-support seam: force the disconnect flag for stubs that must
    /// present a disconnected remote without a live daemon connection.
    #[cfg(any(test, feature = "test"))]
    pub fn set_disconnected_for_test(&self, value: bool) {
        self.disconnected.store(value, Ordering::SeqCst);
    }

    #[must_use]
    pub fn send_ctrl_cmd(&self, cmd: CtrlCmd) -> bool {
        !self.is_disconnected() && self.cmd_tx.send(cmd).is_ok()
    }

    /// Bounded lifecycle shutdown request.
    ///
    /// Sends `RequestShutdown` and waits for the daemon's response with a
    /// bounded timeout. Returns `Accepted` only when the daemon has durably
    /// persisted its queue and acknowledged the request; enqueue success
    /// alone is never returned as `Accepted`.
    ///
    /// # Panics
    ///
    /// Panics if the `shutdown_request_tx` mutex is poisoned: a previous owner
    /// panicked while holding it. The early `Unsupported` return takes no lock.
    #[must_use]
    pub fn request_shutdown(&self, timeout: Duration) -> ShutdownResponse {
        if !self.supports_lifecycle_shutdown() {
            return ShutdownResponse::Unsupported;
        }

        let (response_tx, response_rx) = mpsc::channel();

        // Register the completer before sending the command so the reader
        // thread can resolve it immediately when the response arrives.
        {
            let mut guard = self.shutdown_request_tx.lock().unwrap();
            if guard.is_some() {
                // Another request is already in flight; reject immediately.
                return ShutdownResponse::Rejected {
                    reason: "shutdown request already in flight".to_string(),
                };
            }
            *guard = Some(response_tx);
        };

        // Send the request.
        if self.cmd_tx.send(CtrlCmd::RequestShutdown).is_err() {
            // Channel closed; daemon is disconnected.
            let mut guard = self.shutdown_request_tx.lock().unwrap();
            *guard = None;
            return ShutdownResponse::Disconnected;
        }

        // Wait for the response with the bounded timeout.
        match response_rx.recv_timeout(timeout) {
            Ok(response) => response,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut guard = self.shutdown_request_tx.lock().unwrap();
                *guard = None;
                ShutdownResponse::TimedOut
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread dropped the sender (disconnect).
                let mut guard = self.shutdown_request_tx.lock().unwrap();
                *guard = None;
                ShutdownResponse::Disconnected
            }
        }
    }

    /// Send a guarded playback intent through its dedicated protocol
    /// envelope. There is deliberately no conversion to `PlayerCmd` here:
    /// callers that need lifecycle correlation must use this boundary.
    ///
    /// # Panics
    ///
    /// Panics if the `pending_playback` mutex is poisoned: a previous owner
    /// panicked while holding it.
    #[must_use]
    pub fn send_playback_intent(&self, intent: PlaybackIntent) -> bool {
        let request_id = intent.request_id;
        let generation = intent.generation;
        self.pending_playback
            .lock()
            .unwrap()
            .insert(request_id, intent.clone());
        if self.cmd_tx.send(CtrlCmd::PlaybackIntent(intent)).is_ok() {
            tracing::info!(
                name: "ctrl.intent.sent",
                target: "ctrl",
                request = request_id,
                generation = generation,
                "playback intent sent"
            );
            true
        } else {
            self.pending_playback.lock().unwrap().remove(&request_id);
            false
        }
    }

    #[must_use]
    pub fn new_playback_intent(&self, action: mbv_ctrl::PlaybackIntentAction) -> PlaybackIntent {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed);
        PlaybackIntent {
            request_id: id,
            generation: id,
            action,
        }
    }

    /// Dispatch a transport command (mpris/tray) to whichever protocol
    /// path it belongs on: `Step` becomes a guarded playback intent,
    /// everything else goes over the legacy command channel.
    pub fn send_transport(&self, transport: mbv_ctrl::TransportCommand) {
        match transport {
            mbv_ctrl::TransportCommand::Step(direction) => {
                let action = match direction {
                    mbv_ctrl::Direction::Next => mbv_ctrl::PlaybackIntentAction::Next,
                    mbv_ctrl::Direction::Previous => mbv_ctrl::PlaybackIntentAction::Previous,
                };
                let _ = self.send_playback_intent(self.new_playback_intent(action));
            }
            mbv_ctrl::TransportCommand::Player(command) => {
                let _ = self.send_command(command);
            }
        }
    }

    #[must_use]
    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        let wire_cmd = match cmd {
            // Queue mutation has no legacy wire form; it crosses ctrl
            // exclusively as `CtrlCmd::UnifiedQueue*`. Callers use the unified
            // path (`RemotePlayer::send_queue_op`, `PlayerProxy::submit_queue_slots`).
            PlayerCommand::QueueAppend { .. }
            | PlayerCommand::QueueRemove(_)
            | PlayerCommand::QueueMove(..) => {
                tracing::warn!(name: "remote.command_send.refused", target: "remote", "queue mutation not sendable over legacy ctrl; use a unified queue command");
                return false;
            }
            cmd => match WireCommand::try_from_player_command(cmd) {
                Ok(wire) => wire,
                Err(refused) => {
                    tracing::warn!(name: "remote.command_send.refused", target: "remote", reason = ?refused, "command has no ctrl wire form; refused without delivery");
                    return false;
                }
            },
        };
        self.cmd_tx.send(CtrlCmd::PlayerCmd(wire_cmd)).is_ok()
    }

    #[must_use]
    pub fn play(
        &self,
        item: &EmbyItem,
        source: mbv_queue::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) -> bool {
        let queue_item = QueueItem::Emby(Box::new(item.clone()));
        self.send_ctrl_cmd(CtrlCmd::unified_queue_replace(
            vec![mbv_ctrl::UnifiedQueueSlot {
                slot_id: 1,
                item: queue_item,
            }],
            Some(0),
            source,
        ))
    }

    #[must_use]
    pub fn play_queue(
        &self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) -> bool {
        let slots: Vec<_> = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: (index + 1) as u64,
                item: QueueItem::Emby(Box::new(item)),
            })
            .collect();
        self.send_ctrl_cmd(CtrlCmd::unified_queue_replace(
            slots,
            Some(start_idx),
            source,
        ))
    }

    pub fn stop(&self) {
        let _ = self
            .send_playback_intent(self.new_playback_intent(mbv_ctrl::PlaybackIntentAction::Stop));
    }

    /// Actively tears down the control-socket connection (#233): shuts
    /// down the shared underlying socket so the reader thread's blocking
    /// `read()` (inside `reader.lines()` in `connect_endpoint`) observes
    /// EOF/an error and exits, instead of leaking forever the way it did
    /// when the only teardown was an implicit `Drop` of one fd duplicate.
    /// Idempotent: the stored handle is taken out on first use, so a
    /// second call is a no-op rather than a double `shutdown()`.
    ///
    /// # Panics
    ///
    /// Panics if the `control_stream` mutex is poisoned: a previous owner
    /// panicked while holding it.
    pub fn disconnect(&self) {
        if let Some(stream) = self.control_stream.lock().unwrap().take()
            && let Err(e) = stream.shutdown()
        {
            tracing::warn!(name: "remote.control_socket_shutdown.failed", target: "remote", error = %e, "control-socket shutdown failed");
        }
    }

    #[cfg(any(test, feature = "test"))]
    pub fn set_ctrl_compatibility_for_test(&mut self, compatibility: CtrlCompatibility) {
        self.ctrl_compatibility = compatibility;
    }

    #[must_use]
    pub fn supports_audiobookshelf_queue(&self) -> bool {
        self.ctrl_compatibility.audiobookshelf.queue
    }

    #[must_use]
    pub fn supports_audiobookshelf_book_queue(&self) -> bool {
        self.ctrl_compatibility.audiobookshelf.book_queue
    }

    #[must_use]
    pub fn supports_queue_append(&self) -> bool {
        self.ctrl_compatibility.supports_queue_append
    }

    #[must_use]
    pub fn supports_lifecycle_shutdown(&self) -> bool {
        self.ctrl_compatibility.supports_lifecycle_shutdown
    }

    #[must_use]
    pub fn supports_audio_only(&self) -> bool {
        self.ctrl_compatibility.supports_audio_only
    }

    #[must_use]
    pub fn supports_owner_queue_load(&self) -> bool {
        self.ctrl_compatibility.supports_owner_queue_load
    }

    #[must_use]
    pub fn supports_answered_queue_ops(&self) -> bool {
        self.ctrl_compatibility.supports_answered_queue_ops
    }

    pub fn load_queue_idle(
        &self,
        request_id: mbv_ctrl::QueueLoadRequestId,
        slots: Vec<mbv_ctrl::UnifiedQueueSlot>,
        cursor: usize,
        source: mbv_queue::QueueSource,
    ) -> Result<(), RemotePlayerError> {
        if !self.supports_owner_queue_load() {
            return Err(RemotePlayerError::queue_operation(
                "daemon does not support owner-authoritative idle queue loads",
            ));
        }
        if !self.send_ctrl_cmd(CtrlCmd::UnifiedQueueLoadIdle {
            request_id,
            slots,
            cursor,
            source,
        }) {
            return Err(RemotePlayerError::queue_operation(
                "could not send idle queue load to Player owner",
            ));
        }
        tracing::info!(
            name: "queue.load.sent",
            target: "ctrl",
            queue_request = request_id,
            "idle queue load sent"
        );
        Ok(())
    }

    /// Send an owner queue operation, using answered correlation when negotiated.
    pub fn send_queue_op(
        &self,
        operation: QueueOp,
    ) -> Result<Option<QueueOpId>, RemotePlayerError> {
        let op = self
            .supports_answered_queue_ops()
            .then(|| QueueOpId(self.next_queue_op_id.fetch_add(1, Ordering::Relaxed)));
        let command = match operation {
            QueueOp::Replace {
                items,
                start_idx,
                source,
            } => CtrlCmd::UnifiedQueueReplace {
                items: items.iter().map(|slot| slot.item.clone()).collect(),
                slots: items,
                start_idx,
                source,
                op,
            },
            QueueOp::Append { items, before } => CtrlCmd::UnifiedQueueAppend { items, before, op },
            QueueOp::RemoveSlot { slot_id } => CtrlCmd::UnifiedQueueRemoveSlot { slot_id, op },
            QueueOp::RemoveSlots { slot_ids } => CtrlCmd::UnifiedQueueRemoveSlots { slot_ids, op },
            QueueOp::MoveSlot { slot_id, to_index } => CtrlCmd::UnifiedQueueMoveSlot {
                slot_id,
                to_index,
                op,
            },
            QueueOp::PlaySlot { slot_id } => CtrlCmd::UnifiedQueuePlaySlot { slot_id, op },
            QueueOp::Clear => match op {
                Some(op) => CtrlCmd::UnifiedQueueClearOp { op },
                None => CtrlCmd::UnifiedQueueClear,
            },
            QueueOp::SourceUpdate { source, lineage } => {
                if !self.supports_owner_queue_load() {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support owner queue source updates",
                    ));
                }
                CtrlCmd::UnifiedQueueSourceUpdate {
                    source,
                    lineage,
                    op,
                }
            }
            QueueOp::Refresh => {
                let Some(op) = op else {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support answered queue refresh",
                    ));
                };
                CtrlCmd::UnifiedQueueRefresh { op }
            }
            QueueOp::ApplyProgress { updates } => {
                let Some(op) = op else {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support answered queue progress",
                    ));
                };
                CtrlCmd::UnifiedQueueApplyProgress { op, updates }
            }
        };
        if !self.send_ctrl_cmd(command) {
            return Err(RemotePlayerError::queue_operation(
                "could not send queue operation to Player owner",
            ));
        }
        Ok(op)
    }

    /// Snapshot of the owner-reported playback state. Locks internally and
    /// returns an owned value; callers never touch the shared mutex.
    ///
    /// # Panics
    ///
    /// Panics if the `status` mutex is poisoned: a previous owner panicked
    /// while holding it.
    #[must_use]
    pub fn status_snapshot(&self) -> PlayerStatus {
        self.status.lock().unwrap().clone()
    }

    /// Replace the whole owner-reported playback state (used when an owner
    /// event carries a fresh full status).
    ///
    /// # Panics
    ///
    /// Panics if the `status` mutex is poisoned: a previous owner panicked
    /// while holding it.
    pub fn set_status(&self, status: PlayerStatus) {
        *self.status.lock().unwrap() = status;
    }

    /// In-place update of the owner-reported playback state.
    ///
    /// # Panics
    ///
    /// Panics if the `status` mutex is poisoned: a previous owner panicked
    /// while holding it.
    pub fn update_status(&self, apply: impl FnOnce(&mut PlayerStatus)) {
        apply(&mut self.status.lock().unwrap());
    }

    /// Snapshot of the negotiated subtitle/audio preferences. Locks
    /// internally and returns an owned value.
    ///
    /// # Panics
    ///
    /// Panics if the `subtitle_prefs` mutex is poisoned: a previous owner
    /// panicked while holding it.
    #[must_use]
    pub fn subtitle_prefs_snapshot(&self) -> mbv_ctrl::player::SubtitlePrefs {
        self.subtitle_prefs.lock().unwrap().clone()
    }

    /// Replace the stored subtitle/audio preferences (client-side sync path;
    /// the owner learns of the change through the next preference command).
    ///
    /// # Panics
    ///
    /// Panics if the `subtitle_prefs` mutex is poisoned: a previous owner
    /// panicked while holding it.
    pub fn set_subtitle_prefs(&self, prefs: mbv_ctrl::player::SubtitlePrefs) {
        *self.subtitle_prefs.lock().unwrap() = prefs;
    }

    /// In-place update of the stored subtitle/audio preferences.
    ///
    /// # Panics
    ///
    /// Panics if the `subtitle_prefs` mutex is poisoned: a previous owner
    /// panicked while holding it.
    pub fn update_subtitle_prefs(&self, apply: impl FnOnce(&mut mbv_ctrl::player::SubtitlePrefs)) {
        apply(&mut self.subtitle_prefs.lock().unwrap());
    }

    /// # Panics
    ///
    /// Panics if the `unified_queue` mutex is poisoned: a previous owner
    /// panicked while holding it.
    #[must_use]
    pub fn unified_queue_state(&self) -> Option<mbv_ctrl::UnifiedQueueStateData> {
        self.unified_queue.lock().unwrap().clone()
    }

    /// Test-support seam: in-place edit of the cached unified queue state
    /// (`None` stays `None`), for stubs that must present owner state the
    /// fake connection never delivers over ctrl.
    ///
    /// # Panics
    ///
    /// Panics if the `unified_queue` mutex is poisoned: a previous owner
    /// panicked while holding it.
    #[cfg(any(test, feature = "test"))]
    pub fn update_unified_queue_state_for_test(
        &self,
        apply: impl FnOnce(&mut mbv_ctrl::UnifiedQueueStateData),
    ) {
        if let Some(state) = self.unified_queue.lock().unwrap().as_mut() {
            apply(state);
        }
    }

    #[cfg(any(test, feature = "test"))]
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
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn stub(items: Vec<EmbyItem>, current_idx: usize) -> (Self, mpsc::Receiver<PlayerEvent>) {
        let (remote, event_rx, _cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        (remote, event_rx)
    }

    /// Test helper variant that also exposes commands sent to the daemon.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn stub_with_command_rx(
        items: impl AsRef<[EmbyItem]>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let slots: Vec<_> = items
            .as_ref()
            .iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: index as u64 + 1,
                item: QueueItem::Emby(Box::new(item.clone())),
            })
            .collect();
        let status = Self::stub_status(current_idx, slots.len());
        let unified_queue = mbv_ctrl::UnifiedQueueStateData {
            status: status.clone(),
            active_slot: slots.get(current_idx).map(|slot| slot.slot_id),
            slots,
            revision: 0,
            source: QueueSource::Unknown,
            lineage: QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        };
        let status = Arc::new(Mutex::new(status));
        let subtitle_prefs = Arc::new(Mutex::new(mbv_ctrl::player::SubtitlePrefs::default()));
        let disconnected = Arc::new(AtomicBool::new(false));
        let shutdown_announced = Arc::new(AtomicBool::new(false));
        let next_playback_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let pending_playback = Arc::new(Mutex::new(HashMap::new()));
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();
        let (_event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let compat = CtrlCompatibility::current();
        (
            RemotePlayer {
                status,
                subtitle_prefs,
                unified_queue: Arc::new(Mutex::new(Some(unified_queue))),
                cmd_tx,
                disconnected,
                shutdown_announced,
                ctrl_compatibility: compat,
                control_stream: Arc::new(Mutex::new(None)),
                next_playback_id,
                next_queue_op_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
                pending_playback,
                shutdown_request_tx: Arc::new(Mutex::new(None)),
            },
            event_rx,
            cmd_rx,
        )
    }

    /// Test-support stub that advertises owner-authoritative idle queue loads.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn stub_owner_queue_load_with_command_rx(
        items: Vec<EmbyItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let (mut remote, event_rx, cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        remote.ctrl_compatibility.supports_owner_queue_load = true;
        (remote, event_rx, cmd_rx)
    }

    /// Test-support stub whose advertised ctrl capability identifies an
    /// audio-only playback owner.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn stub_audio_only_with_command_rx(
        items: Vec<EmbyItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let (mut remote, event_rx, cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        remote.ctrl_compatibility.supports_audio_only = true;
        (remote, event_rx, cmd_rx)
    }

    /// Test-support stub whose owner answers correlated queue operations
    /// (queue-owner-process row 5.1: the Client waits for `QueueOpResult`).
    /// A daemon advertising `answered-queue-ops` always advertises
    /// `owner-queue-load` with it (one capability list), so the stub sets both.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn stub_answered_queue_ops_with_command_rx(
        items: Vec<EmbyItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let (mut remote, event_rx, cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        remote.ctrl_compatibility.supports_answered_queue_ops = true;
        remote.ctrl_compatibility.supports_owner_queue_load = true;
        (remote, event_rx, cmd_rx)
    }
}
