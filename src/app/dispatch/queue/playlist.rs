use super::{App, PlaylistMutation};

impl App {
    pub(in crate::app) fn queue_is_saved_playlist(&self) -> bool {
        matches!(
            self.playback_queue().source(),
            mbv_queue::QueueSource::Playlist { id: Some(_), .. }
        )
    }

    pub(in crate::app) fn queue_playlist_id(&self) -> Option<&str> {
        if let mbv_queue::QueueSource::Playlist { id: Some(id), .. } =
            self.playback_queue().source()
        {
            Some(id.as_str())
        } else {
            None
        }
    }

    pub(in crate::app) fn queue_playlist_name(&self) -> &str {
        if let mbv_queue::QueueSource::Playlist { name, .. } = self.playback_queue().source() {
            name.as_str()
        } else {
            ""
        }
    }

    pub(in crate::app) fn save_playlist_to_emby(&mut self) -> Option<u64> {
        let playlist_id = self.queue_playlist_id().map(str::to_string)?;
        let origin = self.queue_origin_or_flash()?;
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
        Some(mutation_id)
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
