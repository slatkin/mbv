use mbv_ids::ItemId;

use super::{Arc, Duration, EmbyClient, EmbyItem, EndFileReason, ExecSlot};
use mbv_ctrl::player::{PlayerCommand, PlayerStatus, SubtitlePrefs};

/// Client-side view of a `RemotePlayer`.
///
/// The shared-state internals of the wrapped `RemotePlayer` stay hidden:
/// callers read state through `status_snapshot` / `subtitle_prefs_snapshot`
/// and act through the send methods, never through the shared mutexes.
#[derive(Clone, Debug)]
pub struct PlayerProxy {
    pub always_play_next: bool,
    remote: mbv_remote_player::RemotePlayer,
}

impl PlayerProxy {
    #[must_use]
    pub fn from_remote(remote: mbv_remote_player::RemotePlayer, always_play_next: bool) -> Self {
        Self {
            always_play_next,
            remote,
        }
    }

    #[must_use]
    pub fn remote(&self) -> &mbv_remote_player::RemotePlayer {
        &self.remote
    }

    #[must_use]
    pub fn can_admit_audiobookshelf(&self) -> bool {
        self.remote.supports_audiobookshelf_queue()
            && self.remote.supports_audiobookshelf_book_queue()
    }

    #[must_use]
    pub fn owner_is_audio_only(&self) -> bool {
        self.remote.supports_audio_only()
    }

    pub fn play(
        &self,
        item: &EmbyItem,
        source: mbv_queue::QueueSource,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        let _ = self.remote.play(item, source, client, initial_volume);
    }

    pub fn play_queue(
        &self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        let _ = self
            .remote
            .play_queue(items, start_idx, source, client, initial_volume);
    }

    #[must_use]
    pub fn submit_queue_slots(
        &self,
        slots: Vec<ExecSlot>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) -> bool {
        if slots.is_empty()
            || slots.iter().any(|slot| {
                (slot.item.as_audiobookshelf().is_some()
                    && !self.remote.supports_audiobookshelf_queue())
                    || (slot.item.as_audiobookshelf_book().is_some()
                        && !self.remote.supports_audiobookshelf_book_queue())
            })
        {
            return false;
        }
        let start_idx = start_idx.min(slots.len() - 1);
        self.remote
            .send_ctrl_cmd(mbv_ctrl::CtrlCmd::unified_queue_replace(
                slots
                    .into_iter()
                    .map(|slot| mbv_ctrl::UnifiedQueueSlot {
                        slot_id: slot.slot_id.raw(),
                        item: slot.item,
                    })
                    .collect(),
                Some(start_idx),
                source,
            ))
    }

    pub fn stop(&self) {
        self.remote.stop();
    }

    pub fn disconnect_remote(&self) {
        self.remote.disconnect();
    }

    #[must_use]
    pub fn send_ctrl_cmd(&self, cmd: mbv_ctrl::CtrlCmd) -> bool {
        self.remote.send_ctrl_cmd(cmd)
    }

    #[must_use]
    pub fn send_playback_intent(&self, intent: mbv_ctrl::PlaybackIntent) -> bool {
        self.remote.send_playback_intent(intent)
    }

