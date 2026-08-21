mod album;
mod album_art;
mod album_cursor;
mod album_detail;
mod album_plan;
mod album_rows;
mod audiobookshelf;
mod audiobookshelf_book_browser;
mod audiobookshelf_books;
mod card;
mod chrome;
mod chrome_player;
mod chrome_seekbar;
mod chrome_status;
mod chrome_tabs;
mod detail;
mod detail_series;
mod detail_series_view;
mod feeds;
mod hero;
mod hero_left;
mod home;
mod home_feed;
mod home_hero;
mod home_latest_row;
mod home_list_rows;
mod home_pills;
mod home_video;
pub mod indicators;
mod list;
mod list_letter_groups;
mod list_plain;
mod list_rows;
mod movies_wide;
mod music;
mod music_wide;
mod music_wide_browser;
mod overlays;
mod pills;
mod queue;
mod search_sidebar;
mod sort_filter;
mod tv_wide;
mod visualizer;
mod widgets;

// Re-exports so paths that resolved at `render::X` (or, from render's other
// submodules, `super::X`) before the render/mod.rs split (issue #365 step 2,
// lane C) keep resolving without editing any call site. The `pub(crate)`
// trio is referenced from outside `render` entirely (src/app/mod.rs,
// src/app/actions.rs); the rest are referenced via `super::X` from render's
// sibling submodules (album, card, detail, home, list, music, pills, queue)
// and/or `use super::*` in render/tests.rs.
pub(super) use album_plan::sorted_group_album_order;
use chrome::LIST_PLAY_ICON;
use list_rows::{selection_marker, MarkerEdge};
pub(super) use sort_filter::{
    effective_sort_str, letter_bucket, parse_album_folder_name, strip_article,
};
pub(crate) use sort_filter::{initial_group_artist_sort_key, LetterFilter, LIBRARY_PILL_THRESHOLD};
use widgets::{
    content_width, render_count_label, render_pill_bar, render_placeholder,
    render_queue_panel_frame, render_right_scrollbar, render_right_scrollbar_with_viewport,
    render_scrollbar_with_viewport_at, render_selected_block_background,
    render_selected_block_borders, right_panel_content_area, PillBar, SelectedBlockBorderStyle,
    COLUMN_GAP, MUSIC_ALBUM_IMAGE_TYPES, RENDER_FILTER,
};

use super::ui_util::natural_sort_key;
use super::{layout::AppLayout, palette, App, PanelFocus, PanelMode, TabSelection};
use crate::app::layout::{LayoutMain, LayoutPlayback};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use std::time::Instant;

// Test-only: these names are otherwise unused in the production build (their
// only production callers moved into chrome.rs/widgets.rs, which
// import them directly), but render/tests.rs still reaches them via
// `use super::*`.
#[cfg(test)]
use mbv_core::api::TICKS_PER_SECOND;
#[cfg(test)]
use unicode_width::UnicodeWidthStr;

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
pub(super) const TAB_BAR_BOX_HEIGHT: u16 = 3;

