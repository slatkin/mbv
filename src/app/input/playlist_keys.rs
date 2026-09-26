use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::types::playback::PlaylistMutation;
use crate::app::App;

impl App {
    /// Effect for `ConfirmAction::SaveOverwritePlaylist`'s "yes" answer
    /// (`y`): deletes the existing playlist and recreates it under the same
    /// name with the current queue's items. Extracted from the old
    /// `SavePlaylistStage::ConfirmOverwrite` key handler so the shared
    /// confirmation-modal dispatcher can call it directly.
    pub(in crate::app) fn do_overwrite_playlist(&mut self, existing_id: &str, name: &str) {
        let Some(origin) = self.queue_origin() else {
            self.flash(
                "Stay-alive queue not available yet".to_string(),
                ToastSeverity::Error,
            );
            return;
        };
        self.force_clear = true;
        let mutation_id = self.next_playlist_mutation;
        self.next_playlist_mutation = self.next_playlist_mutation.saturating_add(1);
        self.enqueue_playlist_mutation(
            existing_id,
            PlaylistMutation::Replace {
                mutation_id,
                origin,
                name: name.to_string(),
                item_ids: None,
            },
        );
    }
}
