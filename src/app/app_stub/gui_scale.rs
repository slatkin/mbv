//! Bench-scale seeds for the GUI `sections` bench (`gui-change-driven-
//! sections` design D12). Each seed writes the same stores the GUI library
//! tests seed, at the size a bench fixture names: the caller builds the
//! items and the seed lands them as a landed fetch would. The seeds compile
//! only under the root package's `test` feature, as the rest of this stub
//! module does.

use std::sync::{Arc, Mutex};

use super::{GUI_MUSIC_LIBRARY_ID, music_group_level};
use crate::app::App;
use crate::app::state::service_runtime::EmbyRuntime;

impl App {
    /// Make the grouped Music library the active destination with `groups`
    /// as its group level: the Emby runtime becomes Ready with `client`, and
    /// the Music tab holds one group row per item. The caller seeds each
    /// group's albums with `seed_gui_music_albums_for_test`, so the store
    /// holds what a landed set of group fetches holds.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_groups_for_test(
        &mut self,
        client: mbv_emby::EmbyClient,
        groups: Vec<mbv_emby_model::EmbyItem>,
    ) {
        self.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
        self.music_levels = vec!["group".to_string(), "album".to_string()];
        let mut library = mbv_emby_model::test_support::make_item("Music", "CollectionFolder");
        library.id = GUI_MUSIC_LIBRARY_ID.to_string();
        library.collection_type = "music".to_string();
        library.is_folder = true;
        let mut tab = mbv_ui_model::library_tab::LibraryTab::new(library);
        tab.nav_stack.push(music_group_level(groups));
        self.libs = vec![tab];
        self.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
    }

    /// Make the Shows library the active destination with `series` as its
    /// root level under the All mode: the stored mode is All, so the frame's
    /// tree reads the root level's series and no mode load starts.
    #[cfg(feature = "test")]
    pub fn seed_gui_tv_series_for_test(
        &mut self,
        client: mbv_emby::EmbyClient,
        series: Vec<mbv_emby_model::EmbyItem>,
    ) {
        self.seed_gui_tv_library_for_test(client);
        if let Some(level) = self
            .libs
            .get_mut(0)
            .and_then(|tab| tab.nav_stack.last_mut())
        {
            level.items = series;
        }
        self.gui_tv_shows.entry(0).or_default().mode = Some(mbv_queue::TvContentMode::All);
    }

    /// Record `album_id` as the pick of `group_id` in `lib_idx`, as the
    /// album pick command does: the selected group's Hero shows the pick
    /// while the group's union holds it.
    #[cfg(feature = "test")]
    pub fn pick_gui_album_for_test(&mut self, lib_idx: usize, group_id: &str, album_id: &str) {
        self.gui_library_picks
            .insert((lib_idx, group_id.to_string()), album_id.to_string());
    }
}
