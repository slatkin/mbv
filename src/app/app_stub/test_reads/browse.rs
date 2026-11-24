//! The Emby library browse fixtures a Client's test seeds (`split-tui-gui-clients`
//! design D4b rule 5): the album-track and album-index caches a landed fetch or
//! rebuild leaves, the grouping levels the libraries browse by, the persisted
//! browse position, and the selected-series activation.
//!
//! They sit in their own child module of `test_reads` so `home.rs` stays within
//! its line ceiling.

use super::super::App;
use mbv_emby_model::EmbyItem;

impl App {
    /// Hold the grouping levels the music libraries browse by, as a saved
    /// library configuration leaves them (`group`, `artist`, `album`).
    #[cfg(any(test, feature = "test"))]
    pub fn seed_music_levels_for_test(&mut self, levels: &[&str]) {
        self.music_levels = levels.iter().map(|level| (*level).to_string()).collect();
    }

    /// Hold `tracks` as the landed album-track fetch of the album with
    /// `album_id`, as an album's track page leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_album_track_items_for_test(&mut self, album_id: &str, tracks: Vec<EmbyItem>) {
        self.album_tracks_cache.insert(album_id.to_string(), tracks);
    }

    /// Whether the album-track cache holds no album, as a browse that fetched
    /// none leaves it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn album_tracks_cache_is_empty_for_test(&self) -> bool {
        self.album_tracks_cache.is_empty()
    }

    /// Hold `state` as the recursive album index of the library with
    /// `library_id`, as a started or settled rebuild leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_album_index_state_for_test(
        &mut self,
        library_id: &str,
        state: crate::app::AlbumIndexState,
    ) {
        self.album_indexes.insert(library_id.to_string(), state);
    }

    /// The persisted browse position of the library with `library_id`, if a
    /// previous run left one.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn library_position_for_test(
        &self,
        library_id: &str,
    ) -> Option<&mbv_queue::LibraryPosition> {
        self.library_position_state.libraries.get(library_id)
    }

    /// The configured Emby client, when the running runtime holds one.
    ///
    /// # Panics
    ///
    /// Panics when the Emby client lock is poisoned.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn emby_client_for_test(&self) -> Option<mbv_emby::EmbyClient> {
        self.emby_runtime
            .client
            .as_ref()
            .map(|client| client.lock().expect("the Emby client lock holds").clone())
    }

    /// Activate the selected series of the Emby library at `index`, as the
    /// library's Enter chord does.
    #[cfg(any(test, feature = "test"))]
    pub fn activate_selected_series_for_test(&mut self, index: usize) -> bool {
        self.activate_selected_series(index)
    }

    /// Whether the Emby library at `index` browses album folders, as the
    /// grouping level the library opened at decides.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn is_viewing_album_folders_for_test(&self, index: usize) -> bool {
        self.is_viewing_album_folders(index)
    }
}
