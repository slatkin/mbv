use super::types_playback::PlaylistMutation;
use super::App;

impl App {
    /// Effect for `ConfirmAction::SaveOverwritePlaylist`'s "yes" answer
    /// (`y`): deletes the existing playlist and recreates it under the same
    /// name with the current queue's items. Extracted from the old
    /// `SavePlaylistStage::ConfirmOverwrite` key handler so the shared
    /// confirmation-modal dispatcher can call it directly.
    pub(super) fn do_overwrite_playlist(&mut self, existing_id: &str, name: &str) {
        self.force_clear = true;
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        self.enqueue_playlist_mutation(
            existing_id.to_string(),
            PlaylistMutation::Replace {
                mutation_id,
                queue_lineage: self.remote_queue_lineage,
                source_playlist_id: existing_id.to_string(),
                name: name.to_string(),
                item_ids: None,
            },
        );
    }
}
