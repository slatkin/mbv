mod album;
mod card;
mod chrome;
mod detail;
mod home;
pub mod indicators;
mod list;
mod music;
mod overlays;
mod pills;
mod power_widgets;
mod queue;
mod sort_filter;

// Re-exports so paths that resolved at `render::X` (or, from render's other
// submodules, `super::X`) before the render/mod.rs split (issue #365 step 2,
// lane C) keep resolving without editing any call site. The `pub(crate)`
// trio is referenced from outside `render` entirely (src/app/mod.rs,
// src/app/actions.rs); the rest are referenced via `super::X` from render's
// sibling submodules (album, card, detail, home, list, music, pills, queue)
// and/or `use super::*` in render/tests.rs.
use chrome::play_icon;
use power_widgets::{
    build_power_queue_rows, power_content_width, power_right_panel_content_area, render_pill_bar,
    render_power_count_label, render_power_placeholder, render_power_queue_panel_frame,
    render_power_right_scrollbar, render_power_right_scrollbar_with_viewport,
    render_power_scrollbar, render_selected_block_background, render_selected_block_borders,
    rendered_power_queue_rows_for_padding, selection_marker, selector_pill_style, PillBar,
    PillUnderlay, MUSIC_ALBUM_IMAGE_TYPES, POWER_RENDER_FILTER, POWER_VIEW_GAP,
};
use sort_filter::{effective_sort_str, letter_bucket, parse_album_folder_name, strip_article};
pub(crate) use sort_filter::{initial_group_artist_sort_key, LetterFilter, LIBRARY_PILL_THRESHOLD};

use super::ui_util::natural_sort_key;
use super::{layout::AppLayout, palette, App, PanelFocus};
use crate::app::layout::{LayoutMain, LayoutPlayback};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use std::time::Instant;

// Test-only: these names are otherwise unused in the production build (their
// only production callers moved into chrome.rs/power_widgets.rs, which
// import them directly), but render/tests.rs still reaches them via
// `use super::*`.
#[cfg(test)]
use mbv_core::api::TICKS_PER_SECOND;
#[cfg(test)]
use power_widgets::{render_power_scrollbar_with_viewport, POWER_TAB_LEFT_PAD};
#[cfg(test)]
use ratatui::style::Modifier;
#[cfg(test)]
use ratatui::text::Span;
#[cfg(test)]
use unicode_width::UnicodeWidthStr;

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
pub(super) const TAB_BAR_BOX_HEIGHT: u16 = 3;

impl App {
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
        // Power View always reserves the player rows (title + controls) so
        // that content doesn't shift when the player appears or disappears.
        let (seek_h, _gap_h, title_h, controls_h): (u16, u16, u16, u16) = (1, 0, 1, 2);
        let player_h = seek_h + title_h + controls_h;
        let [main_area] = Layout::vertical([Constraint::Min(0)]).areas(area);

        layout.playback.ind_mu = Rect::default();
        layout.playback.ind_rc = Rect::default();
        layout.tabs_area = Rect::default();
        layout.tabbar_vol_area = Rect::default();

        // Clear expired toast before any rendering so the status bar sees the latest state.
        if self.status_expires.is_some_and(|t| t <= Instant::now()) {
            self.status.clear();
            self.status_expires = None;
            self.force_clear = true;
        }

        let now_playing: Option<String> = if active {
            let idx = self.player.status.lock().unwrap().current_idx;
            self.playback_queue()
                .items
                .get(idx)
                .map(|i| i.playback_label())
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
            &mut layout.tabbar_vol_area,
            player_h,
            show_controls,
            &now_playing_title,
        );

        self.render_context_menu(f, &mut layout);

        let power_panel_area = (layout.main.panel_area.width > 0).then_some(layout.main.panel_area);
        if self.show_sessions {
            self.render_sessions_overlay(f, power_panel_area);
        }
        if self.show_playlists {
            self.render_playlists_panel(f, power_panel_area);
        }
        if self.show_help {
            self.render_help_panel(f, power_panel_area);
        }
        if self.show_settings {
            self.render_settings_panel(f, &mut layout, power_panel_area);
            if self.multiselect_popup.is_some() {
                self.render_multiselect_popup(f);
            }
            if self.library_routes_popup.is_some() {
                self.render_library_routes_popup(f);
            }
        }
        if self.save_playlist_dialog.is_some() {
            self.render_save_playlist_dialog(f);
        }
        if self.show_save_playlist_modal {
            self.render_dirty_playlist_modal(f);
        }

