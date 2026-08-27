use super::{App, PanelFocus, PanelMode, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::api::EmbyItem;

impl App {
    /// Alt-modified keys are destination-independent: Alt+Left/Right switch
    /// panel focus, Alt+Up/Down cycle the left-panel tab, and every other
    /// Alt chord is swallowed (the historical catch-all) so it can never
    /// leak into queue-item handling below the browse seam. Called from
    /// `handle_key_view_dispatch` ahead of panel/destination dispatch.
    pub(super) fn handle_key_alt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Right if matches!(self.effective_panel_focus(), PanelFocus::Queue) => {
                self.set_panel_focus(PanelFocus::Library);
                self.last_card_height = 0; // reset stale image height for new view
                self.last_card_width = 0;
            }
            KeyCode::Left
                if matches!(self.effective_panel_focus(), PanelFocus::Library)
                    && self.effective_panel_mode() == PanelMode::Both =>
            {
                self.set_panel_focus(PanelFocus::Queue);
                self.last_card_height = 0;
                self.last_card_width = 0;
            }
            KeyCode::Up => self.library_tab_prev(),
            KeyCode::Down => self.library_tab_next(),
            _ => {}
        }
    }

    /// The library-panel keyboard front door: resolve the selected left-panel
    /// destination exhaustively (Home, one Emby library, one Audiobookshelf
    /// library, or Feeds) and dispatch to its handler or consume the key.
    ///
    /// A stale Service library index normalizes to Home first and stops the
    /// dispatch without any destination-specific handling. Unsupported keys
    /// are consumed by the Service handler so they never fall through to
    /// another destination or the queue.
    pub(super) fn handle_key_browse_dispatch(
        &mut self,
        key: KeyEvent,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) -> Option<bool> {
        if self.normalize_stale_browse_destination() {
            return Some(false);
        }
        match self.tab {
            TabSelection::Home => self.handle_key_home(key),
            TabSelection::EmbyLibrary(index) => {
                self.handle_key_emby_library(index, key, home_cw_selected, cw_item)
            }
            TabSelection::AudiobookshelfLibrary(index) => {
                let Some(kind) = self.audiobookshelf_kind_at(index) else {
                    return Some(false);
                };
                match kind {
                    // Audiobookshelf keyboard navigation is owned by the
                    // mounted component; the App swallows keys here so none
                    // fall through to queue-item handling. Typed component
                    // requests handle the supported book actions before this
                    // generic legacy path is reached.
                    super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast
                    | super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                        Some(false)
                    }
                }
            }
            TabSelection::Feeds => self.handle_key_feeds(key),
        }
    }

    /// Home / Continue Watching keys: the local navigation set and the typed
    /// effect-key family (Enter, Ctrl+Enter, Ctrl+A, Ctrl+W, Delete) are
    /// owned by `HomeComponent` (task 5.3d). Nothing remains on the legacy
    /// path here; this handler only preserves Home's catch-all swallow so no
    /// key falls through to queue-item handling while Home is focused.
    fn handle_key_home(&mut self, _key: KeyEvent) -> Option<bool> {
        Some(false)
    }

    /// The Emby library keyboard handler for the exhaustively matched
    /// `EmbyLibrary(lib_idx)`. Preserves the season/music-group/pill
    /// switching and inline album/series-selection interception, then routes
    /// through `handle_lib_key`. Every other key is consumed here (the
    /// view's historical final catch-all), never falling through to
    /// queue-item handling.
    fn handle_key_emby_library(
        &mut self,
        lib_idx: usize,
        key: KeyEvent,
        home_cw_selected: bool,
        cw_item: Option<EmbyItem>,
    ) -> Option<bool> {
        // Season switching: [ = previous season, ] = next season.
        if !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            if let Some(delta) = match key.code {
                KeyCode::Char('[') => Some(-1),
                KeyCode::Char(']') => Some(1),
                _ => None,
            } {
                if self.is_music_group_view(lib_idx) {
                    self.switch_music_group(lib_idx, delta);
                    return Some(false);
                }
                if self.is_feed_home_video_group_view(lib_idx) {
                    self.switch_feed_folder_group(lib_idx, delta);
                    return Some(false);
                }
                // Letter-range pill cycling for large non-music libraries.
                if self.should_show_letter_pills(lib_idx) {
                    self.cycle_letter_pill(lib_idx, delta);
                    return Some(false);
                }
            }
        }

        // Album-folder-listing activation: Enter opens the narrow selection
        // modal. Wide track-focus entry is owned by
        // `MusicWorkspaceComponent` (Enter on an album row is
        // component-local), so this legacy arm is the non-component path
        // (`is_viewing_album_folders` is also reached for plain album
        // browsing outside the group view). The legacy track-mutation paths
        // (Enter/Up/Down reinterpreting a focused track) were deleted with
        // the inline track-focus field (task 5.3d, Album track focus).
        if self.is_viewing_album_folders(lib_idx) {
            match key.code {
                KeyCode::Enter => {
                    self.activate_album_folder_row(lib_idx);
                    return Some(false);
                }
                _ => {}
            }
        }

        // Activate series-selection mode on Enter when the cursor is on a
        // Series item (instead of drilling down via `select`). Wide keeps
        // the existing in-hero episode focus; narrow has no inline
        // season/episode block to focus (see
        // `render_series_inline_detail`), so it opens the selection modal
        // instead (design.md Decision 6).
        if key.code == KeyCode::Enter && self.activate_selected_series(lib_idx) {
            return Some(false);
        }

        // Tab/BackTab are consumed by `handle_global_view_key` in
        // `handle_key_view_dispatch` before browse dispatch is reached.
        if let Some(quit) = self.handle_lib_key(lib_idx, key, home_cw_selected, cw_item) {
            return Some(quit);
        }
        // Every other key is consumed here, never falling through to
        // queue-item handling.
        Some(false)
    }

    /// Applies the single Series activation gate shared by keyboard Enter and
    /// browse double-click. Narrow presentations open the selection modal;
    /// wide presentations retain the persistent season/episode workspace.
    pub(super) fn activate_selected_series(&mut self, lib_idx: usize) -> bool {
        let Some(item) = self.selected_series_item(lib_idx) else {
            return false;
        };
        if self.layout.main.is_wide_tv_active() {
            self.enter_series_selection(lib_idx, &item);
        } else {
            self.open_series_selection_modal(&item);
        }
        true
    }
}
