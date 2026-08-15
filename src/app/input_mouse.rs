#![allow(unused_imports)]

use crate::app::action::Command;
use crate::app::layout::LibraryRowTarget;
use crate::app::{
    App, PanelFocus, PendingQueueAction, QueueScope, TabSelection, HELP_PANEL_W, PLAYLISTS_PANEL_W,
    SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use mbv_core::remote_reconciliation::RemoteIntent;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
impl App {
    /// Whether the installed completed-frame layout was rendered for the
    /// currently selected destination (design §4). This is the *tag* check
    /// alone: browse surfaces may be interpreted only when the tag matches.
    /// Callers responsible for queue-scope / queue-area clicks run
    /// `normalize_stale_browse_destination` first and must NOT consult this
    /// for those non-browse surfaces, which stay live during the one-frame
    /// stale window. `browse_destination` is `None` only on the
    /// pre-first-render default (zero-area) layout, which carries no browse
    /// surface, so there is nothing stale to guard (treated as current).
    pub(super) fn is_browse_layout_current(&self) -> bool {
        match self.layout.main.browse_destination {
            Some(tag) => tag == self.tab,
            None => true,
        }
    }

    /// Whether browse mouse handling for the currently selected destination
    /// may proceed.
    ///
    /// A selected Emby/Audiobookshelf library index that no longer exists
    /// normalizes to Home and returns `false`, stopping the gesture entirely
    /// (the triggering operation performs no destination-specific action).
    /// Otherwise the gesture is a no-op unless the installed completed-frame
    /// layout was rendered for that destination: before a frame for the
    /// selected tab redraws, the layout's hit targets describe the previous
    /// destination and must not be interpreted (design §4). See
    /// `is_browse_layout_current` for the tag-only check used when some
    /// surfaces (the queue) must stay live through that window.
    pub(super) fn browse_mouse_ready(&mut self) -> bool {
        if self.normalize_stale_browse_destination() {
            return false;
        }
        self.is_browse_layout_current()
    }

    /// Map a column click to a left-panel tab index (0 = Home, 1+ = library),
    /// scroll-aware: returns `usize::MAX - 1` for a click on the `«` arrow
    /// and `usize::MAX` for a click on the `»` arrow (see `handle_mouse`).
    pub(super) fn tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout.tabs_area;
        if col < area.x || col >= area.x + area.width {
            return None;
        }
        let rel = col - area.x;
        let (vis_start, vis_end) = self.visible_tab_range(area.width);
        let has_left = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let left_w: u16 = if has_left { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left && rel < left_w {
            return Some(usize::MAX - 1);
        }
        if has_right && rel >= area.width - right_w {
            return Some(usize::MAX);
        }
        let rel = rel - left_w;
        let widths = self.tab_title_widths();
        let pad = 1u16;
        let mut x = 0u16;
        for (i, &w) in widths
            .iter()
            .enumerate()
            .skip(vis_start)
            .take(vis_end - vis_start)
        {
            let end = x + pad + w + pad;
            if rel < end {
                return Some(i);
            }
            x = end;
        }
        None
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
                self.set_queue_scope(QueueScope::Local);
                return true;
            }
            if self
                .layout
                .main
                .queue_scope_remote_area
                .contains((col, row).into())
            {
                self.set_queue_scope(QueueScope::Remote);
                return true;
            }
        }
        // Click in queue area: focus queue and move cursor.
        let qa = self.layout.main.queue_area;
        if qa.contains((col, row).into()) {
            if !matches!(self.panel_focus, PanelFocus::Queue) {
                self.last_card_height = 0;
                self.last_card_width = 0;
            }
            self.set_panel_focus(PanelFocus::Queue);
            let content_y = (row - qa.y) as usize;
            if let Some(&Some(item_idx)) = self.layout.main.queue_row_map.get(content_y) {
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
            // Click on the inline hero: same as clicking anywhere else in
            // the library pane -- a single click only focuses (the cursor
            // is already on the selected item, so there's nothing else to
            // move). Activation (playing a movie, entering a Series'
            // season/episode selection) is a double-click gesture, handled
            // in `handle_mouse`'s `is_double` branch alongside every other
            // library-row activation, so it can't drift from Enter's
            // behavior or from the app-wide "single click only focuses"
            // convention.
            if self.layout.main.hero_area.contains((col, row).into()) {
                // The hero is a browse surface for the two Services that can
                // publish it (Emby and Audiobookshelf); match positively
                // rather than excluding Home/Feeds.
                match self.tab {
                    TabSelection::EmbyLibrary(_) | TabSelection::AudiobookshelfLibrary(_) => {
                        self.set_panel_focus(PanelFocus::Library);
                        return true;
                    }
                    TabSelection::Home | TabSelection::Feeds => {}
                }
            }
            // Wide Music: right-pane clicks (pills + album browser) bypass
            // the left_area gate because the right pane is a physically
            // separate rect. Track hits in the left pane flow through the
            // existing left_area block (wide left pane IS left_area).
            if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
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
                        let click_y = (row - ra.y) as usize;
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
                if !matches!(self.panel_focus, PanelFocus::Library) {
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
                    TabSelection::Feeds => {
                        // Feeds tab: pill-bar click and row click.
                        let target = self
                            .layout
                            .main
                            .selector_tabs
                            .iter()
                            .copied()
                            .find(|(rect, _)| rect.contains((col, row).into()))
                            .map(|(_, target)| target);
                        if let Some(target) = target {
                            self.feed_tab_select_group(target);
                            return true;
                        }
                        let click_y = (row - la.y) as usize;
                        let use_row_map = !self.layout.main.left_row_map.is_empty();
                        let row_map_item = if use_row_map {
                            self.layout.main.left_row_map.get(click_y).copied()
                        } else {
                            None
                        };
                        let n = self.feed_tab.visible_entries().len();
                        if let Some(Some(item_idx)) = row_map_item {
                            if item_idx < n {
                                self.feed_tab.cursor = item_idx;
                            }
                        } else if use_row_map {
                            // No row map entry; cursor unchanged.
                            return false;
                        } else {
                            let visible = la.height as usize;
                            let offset = if self.feed_tab.cursor >= visible {
                                self.feed_tab.cursor - visible + 1
                            } else {
                                0
                            };
                            let clicked = offset + click_y;
                            if clicked < n {
                                self.feed_tab.cursor = clicked;
                            }
                        }
                    }
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
}
