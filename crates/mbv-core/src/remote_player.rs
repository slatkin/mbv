use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::api::{EmbyClient, EmbyItem};
use crate::ctrl::{CtrlCmd, CtrlCompatibility, PlaybackIntent};
use crate::playback_queue::{FeedEntry, QueueItem};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

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

#[derive(Clone)]
pub struct RemotePlayer {
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<crate::player::SubtitlePrefs>>,
    pub items: Arc<Mutex<Vec<EmbyItem>>>,
    pub unified_queue: Arc<Mutex<Option<crate::ctrl::UnifiedQueueStateData>>>,
    pub queue_source: Arc<Mutex<crate::config::QueueSource>>,
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
    pub(crate) control_stream: Arc<Mutex<Option<ControlStream>>>,
    pub(crate) next_playback_id: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) pending_playback: Arc<Mutex<HashMap<u64, PlaybackIntent>>>,
    /// Completer for a pending shutdown request.
    pub(crate) shutdown_request_tx: Arc<Mutex<Option<mpsc::Sender<ShutdownResponse>>>>,
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

    /// Bounded lifecycle shutdown request.
    ///
    /// Sends `RequestShutdown` and waits for the daemon's response with a
    /// bounded timeout. Returns `Accepted` only when the daemon has durably
    /// persisted its queue and acknowledged the request; enqueue success
    /// alone is never returned as `Accepted`.
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
        }

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
            cmd => cmd.into(),
        };
        self.cmd_tx.send(CtrlCmd::PlayerCmd(wire_cmd)).is_ok()
    }

    pub fn adopt_queue(
        &self,
        items: Vec<QueueItem>,
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
        let emby_items: Vec<EmbyItem> = items
            .iter()
            .filter_map(|item| item.as_emby().cloned())
            .collect();
        *self.items.lock().unwrap() = emby_items.clone();
        *self.queue_source.lock().unwrap() = source.clone();
        if self.ctrl_compatibility.supports_unified_queue {
            self.cmd_tx
                .send(CtrlCmd::UnifiedAdoptQueue {
                    items,
                    cursor,
                    source,
                })
                .is_ok()
        } else {
            self.cmd_tx
                .send(CtrlCmd::AdoptQueue {
                    items: emby_items,
                    cursor,
                    source,
                })
                .is_ok()
        }
    }

    pub fn play(
        &self,
        item: &EmbyItem,
        source: crate::config::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) -> bool {
        let sent = if self.ctrl_compatibility.supports_unified_queue {
            self.send_ctrl_cmd(CtrlCmd::UnifiedQueueReplace {
                items: vec![QueueItem::Emby(Box::new(item.clone()))],
                start_idx: Some(0),
            })
        } else {
            self.send_playback_intent(self.new_playback_intent(
                crate::ctrl::PlaybackIntentAction::Play {
                    item_ids: vec![item.id.clone()],
                    start_idx: 0,
                    start_ticks: item.playback_position_ticks,
                    source: source.clone(),
                },
            ))
        };
        if sent {
            *self.items.lock().unwrap() = vec![item.clone()];
            *self.queue_source.lock().unwrap() = source;
        }
        sent
    }

    pub fn supports_feed_playback(&self) -> bool {
        self.ctrl_compatibility.supports_feed_playback
    }

    /// Legacy single-feed submission for peers that advertise `feed-playback`
    /// but not `unified-queue`. Sends the surviving `WireCommand::LoadFeed`
    /// envelope, which the daemon intercepts and folds into its canonical queue.
    pub fn play_feed(&self, entry: FeedEntry) -> bool {
        if !self.supports_feed_playback() {
            return false;
        }
        self.send_ctrl_cmd(CtrlCmd::PlayerCmd(crate::ctrl::WireCommand::LoadFeed {
            entry,
        }))
    }

    pub fn play_queue(
        &self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        _client: Arc<EmbyClient>,
        _initial_volume: u8,
    ) -> bool {
        let sent = if self.ctrl_compatibility.supports_unified_queue {
            let queue_items: Vec<QueueItem> = items
                .iter()
                .cloned()
                .map(|i| QueueItem::Emby(Box::new(i)))
                .collect();
            self.send_ctrl_cmd(CtrlCmd::UnifiedQueueReplace {
                items: queue_items,
                start_idx: Some(start_idx),
            })
        } else {
            let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
            let start_ticks = items
                .get(start_idx)
                .map_or(0, |i| i.playback_position_ticks);
            self.send_playback_intent(self.new_playback_intent(
                crate::ctrl::PlaybackIntentAction::Play {
                    item_ids,
                    start_idx,
                    start_ticks,
                    source: source.clone(),
                },
            ))
        };
        if sent {
            *self.items.lock().unwrap() = items;
            *self.queue_source.lock().unwrap() = source;
        }
        sent
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

    pub fn supports_lifecycle_shutdown(&self) -> bool {
        self.ctrl_compatibility.supports_lifecycle_shutdown
    }

    pub fn supports_unified_queue(&self) -> bool {
        self.ctrl_compatibility.supports_unified_queue
    }

    pub fn unified_queue_state(&self) -> Option<crate::ctrl::UnifiedQueueStateData> {
        self.unified_queue.lock().unwrap().clone()
    }

    pub fn queue_append(&self, items: Vec<QueueItem>) -> bool {
        if items.is_empty() {
            return true;
        }
        if self.supports_unified_queue() {
            self.send_ctrl_cmd(CtrlCmd::UnifiedQueueAppend { items })
        } else if items.iter().any(|item| matches!(item, QueueItem::Feed(_))) {
            false
        } else {
            self.send_command(PlayerCommand::QueueAppend { items })
        }
    }

    /// Remove a slot by its stable identity.  Falls back to `false` if the
    /// peer does not advertise `unified-queue`.
    pub fn queue_remove_slot(&self, slot_id: u64) -> bool {
        if self.supports_unified_queue() {
            self.send_ctrl_cmd(CtrlCmd::UnifiedQueueRemoveSlot { slot_id })
        } else {
            false
        }
    }

    /// Move a slot by its stable identity to `to_index`.  Falls back to
    /// `false` if the peer does not advertise `unified-queue`.
    pub fn queue_move_slot(&self, slot_id: u64, to_index: usize) -> bool {
        if self.supports_unified_queue() {
            self.send_ctrl_cmd(CtrlCmd::UnifiedQueueMoveSlot { slot_id, to_index })
        } else {
            false
        }
    }

    /// Begin playback of an existing slot by its stable identity.  Falls
    /// back to `false` if the peer does not advertise `unified-queue`.
    pub fn queue_play_slot(&self, slot_id: u64) -> bool {
        if self.supports_unified_queue() {
            self.send_ctrl_cmd(CtrlCmd::UnifiedQueuePlaySlot { slot_id })
        } else {
            false
        }
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
    pub fn stub(items: Vec<EmbyItem>, current_idx: usize) -> (Self, mpsc::Receiver<PlayerEvent>) {
        let (remote, event_rx, _cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        (remote, event_rx)
    }

    /// Test helper variant that also exposes commands sent to the daemon.
    pub fn stub_with_command_rx(
        items: Vec<EmbyItem>,
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
        let (cmd_tx, cmd_rx) = mpsc::channel::<CtrlCmd>();
        let (_event_tx, event_rx) = mpsc::channel::<PlayerEvent>();
        let compat = CtrlCompatibility::current();
        (
            RemotePlayer {
                status,
                subtitle_prefs,
                items,
                unified_queue: Arc::new(Mutex::new(None)),
                queue_source,
                cmd_tx,
                disconnected,
                shutdown_announced,
                ctrl_compatibility: compat,
                control_stream: Arc::new(Mutex::new(None)),
                next_playback_id,
                pending_playback,
                shutdown_request_tx: Arc::new(Mutex::new(None)),
            },
            event_rx,
            cmd_rx,
        )
    }
}
