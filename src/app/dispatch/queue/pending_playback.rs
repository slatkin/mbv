use super::{App, QueueScope, ToastSeverity};
use crate::app::state::queue_owner::LocalQueueOwner;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use mbv_core::player::PlayerCommand;

impl App {
    pub(super) fn execute_pending_play_items(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        autostart: bool,
    ) {
        if autostart && self.player.is_remote_disconnected() {
            self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            );
            return;
        }
        let direct_remote = self.has_direct_remote_queue();
        if !autostart && self.local_queue_owner() == LocalQueueOwner::StayAlive {
            self.load_idle_queue_on_owner(items, start_idx, source);
            return;
        }
        if self.local_queue_metadata_applies(self.playing_queue_scope()) {
            self.set_queue_source_if_not_local_daemon(source.clone());
        }
        if autostart {
            self.start_pending_queue_playback(&items, start_idx, source, direct_remote);
        } else {
            self.load_pending_queue_locally(&items, start_idx, direct_remote);
        }
    }

    fn load_idle_queue_on_owner(
        &mut self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
    ) {
        let request_id = self.next_owner_queue_load_request;
        self.next_owner_queue_load_request = self.next_owner_queue_load_request.saturating_add(1);
        let slots = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_core::ctrl::UnifiedQueueSlot {
                slot_id: (index + 1) as u64,
                item: QueueItem::Emby(Box::new(item)),
            })
            .collect();
        let result = self
            .player
            .as_remote()
            .map(|remote| remote.load_queue_idle(request_id, slots, start_idx, source));
        match result {
            Some(Ok(())) => self.set_queue_scope(self.playing_queue_scope()),
            Some(Err(_)) if self.player.is_remote_disconnected() => self.flash(
                crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE.into(),
                ToastSeverity::Warning,
            ),
            Some(Err(reason)) => self.flash(reason, ToastSeverity::Error),
            None => self.flash(
                "Could not send idle queue load to Player owner".into(),
                ToastSeverity::Error,
            ),
        }
    }

    fn load_pending_queue_locally(
        &mut self,
        items: &[EmbyItem],
        start_idx: usize,
        direct_remote: bool,
    ) {
        // Playlist Enter populates the queue; Space/Enter starts it.
        let loaded = items
            .get(start_idx)
            .map(mbv_core::api::EmbyItem::playback_label);
        if !direct_remote {
            self.replace_playback_queue(items.to_vec(), start_idx);
        }
        self.set_queue_scope(self.playing_queue_scope());
        self.persist_local_queue_state_if_needed(self.playing_queue_scope());
        if let Some(label) = loaded {
            self.flash(format!("Loaded: {label}"), ToastSeverity::Neutral);
        }
    }

    fn start_pending_queue_playback(
        &mut self,
        items: &[EmbyItem],
        start_idx: usize,
        source: crate::config::QueueSource,
        direct_remote: bool,
    ) {
        if !direct_remote {
            self.replace_playback_queue(items.to_vec(), start_idx);
        }
        self.set_queue_scope(self.playing_queue_scope());
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let label = items
                .get(start_idx)
                .map(mbv_core::api::EmbyItem::playback_label)
                .unwrap_or_default();
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.submit_attached_sequence(&id, items, start_idx);
        } else {
            self.submit_tab_queue(self.playing_queue_scope(), start_idx, source);
            self.player
                .send_command(PlayerCommand::SetMute(self.mute_on));
        }
        if !direct_remote {
            self.save_queue_state();
        }
    }

    pub(super) fn execute_pending_queue_clear(&mut self) {
        let scope = self.viewed_queue_scope();
        let had_items = self.queue_for_scope(scope).total_queue_len() > 0;
        if self.local_queue_metadata_applies(scope) {
            self.clear_local_queue_metadata();
        } else {
            self.remote_queue_undo_stack.clear();
        }
        if scope == QueueScope::Remote && had_items {
            self.clear_remote_queue();
        } else if self.queue_scope_is_playback(scope) {
            self.reset_bare_transitions();
            self.player.stop();
            if self.local_queue_owner() == LocalQueueOwner::StayAlive {
                self.player.clear_queue();
            }
        }
        if scope != QueueScope::Remote {
            let queue = self.queue_for_scope_mut(scope);
            queue.clear();
        }
        if had_items {
            self.advance_queue_epoch();
        }
        if self.local_queue_metadata_applies(scope) {
            self.save_queue_state_after_explicit_clear();
        }
        self.flash("Queue cleared".into(), ToastSeverity::Success);
    }
}