        // One atomic replace, reached only once the full pass above has
        // completed -- `self.layout` never observes a half-updated frame.
        self.layout = layout;
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
        tabbar_vol_area_out: &mut Rect,
        player_h: u16,
        show_controls: bool,
        now_playing_title: &Option<(String, Color)>,
    ) {
        if area.height < 4 {
            return;
        }
        // Apply the tab saved from the previous session once libs have loaded.
        if self.library_tab_pending > 0 && !self.libs.is_empty() {
            self.library_tab = self.library_tab_pending.min(self.libs.len());
            self.library_tab_pending = 0;
        }
        // Safety clamp -- library_tab should already be valid, but guard against
        // any edge case where libs haven't populated yet.
        if self.library_tab > self.libs.len() {
            self.library_tab = 0;
        }

        // Left panel (card + queue) | Right panel (library, remaining).
        let left_w = if self.queue_column_collapsed {
            0
        } else {
            self.queue_column_width
        };
        let right_w = area.width.saturating_sub(left_w);

        // Header row removed — the tab bar above indicates current location.
        layout.breadcrumbs = Vec::new();
        layout.selector_tabs = Vec::new();
        let content_h = area.height;
        let left_area = if self.queue_column_collapsed {
            Rect::default()
        } else {
            Rect {
                x: area.x,
                y: area.y,
                width: left_w,
                height: content_h,
            }
        };
        layout.panel_area = left_area;
        layout.panel_content_area = Self::power_panel_content_area(left_area);

        let queue_focused = matches!(self.panel_focus, PanelFocus::Queue);
        let left_focused = !queue_focused;

        // Full-column background behind the card image and queue list.
        if !self.queue_column_collapsed {
            let left_bg = if queue_focused {
                palette::QUEUE_COLUMN_FOCUSED_BG
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            f.render_widget(
                Block::default().style(Style::default().bg(left_bg)),
                left_area,
            );
        }

        // Full-column background for the right panel (tabs, player, library, queue, status).
        let right_full_area = Rect {
            x: area.x + left_w + POWER_VIEW_GAP,
            y: area.y,
            width: right_w.saturating_sub(POWER_VIEW_GAP),
            height: area.height,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(palette::LIBRARY_SIDE_BG)),
            right_full_area,
        );

        // Inner content area with padding inside the colored box (queue uses this).
        let left_content = Rect {
            x: left_area.x + 2,
            y: left_area.y + 3,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(4),
        };
        // Blank row, queue title row, then card image.
        if !self.queue_column_collapsed {
            self.render_power_queue_title(
                f,
                Rect {
                    x: left_area.x + 2,
                    y: left_area.y + 1,
                    width: left_area.width.saturating_sub(4),
                    height: 1,
                },
                layout,
            );
        }
        let card_area = Rect {
            x: left_area.x + 2,
            y: left_area.y + 3,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(4),
        };

        let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
        let right_area = Rect {
            x: area.x + left_w + POWER_VIEW_GAP,
            y: area.y + tab_h + player_h,
            width: right_w.saturating_sub(POWER_VIEW_GAP),
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
        self.render_tabs(f, tab_area, tabs_area_out, tabbar_vol_area_out);

        // Player panel below the tab bar.
        if player_h > 0 {
            let player_area = Rect {
                x: right_area.x,
                y: area.y + tab_h,
                width: right_area.width,
                height: player_h,
            };
            self.render_player_panel(
                f,
                player_area,
                playback,
                player_h,
                show_controls,
                now_playing_title,
            );
        }

        // Status bar sits at the bottom of the right panel only.
        let status_area = Rect {
            x: right_area.x,
            y: right_area.y + right_area.height,
            width: right_area.width,
            height: 1,
        };

        let (lib_area, queue_area) = if self.queue_column_collapsed {
            (right_area, Rect::default())
        } else {
            // The card fills the top of the left column; the queue list takes
            // the rows below it. Short terminals keep that same structure.
            let (card_h, _) = self.render_power_card(f, card_area);
            let left_remaining = left_content.height.saturating_sub(card_h);
            (
                right_area,
                Rect {
                    y: left_content.y + card_h,
                    height: left_remaining,
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
        let lib_area = power_right_panel_content_area(lib_area, self.queue_column_collapsed);

        let mut render_lib_area = lib_area;
        if self.library_tab > 0 && self.is_music_group_view(self.library_tab - 1) {
            let lib_idx = self.library_tab - 1;
            if lib_area.height > 0 {
                let pills_area = Rect {
                    x: lib_area.x,
                    y: lib_area.y,
                    width: lib_area.width,
                    height: 1,
                };
                self.render_power_music_group_pills_row(f, pills_area, lib_idx, layout);
                render_lib_area = Rect {
                    y: lib_area.y + 2,
                    height: lib_area.height.saturating_sub(2),
                    ..lib_area
                };
            } else {
                layout.selector_tabs = Vec::new();
            }
        } else if self.library_tab > 0 && self.should_show_letter_pills(self.library_tab - 1) {
            let lib_idx = self.library_tab - 1;
            if lib_area.height > 0 {
                let pills_area = Rect {
                    x: lib_area.x,
                    y: lib_area.y,
                    width: lib_area.width,
                    height: 1,
                };
                self.render_power_letter_pills_row(f, pills_area, lib_idx, layout);
                render_lib_area = Rect {
                    y: lib_area.y + 2,
                    height: lib_area.height.saturating_sub(2),
                    ..lib_area
                };
            } else {
                layout.selector_tabs = Vec::new();
            }
        }

        if !self.queue_column_collapsed {
            let desired_queue_rows = {
                let queue = self.displayed_queue();
                rendered_power_queue_rows_for_padding(&queue.items, queue_area)
            };
            let queue_list_area =
                render_power_queue_panel_frame(f, queue_area, desired_queue_rows, queue_focused);
            self.render_power_queue(f, queue_list_area, queue_focused, layout);
        }
        self.render_power_library(f, render_lib_area, left_focused, layout);

        // Status bar + toast overlay at the bottom of the right panel.
        if status_area.width > 0 {
            self.render_status_bar(f, status_area, playback, false, true);
            let show_toast =
                !self.status.is_empty() && (!self.system_notifications || self.notif_failed);
            if show_toast {
                f.render_widget(Clear, status_area);
                f.render_widget(
                    Paragraph::new(Self::toast_line(&self.status))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(palette::TOAST_FG).bg(palette::TOAST_BG)),
                    status_area,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
