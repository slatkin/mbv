use super::*;

impl App {
    pub(in crate::app) fn queue_is_saved_playlist(&self) -> bool {
        matches!(
            &self.queue_source,
            crate::config::QueueSource::Playlist { id: Some(_), .. }
        )
    }

    pub(in crate::app) fn queue_playlist_id(&self) -> Option<&str> {
        if let crate::config::QueueSource::Playlist {
            id: Some(ref id), ..
        } = self.queue_source
        {
            Some(id.as_str())
        } else {
            None
        }
    }

    pub(in crate::app) fn queue_playlist_name(&self) -> &str {
        if let crate::config::QueueSource::Playlist { ref name, .. } = self.queue_source {
            name.as_str()
        } else {
            ""
        }
    }

    pub(in crate::app) fn save_playlist_to_emby(&mut self) {
        let Some(playlist_id) = self.queue_playlist_id().map(str::to_string) else {
            return;
        };
        let Some(origin) = self.queue_origin_or_flash() else {
            return;
        };
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        self.enqueue_playlist_mutation(
            &playlist_id,
            PlaylistMutation::Save {
                mutation_id,
                origin,
                source_playlist_id: playlist_id.clone(),
                item_ids: None,
            },
        );
    }

    pub(in crate::app) fn save_queue_as_playlist(&mut self, name: String) {
        let Some(origin) = self.queue_origin_or_flash() else {
            return;
        };
        let source_playlist_id = self.queue_playlist_id().map(str::to_string);
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        let key = source_playlist_id
            .clone()
            .filter(|id| self.playlist_mutation_pending(id))
            .unwrap_or_else(|| format!("create:{mutation_id}"));
        self.enqueue_playlist_mutation(
            &key,
            PlaylistMutation::CreateAs {
                mutation_id,
                coordinator_key: key.clone(),
                name,
                origin,
                source_playlist_id,
                item_ids: None,
            },
        );
    }

    fn playlist_mutation_pending(&self, playlist_id: &str) -> bool {
        self.playlist_mutations
            .get(playlist_id)
            .is_some_and(|state| state.active.is_some() || !state.queued.is_empty())
    }

    /// Clears `playlist_item_id` from local queue items after a full playlist
    /// update recreates server entry identities. Every full update
    /// (Save/Replace/CreateAs) pushes this queue to Emby, so those identities
    /// are invalidated.
    pub(in crate::app) fn clear_local_playlist_entry_ids(&mut self) {
        let slot_ids: Vec<_> = self
            .player_tab
            .queue
            .slots()
            .iter()
            .filter(|s| matches!(&s.item, QueueItem::Emby(_)))
            .map(|s| s.slot_id)
            .collect();
        for slot_id in slot_ids {
            if let Some(slot) = self.player_tab.queue.slot(slot_id) {
                if let QueueItem::Emby(mut item) = slot.item.clone() {
                    item.playlist_item_id.clear();
                    let _ = self
                        .player_tab
                        .queue
                        .update_slot_item(slot_id, QueueItem::Emby(item));
                }
            }
        }
    }

    pub(in crate::app) fn enqueue_playlist_mutation(
        &mut self,
        playlist_id: &str,
        mutation: PlaylistMutation,
    ) {
        let state = self
            .playlist_mutations
            .entry(playlist_id.to_string())
            .or_default();
        if state.active.is_some() {
            state.queued.push_back(mutation);
        } else {
            state.active = Some(mutation);
            self.start_playlist_mutation(playlist_id);
        }
    }

    pub(in crate::app) fn finish_playlist_mutation(&mut self, playlist_id: &str, mutation_id: u64) {
        let Some(state) = self.playlist_mutations.get_mut(playlist_id) else {
            return;
        };
        if state.active.as_ref().map(PlaylistMutation::mutation_id) != Some(mutation_id) {
            return;
        }
        state.active = None;
        if let Some(next) = state.queued.pop_front() {
            state.active = Some(next);
            self.start_playlist_mutation(playlist_id);
        } else {
            self.playlist_mutations.remove(playlist_id);
        }
    }
}