impl App {
    pub(super) fn now_playing_throbber_span(&self) -> Span<'static> {
        const FRAMES: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
        Span::styled(
            FRAMES[self.now_playing_throbber_index % FRAMES.len()],
            Style::default().fg(palette::AQUA),
        )
    }

    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        // Guard against zero-dimension terminal (e.g. minimized or piped).
        // `self.layout` is left untouched here -- it still reflects the last
        // frame that rendered in full.
        if area.width == 0 || area.height == 0 {
            return;
        }
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;
        if self.clamp_queue_column_width() {
            self.save_prefs();
        }

        // Set the dim flag from the modal state before any content is drawn,
        // so every image lookup and fetch trigger in this frame sees the right
        // mem-key. `render_modal_frame` resets it on entry so it stays accurate
        // for the receiver between frames too.
        self.dim_backdrop_active = self.any_dim_modal_open();

        // Every render sub-call below writes into this fresh, local value
        // instead of `self.layout` directly. It's swapped into `self.layout`
        // in one atomic assignment only once this pass completes in full, so
        // an early return partway through (like the guard above) can never
        // leave `self.layout` holding a mix of fields from two different
        // frames.
        let mut layout = AppLayout::default();

        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        let playing_panel = show_controls;
        // Always reserve the player rows (title + controls) so
        // that content doesn't shift when the player appears or disappears.
        let (seek_h, _gap_h, title_h, controls_h): (u16, u16, u16, u16) = (1, 0, 1, 2);
        let player_h = seek_h + title_h + controls_h;
        let [main_area] = Layout::vertical([Constraint::Min(0)]).areas(area);

        layout.playback.ind_mu = Rect::default();
        layout.playback.ind_rc = Rect::default();
        layout.playback.ind_vol = Rect::default();
        layout.tabs_area = Rect::default();

        // Clear expired toast before any rendering so the status bar sees the latest state.
        if self.status_expires.is_some_and(|t| t <= Instant::now()) {
            self.status.clear();
            self.status_expires = None;
            self.status_severity = super::notify_actions::ToastSeverity::default();
            self.force_clear = true;
        }

        let now_playing: Option<String> = if active {
            let idx = self.player.status.lock().unwrap().current_idx;
            let queue = self.playback_queue();
            queue.item_at(idx).map(|item| item.title().to_string())
        } else {
            None
        };
        let title_color = palette::PLAYBACK_CONTENT_FG;
        let now_playing_title: Option<(String, Color)> = if playing_panel {
            if active {
                now_playing.map(|t| (t, title_color))
            } else if let Some(ref state) = self.connected_session_state {
                state.now_playing.clone().map(|t| (t, title_color))
            } else {
                None
            }
        } else {
            None
        };
        // Render dispatch (issue #275; folded into a single unconditional
        // call by #361 commit 2, since the deleted Standard view was the
        // only other arm).
        self.render_main(
            f,
            main_area,
            &mut layout.main,
            &mut layout.playback,
            &mut layout.tabs_area,
            player_h,
            show_controls,
            &now_playing_title,
        );

        self.render_context_menu(f, &mut layout);

        debug_assert!(
            self.context_menu.is_none() || !self.any_other_modal_open(),
            "context menu must be the only modal or sidebar surface"
        );

        let panel_area = (layout.main.panel_area.width > 0).then_some(layout.main.panel_area);
        if self.show_sessions {
            self.render_sessions_overlay(f, panel_area);
        }
        if self.show_playlists {
            self.render_playlists_panel(f, panel_area);
        }
        if self.search_sidebar.is_some() {
            self.render_search_sidebar(f, panel_area);
        }
        if self.show_help {
            self.render_help_panel(f, panel_area);
        }
        if self.show_settings {
            self.render_settings_panel(f, &mut layout, panel_area);
            if self.multiselect_popup.is_some() {
                self.render_multiselect_popup(f);
            }
            if self.library_routes_popup.is_some() {
                self.render_library_routes_popup(f);
            }
            if self.feeds_manage_popup.is_some() {
                self.render_feeds_manage_popup(f);
            }
        }
        if self.save_playlist_dialog.is_some() {
            self.render_save_playlist_dialog(f);
        }
        debug_assert!(
            !(self.save_playlist_dialog.is_some() && self.confirm_modal.is_some()),
            "save_playlist_dialog and confirm_modal must not both be active — \
             the backdrop would be dimmed twice"
        );
        if self.confirm_modal.is_some() {
            self.render_confirm_modal(f);
        }
        if self.remote_reanchor_popup.is_some() {
            self.render_remote_reanchor_popup(f);
        }
        if self.daemon_lost_modal.is_some() {
            self.render_daemon_lost_modal(f);
        }

        // Record the destination this completed frame was rendered for, on
        // the layout about to be installed. The tag is set only here (never
        // on the intermediate/draft `layout`), so browse mouse handling can
        // tell whether the installed layout describes the currently selected
        // destination and refuse to interpret stale geometry otherwise
        // (design §4). `self.tab` was already normalized to a live
        // destination inside `render_main`.
        layout.main.browse_destination = Some(self.tab);

        // One atomic replace, reached only once the full pass above has
        // completed -- `self.layout` never observes a half-updated frame.
        self.layout = layout;
    }

    fn any_dim_modal_open(&self) -> bool {
        self.context_menu.is_some()
            || self.confirm_modal.is_some()
            || self.daemon_lost_modal.is_some()
            || self.remote_reanchor_popup.is_some()
            || self.multiselect_popup.is_some()
            || self.save_playlist_dialog.is_some()
            || self.library_routes_popup.is_some()
    }

    pub(super) fn any_other_modal_open(&self) -> bool {
        self.confirm_modal.is_some()
            || self.daemon_lost_modal.is_some()
            || self.remote_reanchor_popup.is_some()
            || self.save_playlist_dialog.is_some()
            || self.multiselect_popup.is_some()
            || self.library_routes_popup.is_some()
            || self.show_help
            || self.show_settings
            || self.show_sessions
            || self.show_playlists
            || self.search_sidebar.is_some()
    }
}

