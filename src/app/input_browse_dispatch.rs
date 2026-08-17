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
    pub(super) fn handle_key_browse_dispatch(&mut self, key: KeyEvent) -> Option<bool> {
        if self.normalize_stale_browse_destination() {
            return Some(false);
        }
        match self.tab {
            TabSelection::Home => self.handle_key_home(key),
            TabSelection::EmbyLibrary(index) => self.handle_key_emby_library(index, key),
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

    /// Home / Continue Watching keys: section switching (`[` / `]`),
    /// watched-state changes (`Ctrl+W`), enqueue (`Ctrl+A`), and Home
    /// navigation (see `handle_cw_key`). Every other key is consumed so it
    /// never reaches queue-item handling while Home is focused.
    fn handle_key_home(&mut self, key: KeyEvent) -> Option<bool> {
        self.handle_cw_key(key);
        Some(false)
    }

    /// The Emby library keyboard handler for the exhaustively matched
    /// `EmbyLibrary(lib_idx)`. Preserves the season/music-group/pill
    /// switching and inline album/series-selection interception, then routes
    /// through `handle_lib_key`. Every other key is consumed here (the
    /// view's historical final catch-all), never falling through to
    /// queue-item handling.
    fn handle_key_emby_library(&mut self, lib_idx: usize, key: KeyEvent) -> Option<bool> {
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

        // Track-selection mode (#145 task 3): while the left panel is sitting
        // on the album-folder-listing nav level, Enter/Escape/Up/Down are
        // reinterpreted for moving a track focus within the currently
        // displayed album instead of drilling into `nav_stack` (`select`) or
        // moving the album cursor (`move_lib_cursor`). Alt-modified keys are
        // already handled by `handle_key_view_dispatch`; this is scoped to
        // `is_viewing_album_folders` (so movies/series/
        // home-video panels and other tabs are completely unaffected).
        if self.is_viewing_album_folders(lib_idx) {
            match key.code {
                KeyCode::Enter => {
                    self.activate_album_folder_row(lib_idx);
                    return Some(false);
                }
                KeyCode::Esc | KeyCode::Backspace => {
                    if self.libs[lib_idx].album_track_focus.is_some() {
                        self.libs[lib_idx].album_track_focus = None;
                        return Some(false);
                    }
                }
                KeyCode::Up | KeyCode::Down => {
                    if let Some(idx) = self.libs[lib_idx].album_track_focus {
                        let track_count = self
                            .selected_album_item(lib_idx)
                            .and_then(|item| self.album_tracks_cache.get(&item.id))
                            .map(|tracks| tracks.len())
                            .unwrap_or(0);
                        if track_count > 0 {
                            let delta: i64 = if key.code == KeyCode::Up { -1 } else { 1 };
                            let new_idx = super::ui_util::move_cursor(idx, delta, track_count);
                            self.libs[lib_idx].album_track_focus = Some(new_idx);
                        }
                        return Some(false);
                    }
                }
                _ => {}
            }
        }

        // Series-selection mode: while the left panel has a Series item
        // selected and selection mode is active, Enter/Escape/Up/Down/[/] are
        // intercepted for navigating within the inline series detail (season
        // pills + episode list) instead of drilling into `nav_stack`.
        if self.libs[lib_idx].series_selection.is_some() {
            match key.code {
                KeyCode::Enter => {
                    self.activate_series_selection_episode(lib_idx);
                    return Some(false);
                }
                KeyCode::Esc | KeyCode::Backspace => {
                    self.libs[lib_idx].series_selection = None;
                    return Some(false);
                }
                KeyCode::Up | KeyCode::Down => {
                    let delta: i64 = if key.code == KeyCode::Up { -1 } else { 1 };
                    if let Some(episodes) = self.series_selection_episodes(lib_idx) {
                        let count = episodes.len();
                        if count > 0 {
                            let cur = self.libs[lib_idx].series_selection.unwrap_or(0);
                            let new_idx = super::ui_util::move_cursor(cur, delta, count);
                            self.libs[lib_idx].series_selection = Some(new_idx);
                        }
                    }
                    return Some(false);
                }
                KeyCode::Char('[') => {
                    self.switch_series_selection_season(lib_idx, -1);
                    return Some(false);
                }
                KeyCode::Char(']') => {
                    self.switch_series_selection_season(lib_idx, 1);
                    return Some(false);
                }
                _ => {}
            }
        }
        // Activate series-selection mode on Enter when the cursor is on a
        // Series item (instead of drilling down via `select`).
        if key.code == KeyCode::Enter && self.libs[lib_idx].series_selection.is_none() {
            if let Some(item) = self.selected_series_item(lib_idx) {
                self.enter_series_selection(lib_idx, &item);
                return Some(false);
            }
        }

        // Tab/BackTab are consumed by `handle_global_view_key` in
        // `handle_key_view_dispatch` before browse dispatch is reached.
        if let Some(quit) = self.handle_lib_key(lib_idx, key) {
            return Some(quit);
        }
        // Every other key is consumed here, never falling through to
        // queue-item handling.
        Some(false)
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
            KeyCode::Enter | KeyCode::Char(' ') if !episode_selection => {
                self.enter_audiobookshelf_episode_selection()
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
    /// `Book`. Navigates whichever pane is focused (the hero's chapter list
    /// or the right-pane book browser), toggles pane focus with left/right
    /// arrow (replacing the old Enter-to-enter/Esc-to-leave modal
    /// transition -- both panes stay visible either way), cycles the
    /// alphabetical-bucket pill with `[`/`]`, and plays or enqueues the
    /// selected book. Every key is consumed: podcast/Emby actions and
    /// queue-item handling are unreachable from here.
    fn handle_key_audiobookshelf_book_library(
        &mut self,
        index: usize,
        key: KeyEvent,
    ) -> Option<bool> {
        let chapters_focused = self
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
            KeyCode::Left if !chapters_focused => self.focus_audiobookshelf_book_chapters(),
            // Space plays the selected book (book-playback spec: ordinary
            // play); Enter mirrors it now that it no longer opens chapter
            // selection (that transition is the Left/Right focus toggle
            // above).
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
