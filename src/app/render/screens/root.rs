use crate::app::layout::{AppLayout, FrameChromeGeometry, LayoutMain, LayoutPlayback};
use crate::app::render::components::chrome;
use crate::app::render::components::widgets::{
    render_queue_panel_frame, right_panel_content_area, COLUMN_GAP,
};
use crate::app::{palette, App, PanelFocus, PanelMode, TabSelection, TABBAR_LEFT_RESERVE};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use std::time::Instant;

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
const TAB_BAR_BOX_HEIGHT: u16 = 3;

/// Height of the player panel box below the tab bar (seekbar + title +
/// controls rows). Must stay in sync with `render`'s local player_h
/// computation (seek 1 + title 1 + controls 2); it is what
/// `compute_frame_layout` uses to place the right-column player area.
const PLAYER_BOX_HEIGHT: u16 = 4;

impl App {
    pub(in crate::app) fn now_playing_throbber_span(&self) -> Span<'static> {
        const FRAMES: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
        Span::styled(
            FRAMES[self.now_playing_throbber_index % FRAMES.len()],
            Style::default().fg(palette::ACCENT),
        )
    }

    /// Paint-free geometry seam entry (D2). Returns `None` on a zero-dimension
    /// frame before ANY mutation, so `self.layout` keeps reflecting the last
    /// frame that rendered in full. Otherwise updates the frame-dependent
    /// inputs (terminal size, mini-view focus, queue column width) and then
    /// computes the root/chrome geometry into the typed partial subresult
    /// `FrameChromeGeometry`. `render` publishes the migrated chrome fields
    /// into the fresh `AppLayout` and `render_main` consumes this subresult
    /// instead of recomputing root/chrome geometry inline.
    pub(in crate::app) fn compute_frame_layout(
        &mut self,
        area: Rect,
    ) -> Option<FrameChromeGeometry> {
        // Guard against zero-dimension terminal (e.g. minimized or piped)
        // before any state mutation or geometry computation.
        if area.width == 0 || area.height == 0 {
            return None;
        }
        if area.width != self.terminal_width || area.height != self.terminal_height {
            self.card_image_states.clear();
            self.card_image_loading.clear();
        }
        if self.terminal_width >= crate::app::MINI_VIEW_THRESHOLD
            && area.width < crate::app::MINI_VIEW_THRESHOLD
        {
            self.mini_view_focus = PanelFocus::Queue;
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;
        if self.clamp_queue_column_width() {
            self.save_prefs();
        }
        Some(self.compute_chrome_geometry(area))
    }

    /// Compute the root/chrome geometry for one frame, paint-free. Pure reads
    /// of `self` state; the single production caller is `compute_frame_layout`
    /// (the only seam entry), and `render_main` consumes the result rather
    /// than recomputing it. `pub(in crate::app::render)` so the render test
    /// helpers can render a view through `render_main` with the same
    /// authoritative geometry without pulling in `compute_frame_layout`'s
    /// terminal-normalization side effects.
    pub(in crate::app::render) fn compute_chrome_geometry(
        &self,
        area: Rect,
    ) -> FrameChromeGeometry {
        // Left panel (card + queue) | Right panel (library, remaining).
        let left_w = match self.effective_panel_mode() {
            PanelMode::Both => self.queue_column_width,
            PanelMode::LibraryOnly => 0,
            PanelMode::QueueOnly => area.width,
        };
        let right_w = area.width.saturating_sub(left_w);
        let right_visible = self.effective_panel_mode() != PanelMode::QueueOnly;

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
        let panel_area = if self.terminal_width < crate::app::MINI_VIEW_THRESHOLD
            && self.effective_panel_mode() == PanelMode::LibraryOnly
        {
            area
        } else {
            left_area
        };
        let panel_content_area = chrome::left_panel_content_area(panel_area);
        let queue_focused = matches!(self.effective_panel_focus(), PanelFocus::Queue);

        // Full-column background behind the card image and queue list.
        let right_full_area = Rect {
            x: area.x + left_w + COLUMN_GAP,
            y: area.y,
            width: right_w.saturating_sub(COLUMN_GAP),
            height: area.height,
        };

        // Inner content area with padding inside the colored box (queue uses this).
        let left_content = Rect {
            x: left_area.x + 2,
            y: left_area.y + 1,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(2),
        };

        let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
        let right_area = Rect {
            x: area.x + left_w + COLUMN_GAP,
            y: area.y + tab_h + PLAYER_BOX_HEIGHT,
            width: right_w.saturating_sub(COLUMN_GAP),
            height: content_h
                .saturating_sub(1)
                .saturating_sub(tab_h)
                .saturating_sub(PLAYER_BOX_HEIGHT),
        };

        // Tab bar at the very top of the right column.
        let tab_bar_area = Rect {
            x: right_area.x,
            y: area.y,
            width: right_area.width,
            height: tab_h,
        };

        // Player panel below the tab bar (right column only).
        let player_area = if right_visible {
            Rect {
                x: right_area.x,
                y: area.y + tab_h,
                width: right_area.width,
                height: PLAYER_BOX_HEIGHT,
            }
        } else {
            Rect::default()
        };

        // Status bar sits at the bottom of the right panel only.
        let status_area = Rect {
            x: right_area.x,
            y: right_area.y + right_area.height,
            width: right_area.width,
            height: 1,
        };

        // Tab-bar hit targets; only published when the tab bar actually paints.
        let tabs_area = if right_visible {
            let tab_row = Rect {
                y: tab_bar_area.y + 1,
                height: 1,
                ..tab_bar_area
            };
            let pb_h: u16 = 2; // 2-col padding inside the coloured box
            let tabs_x = tab_bar_area.x + 1;
            let tabs_w = tab_bar_area
                .width
                .saturating_sub(2 * pb_h + TABBAR_LEFT_RESERVE);
            Rect {
                x: tabs_x,
                width: tabs_w,
                ..tab_row
            }
        } else {
            Rect::default()
        };

        FrameChromeGeometry {
            panel_area,
            panel_content_area,
            left_area,
            right_area,
            right_full_area,
            left_content,
            tab_bar_area,
            tabs_area,
            player_area,
            status_area,
            right_visible,
            queue_focused,
            left_w,
            right_w,
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let Some(chrome) = self.compute_frame_layout(area) else {
            // Zero-dimension terminal: `self.layout` is left untouched here --
            // it still reflects the last frame that rendered in full.
            return;
        };

        // Every render sub-call below writes into this fresh, local value
        // instead of `self.layout` directly. It's swapped into `self.layout`
        // in one atomic assignment only once this pass completes in full, so
        // an early return partway through can never leave `self.layout`
        // holding a mix of fields from two different frames.
        let mut layout = AppLayout::default();

        let active = self.player.status.lock().unwrap().active;
        let show_controls =
            active || self.connected_session_id.is_some() || self.cast_attachment.is_some();
        let playing_panel = show_controls;
        // Always reserve the player rows (title + controls) so
        // that content doesn't shift when the player appears or disappears.
        let player_h = PLAYER_BOX_HEIGHT;
        let [main_area] = Layout::vertical([Constraint::Min(0)]).areas(area);

        // Migrated root/chrome fields are published here from the subresult
        // (one authoritative computation). `render_main` bails on a frame too
        // short to draw (height < 4) without writing anything; publishing is
        // gated identically so a degenerate frame still installs the
        // all-default layout, matching the pre-split behavior.
        if area.height >= 4 {
            layout.main.panel_area = chrome.panel_area;
            layout.main.panel_content_area = chrome.panel_content_area;
            layout.playback.player_area = chrome.player_area;
            layout.playback.status_area = chrome.status_area;
            layout.tabs_area = chrome.tabs_area;
        }

        // Clear expired toast before any rendering so the status bar sees the latest state.
        if self.status_expires.is_some_and(|t| t <= Instant::now()) {
            self.status.clear();
            self.status_expires = None;
            self.status_severity = crate::app::notify_actions::ToastSeverity::default();
            self.force_clear = true;
        }

        let now_playing: Option<String> = if active {
            let idx = self.player.status.lock().unwrap().current_idx;
            let queue = self.playback_queue();
            queue.item_at(idx).map(|item| item.title().to_string())
        } else {
            None
        };
        let title_color = palette::PLAYBACK_VALUE_FG;
        let now_playing_title: Option<(String, Color)> = if playing_panel {
            if active {
                now_playing.map(|t| (t, title_color))
            } else if let Some(ref cast) = self.cast_attachment {
                self.cast_now_playing_title(cast).map(|t| (t, title_color))
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
            &chrome,
            &mut layout.main,
            &mut layout.playback,
            player_h,
            show_controls,
            &now_playing_title,
        );

        // The Context menu is an owned TuiRealm component now (task 5.3c):
        // the shell mounts it from `pending_overlay` and paints it via the
        // overlay stack, so nothing is written to `layout` here.

        // Blocking modal mount exclusivity is asserted by the shell, where the
        // TuiRealm mount state is available.
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
}

impl App {
    pub(in crate::app::render) fn render_main(
        &mut self,
        f: &mut Frame,
        area: Rect,
        chrome: &FrameChromeGeometry,
        layout: &mut LayoutMain,
        playback: &mut LayoutPlayback,
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

        // Root/chrome geometry (left/right columns, tabs/player/status
        // placement, focus facts) comes from the paint-free subresult
        // computed by `compute_frame_layout`; it is consumed here, never
        // recomputed.
        let FrameChromeGeometry {
            panel_area: _,
            panel_content_area: _,
            left_area,
            right_area,
            right_full_area,
            left_content,
            tab_bar_area,
            tabs_area,
            player_area,
            status_area,
            right_visible,
            queue_focused,
            // Widths are carried by the precomputed areas; kept for the typed
            // subresult contract but not consumed by the paint body.
            left_w: _,
            right_w: _,
        } = *chrome;
        let left_focused = !queue_focused;

        // Header row removed — the tab bar above indicates current location.
        layout.breadcrumbs = Vec::new();
        layout.selector_tabs = Vec::new();

        // Full-column background behind the card image and queue list.
        if self.effective_panel_mode() != PanelMode::LibraryOnly {
            let left_bg = palette::resolve_surface_focus(queue_focused);
            f.render_widget(
                Block::default().style(Style::default().bg(left_bg)),
                left_area,
            );
        }

        // Full-column background for the right panel (tabs, player, library, queue, status).
        if right_visible {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
                right_full_area,
            );
        }

        // Tab bar at the very top of the right column.
        if right_visible {
            self.render_tabs(f, tab_bar_area, tabs_area);
        }

        // Player panel below the tab bar.
        if right_visible && player_h > 0 {
            let playback_panel_bg = if queue_focused {
                palette::SURFACE_FOCUSED
            } else {
                palette::SURFACE_PLAYBACK
            };
            crate::app::render::render_player_panel(
                f,
                self.playback_panel_context(
                    player_area,
                    playback,
                    player_h,
                    show_controls,
                    now_playing_title,
                    playback_panel_bg,
                ),
            );
        }

        let (lib_area, queue_area) = if self.effective_panel_mode() == PanelMode::LibraryOnly {
            (right_area, Rect::default())
        } else {
            // The card fills the top of the left column; the queue list takes
            // the rows below it. Short terminals keep that same structure.
            let is_queue_only = self.effective_panel_mode() == PanelMode::QueueOnly;
            let is_wide = is_queue_only && left_area.width >= 100;
            let (card_h, card_w, _) = self.render_card(f, left_content, is_wide);
            let mut left_remaining = left_content.height.saturating_sub(card_h);

            // Queue-only mode has no right column, so the playback panel
            // (seekbar + title + controls) renders here instead: stacked
            // below the card on narrow terminals, or beside it on wide ones.
            let mut narrow_player_h = 0;
            if is_queue_only {
                if is_wide {
                    let panel_area = Rect {
                        x: left_content.x + card_w + 2,
                        y: left_content.y,
                        width: left_content.width.saturating_sub(card_w + 2),
                        height: card_h,
                    };
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
                        panel_area,
                    );
                    crate::app::render::render_player_panel(
                        f,
                        self.playback_panel_context(
                            panel_area,
                            playback,
                            player_h,
                            show_controls,
                            now_playing_title,
                            palette::SURFACE_CHROME,
                        ),
                    );
                } else {
                    let panel_area = Rect {
                        x: left_content.x,
                        y: left_content.y + card_h,
                        width: left_content.width,
                        height: player_h,
                    };
                    crate::app::render::render_player_panel(
                        f,
                        self.playback_panel_context(
                            panel_area,
                            playback,
                            player_h,
                            show_controls,
                            now_playing_title,
                            palette::SURFACE_CHROME,
                        ),
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

        // Status bar at the bottom of the right panel. Playback prompts are
        // painted by the shell-mounted component after this legacy frame.
        if status_area.width > 0 {
            self.render_status_bar(f, status_area, playback, false);
        }
    }
}