impl App {
    fn render_main(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutMain,
        playback: &mut LayoutPlayback,
        tabs_area_out: &mut Rect,
        player_h: u16,
        show_controls: bool,
        now_playing_title: &Option<(String, Color)>,
    ) {
        if area.height < 4 {
            return;
        }
        // Apply the tab saved from the previous session once libs have loaded.
        if self.library_tab_pending > 0
            && (!self.libs.is_empty() || !self.audiobookshelf_libraries.is_empty())
        {
            let fp = self.feeds_tab_pos();
            let emby = self.libs.len();
            let audio = self.audiobookshelf_libraries.len();
            let max_pos = fp.unwrap_or(emby + audio);
            let pos = self.library_tab_pending.min(max_pos);
            self.tab = TabSelection::from_position_with_counts(pos, emby, audio, fp.is_some());
            self.library_tab_pending = 0;
        }
        // A selected Service library index that no longer exists (async
        // Service removal/replacement) becomes Home. Home needs no
        // Service-specific library state, so we keep rendering the (now)
        // Home view instead of aborting to a blank, mostly-default frame:
        // the completed frame installs a full Home layout tagged Home below.
        self.normalize_stale_browse_destination();

        // Left panel (card + queue) | Right panel (library, remaining).
        let left_w = match self.effective_panel_mode() {
            PanelMode::Both => self.queue_column_width,
            PanelMode::LibraryOnly => 0,
            PanelMode::QueueOnly => area.width,
        };
        let right_w = area.width.saturating_sub(left_w);
        let right_visible = self.effective_panel_mode() != PanelMode::QueueOnly;

        // Header row removed — the tab bar above indicates current location.
        layout.breadcrumbs = Vec::new();
        layout.selector_tabs = Vec::new();
        let content_h = area.height;
        let left_area = if self.effective_panel_mode() == PanelMode::LibraryOnly {
            Rect::default()
        } else {
            Rect {
                x: area.x,
                y: area.y,
                width: left_w,
                height: content_h,
            }
        };
        layout.panel_area = if self.terminal_width < super::MINI_VIEW_THRESHOLD
            && self.effective_panel_mode() == PanelMode::LibraryOnly
        {
            area
        } else {
            left_area
        };
        layout.panel_content_area = Self::left_panel_content_area(layout.panel_area);

        // The queue panel always renders with focused styling whenever it
        // holds real panel focus — including queue-only mode and mini view.
        // (Previously queue-only forced unfocused styling, which at narrow
        // widths made the sole visible panel look unresponsive.)
        let queue_focused = matches!(self.effective_panel_focus(), PanelFocus::Queue);
        let left_focused = !queue_focused;

        // Full-column background behind the card image and queue list.
        if self.effective_panel_mode() != PanelMode::LibraryOnly {
            let left_bg = palette::resolve_surface_focus(queue_focused);
            f.render_widget(
                Block::default().style(Style::default().bg(left_bg)),
                left_area,
            );
        }

        // Full-column background for the right panel (tabs, player, library, queue, status).
        let right_full_area = Rect {
            x: area.x + left_w + COLUMN_GAP,
            y: area.y,
            width: right_w.saturating_sub(COLUMN_GAP),
            height: area.height,
        };
        if right_visible {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
                right_full_area,
            );
        }

        // Inner content area with padding inside the colored box (queue uses this).
        let left_content = Rect {
            x: left_area.x + 2,
            y: left_area.y + 1,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(2),
        };
        let card_area = Rect {
            x: left_area.x + 2,
            y: left_area.y + 1,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(2),
        };

        let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
        let right_area = Rect {
            x: area.x + left_w + COLUMN_GAP,
            y: area.y + tab_h + player_h,
            width: right_w.saturating_sub(COLUMN_GAP),
            height: content_h
                .saturating_sub(1)
                .saturating_sub(tab_h)
                .saturating_sub(player_h),
        };

