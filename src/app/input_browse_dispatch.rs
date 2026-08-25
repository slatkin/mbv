use super::{App, PanelFocus, PanelMode, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    /// library, or Feeds) and call exactly one Service-specific handler.
    ///
    /// A stale Service library index normalizes to Home first and stops the
    /// dispatch without any destination-specific handling. Unsupported keys
    /// are consumed by the Service handler so they never fall through to
    /// another destination or the queue.
    pub(super) fn handle_key_browse_dispatch(
        &mut self,
        key: KeyEvent,
        home_cw_selected: bool,
    ) -> Option<bool> {
        if self.normalize_stale_browse_destination() {
            return Some(false);
        }
        match self.tab {
            TabSelection::Home => self.handle_key_home(key),
            TabSelection::EmbyLibrary(index) => {
                self.handle_key_emby_library(index, key, home_cw_selected)
            }
            TabSelection::AudiobookshelfLibrary(index) => {
                let Some(kind) = self.audiobookshelf_kind_at(index) else {
                    return Some(false);
                };
                match kind {
                    super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast => {
                        self.handle_key_audiobookshelf_library(index, key)
                    }
                    super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                        self.handle_key_audiobookshelf_book_library(index, key)
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
        if let Some(quit) = self.handle_lib_key(lib_idx, key, home_cw_selected) {
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

    /// The Audiobookshelf library keyboard handler for the exhaustively
    /// matched `AudiobookshelfLibrary(index)`. Preserves show navigation,
    /// paging, first/last, episode filter cycling, episode selection entry /
    /// exit, and the inert episode-mode Enter/Space. Every key is consumed:
    /// Emby-only actions and queue-item handling are unreachable from here.
    fn handle_key_audiobookshelf_library(&mut self, index: usize, key: KeyEvent) -> Option<bool> {
        let episode_selection = self
            .audiobookshelf_browse
            .get(index)
            .is_some_and(|state| state.episode_selection.is_some());
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if episode_selection => {
                self.move_audiobookshelf_episode_cursor(-1)
            }
            KeyCode::Down | KeyCode::Char('j') if episode_selection => {
                self.move_audiobookshelf_episode_cursor(1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_audiobookshelf_show_rows(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_audiobookshelf_show_rows(1),
            KeyCode::Left | KeyCode::Char('h') if !episode_selection => {
                self.move_audiobookshelf_show_cursor(-1)
            }
            KeyCode::Right | KeyCode::Char('l') if !episode_selection => {
                self.move_audiobookshelf_show_cursor(1)
            }
            KeyCode::PageUp if !episode_selection => {
                self.move_audiobookshelf_show_rows(-(self.lib_page_size() as i64))
            }
            KeyCode::PageDown if !episode_selection => {
                self.move_audiobookshelf_show_rows(self.lib_page_size() as i64)
            }
            KeyCode::Home if !episode_selection => self.jump_audiobookshelf_show_cursor(false),
            KeyCode::End if !episode_selection => self.jump_audiobookshelf_show_cursor(true),
            KeyCode::Char('[') if episode_selection => self.cycle_audiobookshelf_filter(-1),
            KeyCode::Char(']') if episode_selection => self.cycle_audiobookshelf_filter(1),
            KeyCode::Esc | KeyCode::Backspace if episode_selection => {
                self.leave_audiobookshelf_episode_selection()
            }
            KeyCode::Char(' ') if !episode_selection => {
                self.enter_audiobookshelf_episode_selection()
            }
            // Enter on a selected show (instead of the always-focus-episodes
            // behavior above): wide keeps the existing in-hero episode focus
            // (`episode_selection`); narrow has no inline episode block to
            // focus (see `render_audiobookshelf_hero`'s `persistent` gate),
            // so it opens the selection modal instead (design.md decisions
            // 4 and 6).
            KeyCode::Enter if !episode_selection => {
                if self.layout.main.is_wide_podcast_active() {
                    self.enter_audiobookshelf_episode_selection();
                } else {
                    self.open_podcast_selection_modal();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.play_selected_audiobookshelf_episode(index);
            }
            KeyCode::Char('a')
                if episode_selection && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.enqueue_selected_audiobookshelf_episode(index);
            }
            _ => {}
        }
        Some(false)
    }

    /// The Audiobookshelf book-library keyboard handler for the exhaustively
    /// matched `AudiobookshelfLibrary(index)` when its resolved kind is
    /// `Book`. Wide navigates either the hero's chapter list or the right-pane
    /// book browser and toggles pane focus with left/right arrow. Narrow
    /// navigates the browser and opens the chapter modal from the parent.
    /// It cycles the
    /// alphabetical-bucket pill with `[`/`]`, and plays or enqueues the
    /// selected book. Every key is consumed: podcast/Emby actions and
    /// queue-item handling are unreachable from here.
    fn handle_key_audiobookshelf_book_library(
        &mut self,
        index: usize,
        key: KeyEvent,
    ) -> Option<bool> {
        let chapters_focused = self.layout.main.is_wide_book_active()
            && self
                .audiobookshelf_book_browse
                .get(index)
                .is_some_and(|state| state.chapter_selection.is_some());
        // Bucket-pill cycling (a direct precedent: `switch_music_group`'s
        // `[`/`]` group cycling), available regardless of which pane is
        // focused.
        if !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            if let Some(delta) = match key.code {
                KeyCode::Char('[') => Some(-1),
                KeyCode::Char(']') => Some(1),
                _ => None,
            } {
                self.cycle_audiobookshelf_book_bucket(delta);
                return Some(false);
            }
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if chapters_focused => {
                self.move_audiobookshelf_book_row(-1)
            }
            KeyCode::Down | KeyCode::Char('j') if chapters_focused => {
                self.move_audiobookshelf_book_row(1)
            }
            KeyCode::Enter | KeyCode::Char(' ') if chapters_focused => {
                self.activate_audiobookshelf_book_row();
            }
            // Right arrow moves focus from the hero's chapter list to the
            // right-pane browser; Left is a no-op there (already leftmost).
            KeyCode::Right if chapters_focused => self.focus_audiobookshelf_book_browser(),
            KeyCode::Up | KeyCode::Char('k') => self.move_audiobookshelf_book_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_audiobookshelf_book_cursor(1),
            KeyCode::PageUp if !chapters_focused => {
                self.move_audiobookshelf_book_cursor(-(self.lib_page_size() as i64))
            }
            KeyCode::PageDown if !chapters_focused => {
                self.move_audiobookshelf_book_cursor(self.lib_page_size() as i64)
            }
            KeyCode::Home if !chapters_focused => self.jump_audiobookshelf_book_cursor(false),
            KeyCode::End if !chapters_focused => self.jump_audiobookshelf_book_cursor(true),
            // Left arrow moves focus from the browser to the hero's chapter
            // list; Right is a no-op there (already rightmost).
            KeyCode::Left if !chapters_focused && self.layout.main.is_wide_book_active() => {
                self.focus_audiobookshelf_book_chapters()
            }
            // Space plays the selected book (book-playback spec: ordinary
            // play). Narrow Enter opens the chapter modal when the hero fits;
            // otherwise it keeps ordinary book activation. Wide Enter keeps
            // the existing book activation behavior.
            KeyCode::Enter if !chapters_focused && !self.layout.main.is_wide_book_active() => {
                self.activate_audiobookshelf_book_parent();
            }
            KeyCode::Char(' ') | KeyCode::Enter if !chapters_focused => {
                self.play_selected_audiobookshelf_book(index);
            }
            KeyCode::Char('a')
                if !chapters_focused && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.enqueue_selected_audiobookshelf_book(index);
            }
            _ => {}
        }
        Some(false)
    }
}
