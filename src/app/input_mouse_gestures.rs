//! Per-surface mouse-gesture handlers extracted from `App::handle_mouse`
//! (`input_mouse_dispatch.rs`).
//!
//! `handle_mouse` keeps the four `browse_mouse_ready()` guards and the thin
//! `match self.tab { … }` routers; each arm delegates to exactly one method
//! here, named `handle_mouse_<gesture>_<surface>`. This granularity is
//! deliberate: the twelve *Mouse geometry* agents each own one method and
//! never touch a shared dispatch match.

use super::action::Command;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{App, QueueScope, TabSelection};
use crate::app::components::msg::TvHit;
use ratatui::layout::Position;

impl App {
    // ---- selector-tab click (match at input_mouse_dispatch.rs :203) ----

    // ---- Queue mouse geometry (task 5.3d) ----

    pub(super) fn handle_mouse_selector_click_queue(&mut self, scope: QueueScope) {
        self.set_queue_scope(scope);
    }

    // ---- TV workspace mouse geometry (task 5.3d) ----

    /// Single-click select in the TV workspace (tvshows shell arm). The
    /// component resolved the pane + hit; this applies the App-side effects
    /// the legacy `click_set_cursor` TV branches performed: Episodes-pane
    /// hits (episode rows and season pills) pull panel focus to the Library,
    /// Series-pane hits move the library cursor, blank Episodes-pane space
    /// is consumed.
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

    /// Double-click activate in the wide hero-on-left layout, shared by the
    /// TV workspace and wide Emby podcast libraries (which render the same
    /// panes but mount no component, so they still route through
    /// `handle_mouse`). Episode rows and series rows activate the selected
    /// series; season pills and blank space no-op. tvshows clicks arrive
    /// from the `TvClick` shell arm with the hit already resolved; this
    /// method keeps the geometry hit-test because the legacy podcast path
    /// needs it — one implementation for both paths.
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

    /// Right-click context menu in the TV workspace: applies the same
    /// pane-appropriate single-click effect as a left click (via the
    /// caller-resolved `hit`), then opens the context menu at the click —
    /// mirroring the legacy `click_set_cursor`-then-menu flow.
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

    /// Single click on the wide hero-on-left panes, shared by the legacy
    /// podcast path in `click_set_cursor` and the tvshows `SeriesRow` arm:
    /// consumes clicks in the left pane, resolves a right-pane click to the
    /// library cursor via the rendered row map. `click_set_cursor` still
    /// needs this for wide Emby podcast libraries, which render the same
    /// panes but mount no component.
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

    pub(super) fn handle_mouse_selector_click_audiobookshelf(
        &mut self,
        index: usize,
        target: usize,
    ) {
        // Deliberately asymmetric with the double-click arm: this binds the
        // kind via `match` and falls through to the podcast bucket on `None`
        // (it does not early-return). Do not unify with
        // `handle_mouse_double_click_audiobookshelf`.
        match self.audiobookshelf_kind_at(index) {
            Some(AudiobookshelfBrowseKind::Book) => {
                self.select_audiobookshelf_book_bucket(target);
            }
            _ if self.podcast_filter_target_active(index) => {
                self.select_audiobookshelf_filter(target);
            }
            _ => self.select_audiobookshelf_podcast_bucket(target),
        }
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

    pub(super) fn handle_mouse_selector_click_feeds(&mut self) {}

    // ---- double-click activate (match at input_mouse_dispatch.rs :295) ----
    //
    // Each method returns `true` when it has performed the equivalent of the
    // original arm's `return` from `handle_mouse` (skipping the trailing
    // wide-music right-pane block); `false` lets `handle_mouse` fall through
    // to that block and its own final `return`.

    pub(super) fn handle_mouse_double_click_home(&mut self, in_left: bool) {
        if in_left {
            self.home_play();
        }
    }

    pub(super) fn handle_mouse_double_click_feeds(&mut self) {
        // Double-click on Feeds: no-op (playback wiring pending).
    }

    pub(super) fn handle_mouse_double_click_audiobookshelf(
        &mut self,
        index: usize,
        in_left: bool,
        pos: Position,
    ) -> bool {
        // Binds the kind via `let Some(kind) = … else { return; }` and
        // early-returns on `None` — deliberately asymmetric with
        // `handle_mouse_selector_click_audiobookshelf`.
        let Some(kind) = self.audiobookshelf_kind_at(index) else {
            return true;
        };
        match kind {
            AudiobookshelfBrowseKind::Podcast => {
                if in_left {
                    let in_episodes = self
                        .audiobookshelf_browse
                        .get(index)
                        .is_some_and(|state| state.episode_selection.is_some());
                    if !in_episodes {
                        if self.layout.main.is_wide_podcast_active() {
                            self.enter_audiobookshelf_episode_selection();
                        } else {
                            self.open_podcast_selection_modal();
                        }
                    } else {
                        // Episode activation: inert seam for
                        // #518 (double-click on a selected
                        // episode).
                        self.activate_audiobookshelf_episode(index);
                    }
                }
            }
            AudiobookshelfBrowseKind::Book => {
                if !self.layout.main.is_wide_book_active() && in_left {
                    self.activate_audiobookshelf_book_parent();
                } else {
                    let in_chapters = self
                        .audiobookshelf_book_browse
                        .get(index)
                        .is_some_and(|state| state.chapter_selection.is_some());
                    if in_chapters
                        && self
                            .layout
                            .main
                            .audiobookshelf_book_chapter_rows
                            .iter()
                            .any(|(rect, _)| rect.contains(pos))
                    {
                        self.activate_audiobookshelf_book_row();
                    }
                }
            }
        }
        false
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

    // ---- right-click context menu (match at input_mouse_dispatch.rs :497) ----

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