    #[must_use]
    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        self.remote.send_command(cmd)
    }

    #[must_use]
    pub fn next(&self) -> bool {
        self.remote.send_playback_intent(
            self.remote
                .new_playback_intent(mbv_ctrl::PlaybackIntentAction::Next),
        )
    }

    #[must_use]
    pub fn previous(&self) -> bool {
        self.remote.send_playback_intent(
            self.remote
                .new_playback_intent(mbv_ctrl::PlaybackIntentAction::Previous),
        )
    }

    #[must_use]
    pub fn set_paused(&self, paused: bool) -> bool {
        self.remote.send_playback_intent(
            self.remote
                .new_playback_intent(mbv_ctrl::PlaybackIntentAction::SetPaused { paused }),
        )
    }

    #[must_use]
    pub fn is_remote_disconnected(&self) -> bool {
        self.remote.is_disconnected()
    }

    #[must_use]
    pub fn is_shutdown_announced(&self) -> bool {
        self.remote.is_shutdown_announced()
    }

    /// Snapshot of the owner-reported playback state. Locks internally and
    /// returns an owned value; callers never touch the shared mutex.
    #[must_use]
    pub fn status_snapshot(&self) -> PlayerStatus {
        self.remote.status_snapshot()
    }

    /// Replace the whole owner-reported playback state (used when an owner
    /// event carries a fresh full status).
    pub fn set_status(&self, status: PlayerStatus) {
        self.remote.set_status(status);
    }

    /// In-place update of the owner-reported playback state.
    pub fn update_status(&self, apply: impl FnOnce(&mut PlayerStatus)) {
        self.remote.update_status(apply);
    }

    /// Snapshot of the negotiated subtitle/audio preferences.
    #[must_use]
    pub fn subtitle_prefs_snapshot(&self) -> SubtitlePrefs {
        self.remote.subtitle_prefs_snapshot()
    }

    /// Replace the stored subtitle/audio preferences (client-side sync path;
    /// the owner learns of the change through the next preference command).
    pub fn set_subtitle_prefs(&self, prefs: SubtitlePrefs) {
        self.remote.set_subtitle_prefs(prefs);
    }

    /// In-place update of the stored subtitle/audio preferences.
    pub fn update_subtitle_prefs(&self, apply: impl FnOnce(&mut SubtitlePrefs)) {
        self.remote.update_subtitle_prefs(apply);
    }

    /// Dispatch a transport command (MPRIS/tray) through the remote owner:
    /// `Step` becomes a guarded playback intent, everything else goes over
    /// the legacy command channel.
    pub fn send_transport(&self, transport: mbv_ctrl::TransportCommand) {
        self.remote.send_transport(transport);
    }
}

/// Retry `mark_played` in a detached thread with exponential backoff.
/// Max 3 attempts (initial + 2 retries), delays: 500ms, 2s.
pub(super) fn retry_mark_played(client: Arc<EmbyClient>, item_id: ItemId) {
    std::thread::spawn(move || {
        let delays = [500, 2000]; // ms
        for (i, delay_ms) in delays.iter().enumerate() {
            std::thread::sleep(Duration::from_millis(*delay_ms));
            match client.mark_played(item_id.as_str()) {
                Ok(()) => {
                    tracing::info!(name: "player.mark_played.retry_succeeded", target: "player", attempt = i + 1, item = %item_id, "mark played retry succeeded");
                    return;
                }
                Err(e) => {
                    tracing::warn!(name: "player.mark_played.retry_failed", target: "player", attempt = i + 1, item = %item_id, error = %e, "mark played retry failed");
                }
            }
        }
    });
}

// True when the track ended close enough to its natural end to count as played.
// Threshold: position ≥ 95% of runtime (19/20 integer check avoids floating point).
pub(crate) fn is_near_end(
    is_audio: bool,
    natural: bool,
    last_valid_pos: i64,
    runtime_ticks: i64,
) -> bool {
    !natural && !is_audio && runtime_ticks > 0 && last_valid_pos * 20 / runtime_ticks >= 19
}

// Position to report to Emby when a playlist track ends.
// Zero means "treat as fully played / reset resume point".
// was_next_up alone does NOT zero the position — the user may have dismissed
// or ignored the overlay, and we must preserve where they actually were.
pub(crate) fn queue_completed_pos(
    is_audio: bool,
    natural: bool,
    near_end: bool,
    last_valid_pos: i64,
) -> i64 {
    if is_audio || natural || near_end {
        0
    } else {
        last_valid_pos
    }
}

pub(super) fn quit_timeout_stop_flags(
    origin: super::PlaybackOrigin,
    is_audio: bool,
    last_valid_pos: i64,
    runtime_ticks: i64,
    stopped_near_end: bool,
) -> (bool, bool) {
    match origin {
        super::PlaybackOrigin::Standalone => (
            is_near_end(is_audio, false, last_valid_pos, runtime_ticks),
            false,
        ),
        super::PlaybackOrigin::Queue => (stopped_near_end, stopped_near_end),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StopReportContext {
    Ordinary,
    ShutdownAware,
}

pub(super) fn end_file_stop_report_context(reason: EndFileReason) -> StopReportContext {
    if reason == super::mpv_end_file_reason::Quit {
        StopReportContext::ShutdownAware
    } else {
        StopReportContext::Ordinary
    }
}
