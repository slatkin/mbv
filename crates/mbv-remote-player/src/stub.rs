//! Test-support stand-ins for a `RemotePlayer` without a live daemon
//! connection. Split from `lib.rs` along the test-support seam; each stub
//! carries its own `cfg(any(test, feature = "test"))` gate.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};

use mbv_ctrl::player::{PlayerEvent, PlayerStatus};
use mbv_ctrl::{CtrlCmd, CtrlCompatibility};
use mbv_emby_model::EmbyItem;
use mbv_queue::{QueueItem, QueueLineage, QueueSource};

use super::RemotePlayer;

impl RemotePlayer {
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
                playback_output: Arc::new(Mutex::new(None)),
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

    /// Test-support stub whose advertised ctrl capability accepts the
    /// owner's queue source updates (`QueueOp::SourceUpdate` sends its
    /// `UnifiedQueueSourceUpdate` instead of failing the operation).
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn stub_owner_queue_source_update_with_command_rx(
        items: Vec<EmbyItem>,
        current_idx: usize,
    ) -> (Self, mpsc::Receiver<PlayerEvent>, mpsc::Receiver<CtrlCmd>) {
        let (mut remote, event_rx, cmd_rx) = Self::stub_with_command_rx(items, current_idx);
        remote.ctrl_compatibility.supports_owner_queue_source_update = true;
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
        remote.ctrl_compatibility.supports_owner_queue_source_update = true;
        (remote, event_rx, cmd_rx)
    }
}
