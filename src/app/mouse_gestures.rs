//! Shell-invoked mouse effect handlers for migrated interactive surfaces.

use crate::app::action::Command;
use crate::app::components::msg::TvHit;
use crate::app::{App, QueueScope, TabSelection};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerCommand;
use mbv_core::remote_reconciliation::RemoteIntent;
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

    /// Destination-agnostic wheel scroll for the remaining legacy browse
    /// surfaces (Emby library rows; Home, Audiobookshelf, and Feeds are
    /// no-ops). Home no longer routes here: its wheel is claimed by
    /// `HomeComponent` and handled at the Model boundary
    /// (`Model::handle_home_scroll`), which keeps the same throttle/readiness
    /// gates and the Continue Watching `cw_move_cursor` quirk (task 5.3d,
    /// Home wheel-scroll ownership).
    pub(super) fn handle_mouse_scroll_browse(&mut self, delta: i64) {
        if !self.browse_mouse_ready() {
            return;
        }
        match self.tab {
            TabSelection::EmbyLibrary(lib_idx) => self.move_lib_cursor(lib_idx, delta),
            TabSelection::Home | TabSelection::AudiobookshelfLibrary(_) | TabSelection::Feeds => {}
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

    pub(super) fn handle_mouse_scroll_queue(&mut self, delta: i64) {
        let n = self.displayed_queue().total_queue_len();
        if n > 0 {
            let queue = self.displayed_queue_mut();
            queue.queue_cursor = super::ui_util::move_cursor(queue.queue_cursor, delta * 3, n);
        }
    }

    pub(super) fn handle_mouse_single_click_emby(&mut self, lib_idx: usize, target: usize) {
        self.set_panel_focus(super::PanelFocus::Library);
        if let Some(level) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if target < level.items.len() {
                level.cursor = target;
                self.save_default_library_position(lib_idx);
            }
        }
    }

    pub(super) fn handle_mouse_single_click_queue(
        &mut self,
        slot_id: Option<mbv_core::playback_queue::QueueSlotId>,
    ) -> bool {
        self.set_panel_focus(super::PanelFocus::Queue);
        let Some(slot_id) = slot_id else { return false };
        if let Some(index) = self
            .displayed_queue()
            .queue
            .slots()
            .iter()
            .position(|slot| slot.slot_id == slot_id)
        {
            self.mark_queue_cursor_user_active();
            self.displayed_queue_mut().queue_cursor = index;
            return true;
        }
        false
    }

    pub(super) fn handle_mouse_selector_click_queue(&mut self, scope: QueueScope) {
        self.set_queue_scope(scope);
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

    pub(super) fn handle_mouse_double_click_emby(&mut self, lib_idx: usize, target: usize) {
        self.handle_mouse_single_click_emby(lib_idx, target);
        if self.is_viewing_album_folders(lib_idx) {
            self.activate_album_folder_row(lib_idx);
        } else if !self.activate_selected_series(lib_idx) {
            self.select(lib_idx);
        }
    }

    pub(super) fn handle_mouse_double_click_queue(
        &mut self,
        slot_id: Option<mbv_core::playback_queue::QueueSlotId>,
    ) {
        if self.handle_mouse_single_click_queue(slot_id) {
            self.dispatch(Command::QueuePlayCursor);
        }
    }

    pub(super) fn handle_mouse_right_click_emby(
        &mut self,
        lib_idx: usize,
        target: usize,
        col: u16,
        row: u16,
    ) {
        self.handle_mouse_single_click_emby(lib_idx, target);
        // Emby-library right-click is never a Home-tab menu, so the
        // Continue-Watching-selected fact is a harmless `false` (the
        // `self.tab.is_home()` guard short-circuits it).
        self.open_context_menu_at(col, row, false);
    }

    pub(super) fn handle_mouse_right_click_queue(
        &mut self,
        slot_id: Option<mbv_core::playback_queue::QueueSlotId>,
        col: u16,
        row: u16,
        home_cw_selected: bool,
    ) {
        self.handle_mouse_single_click_queue(slot_id);
        self.open_context_menu_at(col, row, home_cw_selected);
    }

    pub(super) fn handle_mouse_single_click_tv(&mut self, lib_idx: usize, hit: TvHit) {
        match hit {
            TvHit::SeasonTab(_) | TvHit::EpisodeRow(_) => {
                self.set_panel_focus(super::PanelFocus::Library);
            }
            TvHit::SeriesRow(target) => {
                // The component resolved the series under the click; apply it
                // to `App`'s library cursor before any further pane effect.
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.cursor = target;
                }
            }
            TvHit::EpisodesPane => {}
        }
    }

    pub(super) fn handle_mouse_double_click_tv(&mut self, lib_idx: usize, hit: TvHit) {
        if let TvHit::SeriesRow(target) = hit {
            // Apply the clicked series before activating (the click may land
            // on a series other than the focused one).
            if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                level.cursor = target;
            }
        }
        if matches!(hit, TvHit::EpisodeRow(_) | TvHit::SeriesRow(_)) {
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
        self.handle_mouse_single_click_tv(lib_idx, hit);
        // TV-workspace right-click is never a Home-tab menu, so the
        // Continue-Watching-selected fact is a harmless `false`.
        self.open_context_menu_at(col, row, false);
    }
}
