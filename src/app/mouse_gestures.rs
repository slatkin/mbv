//! Shell-invoked mouse effect handlers for migrated interactive surfaces.

use crate::app::action::Command;
use crate::app::components::msg::TvHit;
use crate::app::layout::LibraryRowTarget;
use crate::app::{App, PanelFocus, QueueScope, TabSelection};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerCommand;
use mbv_core::remote_reconciliation::RemoteIntent;
use ratatui::layout::Position;
use std::time::{Duration, Instant};

impl App {
    pub(super) fn is_browse_layout_current(&self) -> bool {
        match self.layout.main.browse_destination {
            Some(tag) => tag == self.tab,
            None => true,
        }
    }

    pub(super) fn browse_mouse_ready(&mut self) -> bool {
        if self.normalize_stale_browse_destination() {
            return false;
        }
        self.is_browse_layout_current()
    }

    pub(super) fn click_set_cursor(&mut self, col: u16, row: u16) -> bool {
        // A selected Service library index that no longer exists normalizes
        // to Home and aborts the triggering gesture.
        if self.normalize_stale_browse_destination() {
            return false;
        }
        // Queue-scope and queue-area clicks are NOT browse surfaces: they
        // stay live even during the one-frame window in which the installed
        // completed-frame layout still describes the previous destination
        // (design §4 governs only Service browse geometry, so these are
        // handled before the tag check below).
        if self.has_direct_remote_queue() {
            if self
                .layout
                .main
                .queue_scope_local_area
                .contains((col, row).into())
            {
                self.handle_mouse_selector_click_queue(QueueScope::Local);
                return true;
            }
            if self
                .layout
                .main
                .queue_scope_remote_area
                .contains((col, row).into())
            {
                self.handle_mouse_selector_click_queue(QueueScope::Remote);
                return true;
            }
        }
        // Click in queue area: focus queue and move cursor.
        let qa = self.layout.main.queue_area;
        if qa.contains((col, row).into()) {
            if !matches!(self.effective_panel_focus(), PanelFocus::Queue) {
                self.last_card_height = 0;
                self.last_card_width = 0;
            }
            self.set_panel_focus(PanelFocus::Queue);
            let content_y = (row - qa.y) as usize;
            if let Some(&Some(item_idx)) = self.layout.main.queue_row_map.get(content_y) {
                self.mark_queue_cursor_user_active();
                self.displayed_queue_mut().queue_cursor = item_idx;
            }
            return true;
        }
        // Everything below is a browse surface and is a no-op until the
        // installed completed-frame layout was rendered for the selected
        // destination (design §4): a stale frame describes the previous
        // destination's geometry and must not be interpreted.
        if !self.is_browse_layout_current() {
            return false;
        }
        {
            if let TabSelection::AudiobookshelfLibrary(_) = self.tab {
                let pos = (col, row).into();
                if self.layout.main.is_wide_podcast_active() {
                    if let Some(episode_index) = self
                        .layout
                        .main
                        .audiobookshelf_episode_rows
                        .iter()
                        .find(|(rect, _)| rect.contains(pos))
                        .map(|(_, index)| *index)
                    {
                        let Some(index) = self.tab.audiobookshelf_index() else {
                            return false;
                        };
                        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
                            if state.episode_selection.is_some() {
                                state.episode_selection = Some(episode_index);
                                self.set_panel_focus(PanelFocus::Library);
                                return true;
                            }
                        }
                    }
                }
            }
            if let TabSelection::AudiobookshelfLibrary(index) = self.tab {
                let pos = (col, row).into();
                if self.layout.main.is_wide_book_active() {
                    if let Some(chapter_index) = self
                        .layout
                        .main
                        .audiobookshelf_book_chapter_rows
                        .iter()
                        .find(|(rect, _)| rect.contains(pos))
                        .map(|(_, index)| *index)
                    {
                        if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
                            state.chapter_selection = Some(chapter_index);
                            self.set_panel_focus(PanelFocus::Library);
                            return true;
                        }
                    }
                }
            }
            if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
                let pos = (col, row).into();
                if self.is_music_group_view(lib_idx) && self.layout.main.is_wide_music_active() {
                    if let Some(track) = self.layout.main.wide_music_track_at(pos) {
                        self.set_panel_focus(PanelFocus::Library);
                        self.libs[lib_idx].album_track_focus = Some(track);
                        return true;
                    }
                }
            }
            // Click on the inline hero: same as clicking anywhere else in
            // the library pane -- a single click only focuses (the cursor
            // is already on the selected item, so there's nothing else to
            // move). Activation (playing a movie, entering a Series'
            // season/episode selection) is a double-click gesture, handled
            // in `handle_mouse`'s `is_double` branch alongside every other
            // library-row activation, so it can't drift from Enter's
            // behavior or from the app-wide "single click only focuses"
            // convention.
            if self
                .layout
                .main
                .inline_hero_area
                .contains((col, row).into())
            {
                // The hero is a browse surface for the two Services that can
                // publish it (Emby and Audiobookshelf); match positively
                // rather than excluding Home/Feeds.
                match self.tab {
                    TabSelection::EmbyLibrary(_) | TabSelection::AudiobookshelfLibrary(_) => {
                        self.set_panel_focus(PanelFocus::Library);
                        return true;
                    }
                    TabSelection::Home | TabSelection::Feeds => {
                        self.set_panel_focus(PanelFocus::Library);
                        return true;
                    }
                }
            }
            // Wide Music: right-pane clicks (pills + album browser) bypass
            // the left_area gate because the right pane is a physically
            // separate rect. Track hits in the left pane flow through the
            // existing left_area block (wide left pane IS left_area).
            if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
                // Wide hero-on-left panes: tvshows clicks are claimed by
                // `TvWorkspaceComponent` (episode rows and season pills are
                // resolved there); this branch survives for wide Emby
                // podcast libraries, which render the same panes but mount
                // no component (task 5.3d, tv_workspace hit_test).
                if self.layout.main.is_wide_tv_active()
                    && self.wide_tv_panes_click(lib_idx, col, row)
                {
                    return true;
                }
                if self.is_music_group_view(lib_idx) && self.layout.main.is_wide_music_active() {
                    let pos = (col, row).into();
                    for (rect, target) in self.layout.main.selector_tabs.clone() {
                        if rect.contains(pos) {
                            self.set_panel_focus(PanelFocus::Library);
                            self.select_music_group(lib_idx, target);
                            return true;
                        }
                    }
                    let ra = self.layout.main.wide_music_right_area;
                    if ra.contains(pos) {
                        self.set_panel_focus(PanelFocus::Library);
                        let browser = self.layout.main.wide_music_browser_area;
                        if browser.contains(pos) {
                            let click_y = (row - browser.y) as usize;
                            let row_target = self
                                .layout
                                .main
                                .left_row_targets
                                .get(click_y)
                                .cloned()
                                .flatten();
                            if let Some(LibraryRowTarget::Album(item_idx)) = row_target {
                                let lib = &mut self.libs[lib_idx];
                                if let Some(lvl) = lib.nav_stack.last_mut() {
                                    if item_idx < lvl.items.len() {
                                        lib.album_track_focus = None;
                                        lvl.cursor = item_idx;
                                        self.save_default_library_position(lib_idx);
                                    }
                                }
                            }
                        }
                        return true;
                    }
                }
            }
            // Audiobookshelf book tab: right-pane clicks (bucket pills +
            // book browser) bypass the left_area gate because the right
            // pane is a physically separate rect, mirroring wide Music
            // above -- except the book right pane is always populated (both
            // panes render at every width per the book-browsing spec), so
            // this isn't gated on a "wide" flag.
            if let TabSelection::AudiobookshelfLibrary(index) = self.tab {
                if matches!(
                    self.audiobookshelf_kind_at(index),
                    Some(crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book)
                ) {
                    let pos = (col, row).into();
                    for (rect, target) in self.layout.main.selector_tabs.clone() {
                        if rect.contains(pos) {
                            self.set_panel_focus(PanelFocus::Library);
                            self.select_audiobookshelf_book_bucket(target);
                            return true;
                        }
                    }
                    let ra = self.layout.main.audiobookshelf_book_right_area;
                    if ra.contains(pos) {
                        self.set_panel_focus(PanelFocus::Library);
                        let click_y = (row - ra.y) as usize;
                        let row_target = self
                            .layout
                            .main
                            .left_row_targets
                            .get(click_y)
                            .cloned()
                            .flatten();
                        if let Some(LibraryRowTarget::Book(book_idx)) = row_target {
                            self.focus_audiobookshelf_book_browser();
                            self.select_audiobookshelf_book(book_idx);
                        }
                        return true;
                    }
                }
            }
            // Click in the left panel: focus it and set its cursor.
            let la = self.layout.main.left_area;
            if la.contains((col, row).into()) {
                if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
                    self.last_card_height = 0;
                    self.last_card_width = 0;
                }
                self.set_panel_focus(PanelFocus::Library);
                // Exhaustive destination dispatch: each Service reads only
                // its own hit targets / row maps / index space. There is no
                // default-to-Emby branch.
                match self.tab {
                    TabSelection::Home => {
                        // Home tab: rectangle hit-test the two-column card grid.
                        let pos = (col, row).into();
                        if let Some((_, flat_idx)) = self
                            .layout
                            .main
                            .home
                            .hitmap
                            .iter()
                            .find(|(rect, _)| rect.contains(pos))
                        {
                            self.home.home_cursor = *flat_idx;
                        }
                    }
                    TabSelection::Feeds => {}
                    TabSelection::AudiobookshelfLibrary(index) => {
                        let click_y = (row - la.y) as usize;
                        let cols = crate::app::library_column_width::library_column_count(la.width);
                        let cell_width =
                            crate::app::library_column_width::library_cell_width(la, cols) as usize;
                        let x = (col - la.x) as usize;
                        let stride = cell_width
                            + crate::app::library_column_width::LIBRARY_COLUMN_GAP as usize;
                        let cell = x / stride;
                        let target = (x % stride < cell_width)
                            .then(|| {
                                self.layout
                                    .main
                                    .left_item_rows
                                    .get(self.layout.main.left_screen_offset + click_y)
                                    .and_then(|row| row.get(cell))
                                    .copied()
                            })
                            .flatten();
                        if let Some(target) = target {
                            match self.audiobookshelf_kind_at(index) {
                                Some(crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                                    self.select_audiobookshelf_book(target);
                                }
                                _ if self.podcast_filter_target_active(index) => {
                                    self.select_audiobookshelf_filter(target);
                                }
                                _ => self.select_audiobookshelf_show(target),
                            }
                        }
                        return true;
                    }
                    TabSelection::EmbyLibrary(lib_idx) => {
                        if self.is_music_group_view(lib_idx)
                            || self.is_feed_home_video_group_view(lib_idx)
                            || self.should_show_letter_pills(lib_idx)
                        {
                            let target = self
                                .layout
                                .main
                                .selector_tabs
                                .iter()
                                .copied()
                                .find(|(rect, _)| rect.contains((col, row).into()))
                                .map(|(_, target)| target);
                            if let Some(target) = target {
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else if self.is_feed_home_video_group_view(lib_idx) {
                                    self.select_feed_folder_group(lib_idx, target);
                                } else {
                                    self.select_letter_pill(lib_idx, target);
                                }
                                return true;
                            }
                        }
                        let click_y = (row - la.y) as usize;
                        // Read the row map before taking a mutable borrow on libs (borrow checker).
                        let use_row_map = !self.layout.main.left_row_map.is_empty();
                        let row_map_item = if use_row_map {
                            self.layout.main.left_row_map.get(click_y).copied()
                        } else {
                            None
                        };
                        // Two-column lists: resolve the clicked *cell* so the item
                        // under the click is selected, not the row's first item.
                        // Geometry mirrors the renderer's cell layout, derived
                        // from the list pane width. `cell_target` is
                        // Some(Some(idx)) for a filled cell, Some(None) for an
                        // empty cell / inter-column gap (cursor unchanged), and
                        // None when the list is single-column (fall through to
                        // the existing row-map / arithmetic paths).
                        let cell_target: Option<Option<usize>> = {
                            use crate::app::library_column_width::{
                                library_cell_width, LIBRARY_COLUMN_GAP,
                            };
                            let cols = self.current_library_columns(lib_idx);
                            let cw = library_cell_width(la, cols) as usize;
                            let x = (col as usize).saturating_sub(la.x as usize);
                            if !self.layout.main.left_item_rows.is_empty() && cols > 1 && cw > 0 {
                                let cell = x / (cw + LIBRARY_COLUMN_GAP as usize);
                                if cell < cols {
                                    // Grouped album views pack display rows into screen
                                    // rows; use the screen-row offset from the last
                                    // render so the Y index matches left_item_rows.
                                    let scroll = if self.is_music_group_view(lib_idx) {
                                        self.layout.main.left_screen_offset
                                    } else {
                                        self.libs[lib_idx]
                                            .nav_stack
                                            .last()
                                            .map(|l| l.scroll)
                                            .unwrap_or(0)
                                    };
                                    Some(
                                        self.layout
                                            .main
                                            .left_item_rows
                                            .get(scroll + click_y)
                                            .and_then(|row| row.get(cell).copied()),
                                    )
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };
                        let row_target = self
                            .layout
                            .main
                            .left_row_targets
                            .get(click_y)
                            .cloned()
                            .flatten();
                        if self.is_music_group_view(lib_idx) {
                            // Wide Music: handle left-pane track clicks and
                            // right-pane album clicks separately.
                            let is_wide = self.layout.main.is_wide_music_active();
                            if is_wide {
                                let pos = (col, row).into();
                                // Track hitmap (left pane).
                                if let Some(track_idx) = self.layout.main.wide_music_track_at(pos) {
                                    self.libs[lib_idx].album_track_focus = Some(track_idx);

                                    self.set_panel_focus(PanelFocus::Library);
                                    self.save_default_library_position(lib_idx);
                                    return true;
                                }
                                // Artwork area: no-op.
                                if self.layout.main.wide_music_art_area.contains(pos) {
                                    return true;
                                }
                                // Any remaining left-pane click is consumed
                                // (other surfaces — pills, album browser — are
                                // in the right pane and handled above).
                                return true;
                            } else {
                                // Narrow path: existing row_target handling.
                                match row_target {
                                    Some(LibraryRowTarget::Album(item_idx)) => {
                                        let lib = &mut self.libs[lib_idx];
                                        if let Some(lvl) = lib.nav_stack.last_mut() {
                                            if item_idx < lvl.items.len() {
                                                if lvl.cursor != item_idx {
                                                    lib.album_track_focus = None;
                                                }
                                                lvl.cursor = item_idx;
                                                self.save_default_library_position(lib_idx);
                                                return true;
                                            }
                                        }
                                    }
                                    Some(LibraryRowTarget::Book(_)) | None => {}
                                }
                            }
                        }
                        let is_feed_group = self.is_feed_home_video_group_view(lib_idx);
                        let lib = &mut self.libs[lib_idx];
                        if is_feed_group {
                            let visible = la.height as usize;
                            if let Some(state) = lib.feed_home_video.as_mut() {
                                let items_len = state.selected_len();
                                if use_row_map {
                                    if let Some(Some(item_idx)) = row_map_item {
                                        if item_idx < items_len {
                                            state.video_cursor = item_idx;
                                        }
                                    }
                                } else {
                                    let offset = if state.video_cursor >= visible {
                                        state.video_cursor - visible + 1
                                    } else {
                                        0
                                    };
                                    let clicked = offset + click_y;
                                    if clicked < items_len {
                                        state.video_cursor = clicked;
                                    }
                                }
                            }
                            self.save_default_library_position(lib_idx);
                        } else if let Some(lvl) = lib.nav_stack.last_mut() {
                            if let Some(cell_item) = cell_target {
                                // Two-column list: select the item under the
                                // click's cell; empty cells/gaps leave the
                                // cursor unchanged.
                                if let Some(item_idx) = cell_item {
                                    if item_idx < lvl.items.len() {
                                        if lvl.cursor != item_idx {
                                            lib.album_track_focus = None;
                                        }
                                        lvl.cursor = item_idx;
                                    }
                                }
                            } else if use_row_map {
                                // Letter-grouped mode: row map gives item index (None = header row).
                                if let Some(Some(item_idx)) = row_map_item {
                                    if item_idx < lvl.items.len() {
                                        if lvl.cursor != item_idx {
                                            lib.album_track_focus = None;
                                        }
                                        lvl.cursor = item_idx;
                                    }
                                }
                            } else {
                                let visible = la.height as usize;
                                let offset = if lvl.cursor >= visible {
                                    lvl.cursor - visible + 1
                                } else {
                                    0
                                };
                                let clicked = offset + click_y;
                                if clicked < lvl.items.len() {
                                    if lvl.cursor != clicked {
                                        lib.album_track_focus = None;
                                    }
                                    lvl.cursor = clicked;
                                }
                            }
                            self.save_default_library_position(lib_idx);
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    pub(super) fn seek_to_col(&mut self, col: u16) {
        let bar = self.layout.playback.seekbar_area;
        if bar.width == 0 {
            return;
        }
        let fraction = (col.saturating_sub(bar.x)) as f64 / bar.width as f64;
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let runtime_s = self
                .connected_session_state
                .as_ref()
                .map(|s| s.runtime_s)
                .unwrap_or(0);
            if runtime_s == 0 {
                return;
            }
            let ticks = (fraction * (runtime_s * mbv_core::api::TICKS_PER_SECOND) as f64) as i64;
            let id = conn_id.clone();
            self.remote_pos_s = (fraction * runtime_s as f64) as i64;
            self.remote_pos_at = Instant::now();
            self.remote_seek_pending_until = Instant::now() + Duration::from_secs(4);
            self.issue_remote_intent(RemoteIntent::Seek);
            self.do_reconciliation_session_command(&id.clone(), move |c| {
                c.session_seek(&id, ticks)
            });
            return;
        }
        let runtime_ticks = self.player.status.lock().unwrap().runtime_ticks;
        if runtime_ticks == 0 {
            return;
        }
        let target_secs = (fraction * runtime_ticks as f64) / TICKS_PER_SECOND as f64;
        self.player
            .send_command(PlayerCommand::SeekAbsolute(target_secs));
        // Mark a pending Feed seek so the next OutputStarted persists
        // the resulting position (confirmed seek completion).
        if let Some(slot_id) = self.playback_queue().queue.active_slot_id() {
            if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                if matches!(slot.item, mbv_core::playback_queue::QueueItem::Feed(ref e) if e.feed_id.is_some())
                {
                    self.feed_seek_pending_slot = Some(slot_id);
                }
            }
        }
    }

    pub(super) fn handle_mouse_scroll_browse(&mut self, delta: i64) {
        if !self.browse_mouse_ready() {
            return;
        }
        match self.tab {
            TabSelection::Home => self.cw_move_cursor(delta),
            TabSelection::EmbyLibrary(lib_idx) => self.move_lib_cursor(lib_idx, delta),
            TabSelection::AudiobookshelfLibrary(_) => {
                let in_episodes = self
                    .tab
                    .audiobookshelf_index()
                    .and_then(|index| self.audiobookshelf_browse.get(index))
                    .is_some_and(|state| state.episode_selection.is_some());
                if in_episodes {
                    self.move_audiobookshelf_episode_cursor(delta * 3);
                } else {
                    self.move_audiobookshelf_show_rows(delta * 3);
                }
            }
            TabSelection::Feeds => {}
        }
    }

    pub(super) fn note_browse_double_click(&mut self, col: u16, row: u16) -> bool {
        let now = Instant::now();
        let is_double = now.duration_since(self.last_click_time) < Duration::from_millis(400)
            && self.last_click_pos == (col, row);
        self.last_click_time = now;
        self.last_click_pos = (col, row);
        is_double
    }

    pub(super) fn note_browse_scroll(&mut self) -> bool {
        let now = Instant::now();
        let allow = now.duration_since(self.last_scroll_at) >= Duration::from_millis(30);
        if allow {
            self.last_scroll_at = now;
        }
        allow
    }

    pub(super) fn handle_mouse_selector_click_queue(&mut self, scope: QueueScope) {
        self.set_queue_scope(scope);
    }

    pub(super) fn handle_mouse_single_click_tv(
        &mut self,
        lib_idx: usize,
        hit: TvHit,
        col: u16,
        row: u16,
    ) {
        match hit {
            TvHit::SeasonTab(_) | TvHit::EpisodeRow(_) => {
                self.set_panel_focus(super::PanelFocus::Library);
            }
            TvHit::SeriesRow => {
                self.wide_tv_panes_click(lib_idx, col, row);
            }
            TvHit::EpisodesPane => {}
        }
    }

    pub(super) fn handle_mouse_double_click_tv(&mut self, lib_idx: usize, col: u16, row: u16) {
        let pos: Position = (col, row).into();
        if self
            .layout
            .main
            .tv_wide_episode_rows
            .iter()
            .any(|(rect, _)| rect.contains(pos))
            || self.layout.main.tv_wide_right_area.contains(pos)
        {
            self.activate_selected_series(lib_idx);
        }
    }

    pub(super) fn handle_mouse_right_click_tv(
        &mut self,
        lib_idx: usize,
        hit: TvHit,
        col: u16,
        row: u16,
    ) {
        match hit {
            TvHit::SeasonTab(_) | TvHit::EpisodeRow(_) => {
                self.set_panel_focus(super::PanelFocus::Library);
            }
            TvHit::SeriesRow => {
                self.wide_tv_panes_click(lib_idx, col, row);
            }
            TvHit::EpisodesPane => {}
        }
        self.open_context_menu_at(col, row);
    }

    pub(super) fn wide_tv_panes_click(&mut self, lib_idx: usize, col: u16, row: u16) -> bool {
        let pos: Position = (col, row).into();
        if self.layout.main.tv_wide_left_area.contains(pos) {
            return true;
        }
        let right = self.layout.main.tv_wide_right_area;
        if right.contains(pos) {
            let click_y = (row.saturating_sub(self.layout.main.left_area.y)) as usize;
            let target = self
                .layout
                .main
                .left_row_map
                .get(click_y)
                .copied()
                .flatten();
            if let Some(target) = target {
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.cursor = target;
                }
            }
            return true;
        }
        false
    }

    pub(super) fn handle_mouse_scroll_queue(&mut self, delta: i64) {
        let n = self.displayed_queue().total_queue_len();
        if n > 0 {
            let queue = self.displayed_queue_mut();
            queue.queue_cursor = super::ui_util::move_cursor(queue.queue_cursor, delta * 3, n);
        }
    }

    pub(super) fn handle_mouse_double_click_queue(&mut self, col: u16, row: u16) {
        let queue = self.displayed_queue();
        if queue.queue_cursor < queue.total_queue_len()
            && self.layout.main.queue_area.contains((col, row).into())
        {
            // Spatial hit-test stays local (issue #134); the activation is
            // the same QueuePlayCursor command as queue Enter.
            self.dispatch(Command::QueuePlayCursor);
        }
    }

    pub(super) fn handle_mouse_right_click_queue(&mut self, col: u16, row: u16) {
        match self.tab {
            TabSelection::Home => self.handle_mouse_right_click_home(col, row),
            TabSelection::EmbyLibrary(_) => self.handle_mouse_right_click_emby(col, row),
            TabSelection::AudiobookshelfLibrary(_) => {
                self.handle_mouse_right_click_audiobookshelf(col, row)
            }
            TabSelection::Feeds => self.handle_mouse_right_click_feeds(col, row),
        }
    }

    pub(super) fn handle_mouse_selector_click_home(&mut self, target: usize) {
        self.home_select_section(target);
    }

    pub(super) fn handle_mouse_selector_click_emby(&mut self, lib_idx: usize, target: usize) {
        if self.is_music_group_view(lib_idx) {
            self.select_music_group(lib_idx, target);
        } else if self.is_feed_home_video_group_view(lib_idx) {
            self.select_feed_folder_group(lib_idx, target);
        } else if self.should_show_letter_pills(lib_idx) {
            self.select_letter_pill(lib_idx, target);
        }
    }

    pub(super) fn handle_mouse_double_click_home(&mut self, in_left: bool) {
        if in_left {
            self.home_play();
        }
    }

    pub(super) fn handle_mouse_double_click_emby(
        &mut self,
        lib_idx: usize,
        in_left: bool,
        pos: Position,
    ) -> bool {
        if in_left {
            if self.layout.main.is_wide_music_active() {
                if let Some(track_idx) = self.layout.main.wide_music_track_at(pos) {
                    self.libs[lib_idx].album_track_focus = Some(track_idx);
                    self.select(lib_idx);
                }
                // Double-click on artwork or blank space: no-op.
                return true;
            }
            if self.is_viewing_album_folders(lib_idx) {
                self.activate_album_folder_row(lib_idx);
            } else if !self.activate_selected_series(lib_idx) {
                self.select(lib_idx);
            }
        }
        false
    }

    pub(super) fn handle_mouse_right_click_home(&mut self, col: u16, row: u16) {
        if self.click_set_cursor(col, row) {
            self.open_context_menu_at(col, row);
        }
    }

    pub(super) fn handle_mouse_right_click_emby(&mut self, col: u16, row: u16) {
        if self.click_set_cursor(col, row) {
            self.open_context_menu_at(col, row);
        }
    }

    pub(super) fn handle_mouse_right_click_audiobookshelf(&mut self, col: u16, row: u16) {
        self.click_set_cursor(col, row);
    }

    pub(super) fn handle_mouse_right_click_feeds(&mut self, col: u16, row: u16) {
        self.click_set_cursor(col, row);
    }
}