        // Tab bar at the very top of the right column.
        let tab_area = Rect {
            x: right_area.x,
            y: area.y,
            width: right_area.width,
            height: tab_h,
        };
        if right_visible {
            self.render_tabs(f, tab_area, tabs_area_out);
        }

        // Player panel below the tab bar.
        if right_visible && player_h > 0 {
            let player_area = Rect {
                x: right_area.x,
                y: area.y + tab_h,
                width: right_area.width,
                height: player_h,
            };
            let playback_panel_bg = if queue_focused {
                palette::SURFACE_FOCUSED
            } else {
                palette::SURFACE_PLAYBACK
            };
            self.render_player_panel(
                f,
                player_area,
                playback,
                player_h,
                show_controls,
                now_playing_title,
                playback_panel_bg,
            );
        }

        // Status bar sits at the bottom of the right panel only.
        let status_area = Rect {
            x: right_area.x,
            y: right_area.y + right_area.height,
            width: right_area.width,
            height: 1,
        };

        let (lib_area, queue_area) = if self.effective_panel_mode() == PanelMode::LibraryOnly {
            (right_area, Rect::default())
        } else {
            // The card fills the top of the left column; the queue list takes
            // the rows below it. Short terminals keep that same structure.
            let is_queue_only = self.effective_panel_mode() == PanelMode::QueueOnly;
            let is_wide = is_queue_only && left_area.width >= 100;
            let (card_h, card_w, _) = self.render_card(f, card_area, is_wide);
            let mut left_remaining = left_content.height.saturating_sub(card_h);

            // Queue-only mode has no right column, so the playback panel
            // (seekbar + title + controls) renders here instead: stacked
            // below the card on narrow terminals, or beside it on wide ones.
            let mut narrow_player_h = 0;
            if is_queue_only {
                if is_wide {
                    let panel_area = Rect {
                        x: card_area.x + card_w + 2,
                        y: card_area.y,
                        width: left_content.width.saturating_sub(card_w + 2),
                        height: card_h,
                    };
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
                        panel_area,
                    );
                    self.render_player_panel(
                        f,
                        panel_area,
                        playback,
                        player_h,
                        show_controls,
                        now_playing_title,
                        palette::SURFACE_CHROME,
                    );
                } else {
                    let panel_area = Rect {
                        x: left_content.x,
                        y: left_content.y + card_h,
                        width: left_content.width,
                        height: player_h,
                    };
                    self.render_player_panel(
                        f,
                        panel_area,
                        playback,
                        player_h,
                        show_controls,
                        now_playing_title,
                        palette::SURFACE_CHROME,
                    );
                    narrow_player_h = player_h;
                    left_remaining = left_remaining.saturating_sub(player_h);
                }
            }

