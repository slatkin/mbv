use crate::app::App;

impl App {
    /// Apply Audiobookshelf progress to browse state by provider-qualified
    /// identity. Queue progress is applied by the Player owner and arrives in
    /// its published snapshot.
    pub(in crate::app) fn reconcile_audiobookshelf_progress(
        &mut self,
        library_item_id: &str,
        episode_id: &str,
        current_time_seconds: f64,
        is_finished: bool,
    ) {
        for state in &mut self.audiobookshelf_browse {
            state.progress.insert(
                (library_item_id.to_string(), episode_id.to_string()),
                mbv_audiobookshelf::AudiobookshelfProgress {
                    library_item_id: library_item_id.to_string(),
                    episode_id: episode_id.to_string(),
                    current_time_seconds,
                    is_finished,
                },
            );
        }
    }

    /// Apply acknowledged book progress to browse state by library-item identity.
    pub(in crate::app) fn reconcile_audiobookshelf_book_progress(
        &mut self,
        library_item_id: &str,
        current_time_seconds: f64,
        is_finished: bool,
    ) {
        for state in &mut self.audiobookshelf_book_browse {
            state.progress.insert(
                library_item_id.to_string(),
                mbv_audiobookshelf::AudiobookshelfBookProgress {
                    library_item_id: library_item_id.to_string(),
                    current_time_seconds,
                    is_finished,
                },
            );
        }
    }
}