            let queue_h = left_remaining.saturating_sub(1);
            (
                right_area,
                Rect {
                    y: left_content.y + card_h + narrow_player_h + 1,
                    height: queue_h,
                    ..left_content
                },
            )
        };

        // Apply the shared horizontal padding once here, at the single point
        // where the tab content area is finalized, so every tab kind (and the
        // music-group pills row below) inherits consistent left/right gutters
        // instead of each renderer inventing its own. When the left column is
        // collapsed the user has asked to reclaim maximum width, so the gutters
        // are dropped and the library spans the panel edge-to-edge.
        let lib_area =
            right_panel_content_area(lib_area, self.effective_panel_mode() != PanelMode::Both);
        let render_lib_area = lib_area;
        // Both letter-range pills (large non-music libraries) and the
        // narrow music-group selector render inside `render_list` itself
        // now, below the hero (`list.rs`), unified with every other
        // inline browser's pill placement (design.md decision 6: pill
        // *position* is geometry, not a per-screen declaration) -- not
        // carved out of `lib_area` here. Wide grouped Music is the one
        // exception: its pills sit in the hero-on-left right rail instead
        // (`render_wide_music_group`), which `list.rs` still branches to
        // internally before reaching the inline presentation path.

        if self.effective_panel_mode() != PanelMode::LibraryOnly {
            let queue_list_area = render_queue_panel_frame(f, queue_area, queue_focused);
            let qla = queue_list_area;

            // Title and status bar panels float within the queue panel with
            // 2 columns left/right indent and 1 row of space on all sides.
            let title_overhead = if qla.height >= 4 { 3 } else { 0 };
            let status_overhead = if qla.height >= title_overhead + 4 {
                3
            } else {
                0
            };

            if title_overhead > 0 {
                self.render_queue_title(
                    f,
                    Rect {
                        x: qla.x + 2,
                        y: qla.y + 1,
                        width: qla.width.saturating_sub(4),
                        height: 1,
                    },
                    layout,
                );
            }
            let queue_content_area = Rect {
                y: qla.y + title_overhead,
                height: qla.height.saturating_sub(title_overhead + status_overhead),
                ..qla
            };
            self.render_queue(f, queue_content_area, queue_focused, layout);

            if status_overhead > 0 {
                let pill_row = Rect {
                    x: qla.x + 2,
                    y: qla.y + qla.height.saturating_sub(2),
                    width: qla.width.saturating_sub(4),
                    height: 1,
                };
                let pill_bg = palette::SURFACE_CHROME;
                f.render_widget(
                    Block::default().style(Style::default().bg(pill_bg)),
                    pill_row,
                );
                f.render_widget(
                    Paragraph::new(Line::from(self.playlist_status_spans())),
                    pill_row,
                );
                if let Some(autosave_spans) = self.autosave_status_spans() {
                    let autosave_w = Self::status_width(&autosave_spans);
                    let autosave_x = pill_row.x + pill_row.width.saturating_sub(autosave_w);
                    if autosave_x > pill_row.x {
                        f.render_widget(
                            Paragraph::new(Line::from(autosave_spans)),
                            Rect {
                                x: autosave_x,
                                y: pill_row.y,
                                width: autosave_w,
                                height: 1,
                            },
                        );
                    }
                }
            }
        }
        if right_visible {
            self.render_library(f, render_lib_area, left_focused, layout);
        }

        // Status bar + toast overlay at the bottom of the right panel.
        if status_area.width > 0 {
            self.render_status_bar(f, status_area, playback, false);
            // Neutral toasts always render in-app (spec: "Neutral toasts …
            // SHALL always render in-app"). Prompts (status_expires == None)
            // and non-system-notification fallback toasts follow the existing
            // gating: visible only when the desktop notification path isn't
            // working (system notifications disabled or failed).
            let is_neutral_toast = self.status_expires.is_some()
                && self.status_severity == super::notify_actions::ToastSeverity::Neutral;
            let show_toast = !self.status.is_empty()
                && (is_neutral_toast || !self.system_notifications || self.notif_failed);
            if show_toast {
                // Prompts (no expiry) and Neutral toasts use standard
                // status-bar styling; colored toasts use severity palette.
                let styled_as_status_bar = self.status_expires.is_none()
                    || self.status_severity == super::notify_actions::ToastSeverity::Neutral;
                let (toast_bg, toast_fg) = if styled_as_status_bar {
                    (palette::SURFACE_CHROME, palette::TEXT)
                } else {
                    (self.status_severity.toast_bg(), palette::TOAST_FG)
                };
                f.render_widget(Clear, status_area);
                f.render_widget(
                    Paragraph::new(Self::toast_line(&self.status, toast_fg))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(toast_fg).bg(toast_bg)),
                    status_area,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "tests_album_focus.rs"]
mod album_focus_tests;
#[cfg(test)]
#[path = "tests_album_listing.rs"]
mod album_listing_tests;
#[cfg(test)]
#[path = "tests_audiobookshelf_books.rs"]
mod audiobookshelf_book_tests;
#[cfg(test)]
#[path = "tests_music_groups.rs"]
mod music_group_tests;
#[cfg(test)]
#[path = "tests_non_music.rs"]
mod non_music_tests;
#[cfg(test)]
#[path = "tests_panel.rs"]
mod panel_tests;
#[cfg(test)]
#[path = "tests_queue.rs"]
mod queue_tests;
#[cfg(test)]
#[path = "tests_scroll_pills.rs"]
mod scroll_pills_tests;
#[cfg(test)]
#[path = "test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
mod tests_audiobookshelf_podcasts;
#[cfg(test)]
mod tests_feeds;
#[cfg(test)]
mod tests_home_inline;
