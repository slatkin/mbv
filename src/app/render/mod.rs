mod home;
pub mod indicators;
mod library;
mod overlays;
mod playlist;
pub(crate) mod power;

use super::ui_util::{fmt_duration, trunc_str};
use super::{layout::AppLayout, palette, App};
use crate::app::layout::LayoutPlayback;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use std::time::Instant;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

pub(super) fn thin_vertical_thumb(mut glyphs: GlyphSet) -> GlyphSet {
    glyphs.thumb_vertical_lower = ['▕'; 8];
    glyphs.thumb_vertical_upper = ['▕'; 8];
    glyphs
}

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
/// Shared by both view modes (Standard here, Power in `power/mod.rs`) so the
/// two layouts can't drift apart on this shared constant.
pub(super) const TAB_BAR_BOX_HEIGHT: u16 = 3;
pub(super) const PLAY_ICON: &str = "\u{f04b}";
const PLAY_ICON_FALLBACK: &str = ">";

pub(super) fn play_icon(use_nerd_fonts: bool) -> &'static str {
    if use_nerd_fonts {
        PLAY_ICON
    } else {
        PLAY_ICON_FALLBACK
    }
}

fn daemon_endpoint_label(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.eq_ignore_ascii_case("local") {
        return None;
    }
    if let Some(tcp) = endpoint.strip_prefix("tcp://") {
        return tcp
            .rsplit_once(':')
            .map(|(host, _port)| host)
            .filter(|host| !host.is_empty())
            .map(str::to_string);
    }
    if let Some(path) = endpoint.strip_prefix("unix://") {
        return std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
    }
    std::path::Path::new(endpoint)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn server_url_label(server_url: &str) -> Option<String> {
    let value = server_url.trim();
    if value.is_empty() {
        return None;
    }
    let without_scheme = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value);
    without_scheme
        .split('/')
        .next()
        .and_then(|host_port| host_port.split('@').next_back())
        .and_then(|host_port| host_port.split(':').next())
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

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
        if self.clamp_power_left_width() {
            self.save_prefs();
        }

        // Every render sub-call below writes into this fresh, local value
        // instead of `self.layout` directly. It's swapped into `self.layout`
        // in one atomic assignment only once this pass completes in full, so
        // an early return partway through (like the guard above) can never
        // leave `self.layout` holding a mix of fields from two different
        // frames.
        let mut layout = AppLayout::default();
        // The library-table geometry caches are indexed by library tab index
        // and sized once (to `self.libs.len()`) by `rebuild_library_tabs_from_views`,
        // not rebuilt every frame -- each render pass only overwrites the slot
        // for the currently-viewed library. Carry the existing sizing/values
        // forward so `get_mut(lib_idx)` below still finds a slot to write into;
        // otherwise every library-tab index in a freshly-defaulted (empty) Vec
        // would be unwritable and this state would never survive a frame.
        layout.library.lib_scroll = self.layout.library.lib_scroll.clone();
        layout.library.lib_row_heights = self.layout.library.lib_row_heights.clone();
        layout.library.lib_table_area = self.layout.library.lib_table_area.clone();

        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        let in_power = self.view_mode == super::ViewMode::Power;
        let playing_panel = show_controls;
        // In Power View always reserve the player rows (title + controls) so that
        // content doesn't shift when the player appears or disappears.
        let reserve_player_rows = in_power;
        // Player panel and tab bar now render within the right column of each
        // view instead of as full-width rows above the content area.
        let (seek_h, _gap_h, title_h, controls_h): (u16, u16, u16, u16) =
            if playing_panel || reserve_player_rows {
                (1, 0, 1, 1)
            } else {
                (1, 0, 0, 0)
            };
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
        let title_color = palette::BLUE;
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
        // Top-level render dispatch (issue #275): Power owns its own nav via
        // `power_left_tab` and ignores `tab_idx` entirely; `tab_idx` is
        // meaningful only in Standard mode.
        match self.view_mode {
            super::ViewMode::Power => {
                self.render_power_view(
                    f,
                    main_area,
                    &mut layout.power,
                    &mut layout.playback,
                    &mut layout.tabs_area,
                    &mut layout.tabbar_vol_area,
                    player_h,
                    show_controls,
                    &now_playing_title,
                );
            }
            super::ViewMode::Standard => {
                let tab_h: u16 = TAB_BAR_BOX_HEIGHT;
                let tab_area = Rect {
                    height: tab_h,
                    ..main_area
                };
                self.render_tabs(
                    f,
                    tab_area,
                    &mut layout.tabs_area,
                    &mut layout.tabbar_vol_area,
                    false,
                );

                let content_h = main_area
                    .height
                    .saturating_sub(tab_h)
                    .saturating_sub(player_h)
                    .saturating_sub(1);
                let content_area = Rect {
                    y: main_area.y + tab_h + player_h,
                    height: content_h,
                    ..main_area
                };

                if player_h > 0 {
                    let player_area = Rect {
                        y: main_area.y + tab_h,
                        height: player_h,
                        ..main_area
                    };
                    self.render_player_panel(
                        f,
                        player_area,
                        &mut layout.playback,
                        player_h,
                        show_controls,
                        &now_playing_title,
                    );
                }

                if self.tab_idx == 0 {
                    self.render_combined(f, content_area, &mut layout.home);
                } else if self.tab_idx == 1 {
                    self.render_queue_panel(f, content_area, &mut layout.queue);
                } else {
                    self.render_library(
                        f,
                        content_area,
                        self.tab_idx - self.lib_tab_offset(),
                        None,
                        &mut layout.library,
                    );
                }

                let sb_area = Rect {
                    y: main_area.y + content_h + tab_h + player_h,
                    height: 1,
                    ..main_area
                };
                self.render_status_bar(f, sb_area, &mut layout.playback, true, true);
                let show_toast =
                    !self.status.is_empty() && (!self.system_notifications || self.notif_failed);
                if show_toast {
                    f.render_widget(Clear, sb_area);
                    f.render_widget(
                        Paragraph::new(Self::toast_line(&self.status))
                            .alignment(Alignment::Center)
                            .style(Style::default().fg(palette::TEXT).bg(palette::IRIS)),
                        sb_area,
                    );
                }
            }
        }

        self.render_context_menu(f, &mut layout);

        if self.show_sessions {
            self.render_sessions_overlay(f);
        }
        if self.show_playlists {
            self.render_playlists_panel(f);
        }
        if self.show_help {
            self.render_help_panel(f);
        }
        if self.show_settings {
            self.render_settings_panel(f, &mut layout);
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

    fn toast_line(s: &str) -> Line<'static> {
        let text_style = Style::default()
            .fg(palette::TEXT)
            .add_modifier(Modifier::BOLD);
        let yellow_style = Style::default()
            .fg(palette::YELLOW)
            .add_modifier(Modifier::BOLD);
        let open = s.find(['[', '(']);
        if let Some(i) = open {
            let close = s[i..].find([']', ')']).map(|j| i + j);
            if let Some(j) = close {
                let mut spans = vec![
                    Span::styled(s[..i].to_string(), text_style),
                    Span::styled(s[i..i + 1].to_string(), text_style),
                ];
                for c in s[i + 1..j].chars() {
                    spans.push(if c.is_uppercase() {
                        Span::styled(c.to_string(), yellow_style)
                    } else {
                        Span::styled(c.to_string(), text_style)
                    });
                }
                spans.push(Span::styled(s[j..j + 1].to_string(), text_style));
                if j + 1 < s.len() {
                    spans.push(Span::styled(s[j + 1..].to_string(), text_style));
                }
                return Line::from(spans);
            }
        }
        Line::from(Span::styled(s.to_string(), text_style))
    }

    pub(super) fn render_panel_shell(
        f: &mut Frame,
        full: Rect,
        width: u16,
        title: &str,
        hints: &str,
    ) -> Rect {
        let sidebar = Rect {
            x: full.x,
            y: full.y + 2,
            width: width.min(full.width),
            height: full.height.saturating_sub(2),
        };
        f.render_widget(Clear, sidebar);
        // Too short to fit a title row, a content row, and the 2-row footer;
        // bail out rather than let `footer_y = sidebar.y + sidebar.height - 2`
        // underflow below.
        if sidebar.height < 4 || sidebar.width == 0 {
            return sidebar;
        }
        f.render_widget(
            Block::default().style(Style::default().bg(palette::PANEL_BG)),
            sidebar,
        );
        for row in sidebar.y..sidebar.y + sidebar.height {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "\u{2502}",
                    Style::default().fg(palette::OVERLAY),
                )),
                Rect {
                    x: sidebar.x + sidebar.width - 1,
                    y: row,
                    width: 1,
                    height: 1,
                },
            );
        }
        let inner_w = sidebar.width.saturating_sub(1);
        let ix = sidebar.x;
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                title.to_owned(),
                Style::default()
                    .fg(palette::TEXT)
                    .add_modifier(Modifier::BOLD),
            )]))
            .style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: sidebar.x,
                y: sidebar.y,
                width: sidebar.width.saturating_sub(1),
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: sidebar.x + sidebar.width - 1,
                y: sidebar.y,
                width: 1,
                height: 1,
            },
        );
        let footer_y = sidebar.y + sidebar.height - 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                "\u{2500}".repeat(inner_w as usize),
                Style::default().fg(palette::OVERLAY),
            )),
            Rect {
                x: ix,
                y: footer_y,
                width: inner_w,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                trunc_str(hints, inner_w as usize),
                Style::default().fg(palette::TEXT),
            )]))
            .style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: ix,
                y: footer_y + 1,
                width: inner_w,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
            Rect {
                x: sidebar.x + sidebar.width - 1,
                y: footer_y + 1,
                width: 1,
                height: 1,
            },
        );
        Rect {
            x: ix,
            y: sidebar.y + 1,
            width: inner_w,
            height: sidebar.height.saturating_sub(3),
        }
    }

    /// Overlay a thin scroll indicator on a sidebar's right border column when
    /// its content doesn't fit `content.height`. Reuses the existing border
    /// column instead of reserving a dedicated width for a scrollbar.
    pub(super) fn render_sidebar_scrollbar(
        f: &mut Frame,
        content: Rect,
        total: usize,
        scroll: usize,
    ) {
        let visible = content.height as usize;
        if visible == 0 || total <= visible {
            return;
        }
        let max_offset = total.saturating_sub(visible);
        let scrollbar = ScrollBar::vertical(ScrollLengths {
            content_len: total,
            viewport_len: visible,
        })
        .offset(scroll.min(max_offset))
        .glyph_set(thin_vertical_thumb(GlyphSet::box_drawing()))
        .track_style(Style::default().fg(palette::SCROLLBAR))
        .thumb_style(Style::default().fg(palette::SCROLLBAR));
        f.render_widget(
            &scrollbar,
            Rect {
                x: content.x.saturating_add(content.width),
                width: 1,
                ..content
            },
        );
    }

    /// Render one row in a sidebar panel list.
    /// `content_spans` should not include the indicator — it is prepended automatically.
    /// Returns the usable text width (content area minus indicator and space).
    pub(super) fn panel_row_text_width(content_width: u16) -> usize {
        content_width.saturating_sub(1) as usize // indicator char
    }

    pub(super) fn render_panel_row(
        f: &mut Frame,
        x: u16,
        y: u16,
        width: u16,
        selected: bool,
        spans: Vec<Span>,
    ) {
        let indicator = Span::styled(
            if selected { "\u{258c}" } else { " " },
            Style::default().fg(palette::AQUA),
        );
        let mut all = vec![indicator];
        all.extend(spans);
        f.render_widget(
            Paragraph::new(Line::from(all)),
            Rect {
                x,
                y,
                width,
                height: 1,
            },
        );
    }

    /// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
    /// Returns None if the local player is not active.
    /// Callers wrap these in [ ... ] with whatever surrounding style they need.
    pub(super) fn build_status_indicator_spans(&self) -> Option<Vec<Span<'static>>> {
        let data = self.playback_indicator_target().indicator_data(self)?;
        Some(indicators::indicator_spans(
            self.indicator_style,
            &data,
            self.use_nerd_fonts,
        ))
    }

    /// Renders the tab bar within the given 1-row `area` and populates
    /// `layout.tabs_area` / `layout.tabbar_vol_area` for mouse hit testing.
    fn render_tabs(
        &mut self,
        f: &mut Frame,
        area: Rect,
        tabs_area_out: &mut Rect,
        tabbar_vol_area_out: &mut Rect,
        in_power: bool,
    ) {
        // Fill the tab bar area with the tab box's own background.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::DARK_BG)),
            area,
        );

        // Tabs render on the second row; first row is padding inside the box.
        let tab_row = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };

        let pb_h: u16 = 2; // 2-col padding inside the coloured box
        let tabs_x = area.x + pb_h;
        let tabs_w = area
            .width
            .saturating_sub(2 * pb_h + super::TABBAR_LEFT_RESERVE + super::TABBAR_RIGHT_RESERVE);
        let tabs_area = Rect {
            x: tabs_x,
            width: tabs_w,
            ..tab_row
        };
        *tabs_area_out = tabs_area;

        let volume = self.playback_display_target().displayed_volume(self);
        let vol_color = if volume > 100 {
            palette::RED
        } else if volume > 60 {
            palette::YELLOW
        } else {
            palette::AQUA
        };
        let vol_spans = vec![
            Span::styled("VOL ", Style::default().fg(palette::MUTED)),
            Span::styled(
                volume.to_string(),
                Style::default().fg(vol_color).add_modifier(Modifier::BOLD),
            ),
        ];
        let vol_w: u16 = vol_spans.iter().map(|s| s.content.width() as u16).sum();
        let vol_rect = Rect {
            x: area.x + area.width.saturating_sub(vol_w + pb_h),
            y: tab_row.y,
            width: vol_w,
            height: 1,
        };
        *tabbar_vol_area_out = vol_rect;
        f.render_widget(Paragraph::new(Line::from(vol_spans)), vol_rect);

        let (vis_start, vis_end) = self.visible_tab_range(tabs_w);
        let has_left = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let ind_style = Style::default().fg(palette::WHITE);
        let left_w: u16 = if has_left { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left {
            f.render_widget(
                Paragraph::new("« ").style(ind_style),
                Rect {
                    x: tabs_x,
                    y: tab_row.y,
                    width: 2,
                    height: 1,
                },
            );
        }
        if has_right {
            f.render_widget(
                Paragraph::new(" »").style(ind_style),
                Rect {
                    x: tabs_x + tabs_w.saturating_sub(2),
                    y: tab_row.y,
                    width: 2,
                    height: 1,
                },
            );
        }
        let inner_tabs = Rect {
            x: tabs_x + left_w,
            y: tab_row.y,
            width: tabs_w.saturating_sub(left_w + right_w),
            height: area.height,
        };
        let tab_titles: Vec<Line> = if in_power {
            let names: Vec<String> = std::iter::once("Home".to_string())
                .chain(self.libs.iter().map(|l| l.library.name.clone()))
                .collect();
            let sel = self.power_left_tab;
            names
                .into_iter()
                .enumerate()
                .map(|(i, n)| {
                    let n = n.to_uppercase();
                    if i == sel {
                        Line::from(vec![
                            Span::styled("▐", Style::default().fg(palette::AQUA)),
                            Span::styled(
                                format!(" {n}  "),
                                Style::default()
                                    .fg(palette::WHITE)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])
                    } else {
                        Line::from(Span::styled(
                            format!("  {n}  "),
                            Style::default().fg(palette::SUBTLE),
                        ))
                    }
                })
                .collect()
        } else {
            let all_names: Vec<String> = std::iter::once("Home".to_string())
                .chain(std::iter::once("Queue".to_string()))
                .chain(self.libs.iter().map(|l| l.library.name.clone()))
                .collect();
            let selected_tab = if self.tab_idx < vis_start || self.tab_idx >= vis_end {
                usize::MAX
            } else {
                self.tab_idx - vis_start
            };
            all_names[vis_start..vis_end]
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let n = n.to_uppercase();
                    if i == selected_tab {
                        Line::from(vec![
                            Span::styled("▐", Style::default().fg(palette::AQUA)),
                            Span::styled(
                                format!(" {n}  "),
                                Style::default()
                                    .fg(palette::WHITE)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])
                    } else {
                        Line::from(Span::styled(
                            format!("  {n}  "),
                            Style::default().fg(palette::SUBTLE),
                        ))
                    }
                })
                .collect()
        };
        f.render_widget(
            Tabs::new(tab_titles)
                .select(usize::MAX)
                .style(Style::default().fg(palette::SUBTLE))
                .highlight_style(Style::default())
                .divider(Span::raw(""))
                .padding("", ""),
            inner_tabs,
        );
    }

    /// Renders the player panel (seekbar + now-playing title row) within the
    /// given `area`, which should be `player_h` rows tall.
    fn render_player_panel(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut super::layout::LayoutPlayback,
        player_h: u16,
        show_controls: bool,
        now_playing_title: &Option<(String, Color)>,
    ) {
        if player_h == 0 {
            return;
        }
        // Seekbar row (always present when player_h > 0).
        let seek_area = Rect { height: 1, ..area };
        if show_controls {
            self.render_seekbar(f, seek_area, layout);
        } else {
            layout.seekbar_area = Rect::default();
            let bar = "\u{2594}".repeat(seek_area.width as usize);
            f.render_widget(
                Paragraph::new(Span::styled(bar, Style::default().fg(palette::SEEK_TRACK))),
                seek_area,
            );
        }
        // Title row (when panel is expanded).
        if player_h >= 2 {
            const H_PAD: u16 = 1;
            let title_area = if area.width > 2 * H_PAD {
                Rect {
                    x: area.x + H_PAD,
                    width: area.width.saturating_sub(2 * H_PAD),
                    y: area.y + 1,
                    height: 1,
                }
            } else {
                Rect {
                    y: area.y + 1,
                    height: 1,
                    ..area
                }
            };
            if let Some((ref title, color)) = now_playing_title {
                self.render_title_row(f, title_area, title, *color, layout);
            }
        }
    }

    /// One-line now-playing header: play/pause, next, title, and time on the
    /// left, with the status-indicator badges right-aligned. Records click
    /// regions for the play/pause and next glyphs into `layout` (see issue
    /// #112); next is greyed out (and, per `handle_mouse`, non-clickable)
    /// when `transport_prev_next_available()` says the queue is at that
    /// boundary.
    fn render_title_row(
        &mut self,
        f: &mut Frame,
        area: Rect,
        title: &str,
        title_color: Color,
        layout: &mut LayoutPlayback,
    ) {
        if area.height == 0 || area.width == 0 {
            layout.play_pause_area = Rect::default();
            layout.stop_area = Rect::default();
            layout.next_area = Rect::default();
            return;
        }

        let (pos_ticks, rt_ticks, paused) = self.playback_progress();
        let pos_str = fmt_duration(pos_ticks / TICKS_PER_SECOND);
        let dur_str = fmt_duration(rt_ticks / TICKS_PER_SECOND);

        let (glyph, gcolor): (&str, Color) = if paused {
            (play_icon(self.use_nerd_fonts), palette::AQUA)
        } else {
            (
                if self.use_nerd_fonts {
                    "\u{f04c}"
                } else {
                    "||"
                },
                palette::YELLOW,
            )
        };
        let stop_glyph = if self.use_nerd_fonts { "\u{f04d}" } else { "X" };
        let stop_gap = " ";

        let next_glyph = if self.use_nerd_fonts {
            "\u{f051}"
        } else {
            ">>"
        };
        let next_gap = " ";
        let next_avail = self.transport_prev_next_available().1;
        let next_color = if next_avail {
            palette::WHITE
        } else {
            palette::MUTED
        };
        let stop_avail =
            self.connected_session_id.is_some() || self.player.status.lock().unwrap().active;
        let stop_color = if stop_avail {
            palette::RED
        } else {
            palette::MUTED
        };
        let right = self.build_status_indicator_spans().unwrap_or_default();

        // Left: glyph  stop  next  title  │  elapsed / total
        // A running `x` cursor tracks where each clickable glyph lands in the
        // rendered `Line`, so `layout.*_area` exactly matches what's on screen
        // rather than an estimate.
        let mut left: Vec<Span> = Vec::new();
        let mut x = area.x;

        let glyph_text = format!("{glyph} ");
        let glyph_w = glyph_text.width() as u16;
        layout.play_pause_area = Rect {
            x,
            y: area.y,
            width: glyph_w,
            height: 1,
        };
        x += glyph_w;
        left.push(Span::styled(
            glyph_text,
            Style::default().fg(gcolor).add_modifier(Modifier::BOLD),
        ));

        let stop_w = stop_glyph.width() as u16;
        layout.stop_area = Rect {
            x,
            y: area.y,
            width: stop_w,
            height: 1,
        };
        x += stop_w;
        left.push(Span::styled(stop_glyph, Style::default().fg(stop_color)));
        left.push(Span::raw(stop_gap));
        x += stop_gap.width() as u16;

        let next_w = next_glyph.width() as u16;
        layout.next_area = Rect {
            x,
            y: area.y,
            width: next_w,
            height: 1,
        };
        left.push(Span::styled(next_glyph, Style::default().fg(next_color)));

        left.push(Span::raw(next_gap));

        let sep_text = " \u{2502} ";
        let time_text = format!("{pos_str} / {dur_str}");
        let post_time_gap = "  ";
        let right_w: u16 = right.iter().map(|s| s.content.width() as u16).sum();
        let fixed_w = glyph_w as usize
            + stop_w as usize
            + stop_gap.width()
            + next_w as usize
            + next_gap.width()
            + sep_text.width()
            + time_text.width()
            + post_time_gap.width()
            + right_w as usize;
        let title_w = (area.width as usize).saturating_sub(fixed_w);
        let title_text = if title_w == 0 {
            String::new()
        } else {
            trunc_str(title, title_w)
        };
        left.push(Span::styled(title_text, Style::default().fg(title_color)));

        left.push(Span::styled(
            sep_text,
            Style::default().fg(palette::OVERLAY),
        ));

        left.push(Span::styled(
            time_text,
            Style::default().fg(palette::SUBTLE),
        ));

        left.push(Span::raw(post_time_gap));

        let left_w: u16 = left.iter().map(|s| s.content.width() as u16).sum();
        let gap = area.width.saturating_sub(left_w + right_w) as usize;

        let mut spans = left;
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right);
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Current playback position / runtime (ticks) and paused state, from the
    /// connected remote session if any, otherwise the local player.
    fn playback_progress(&self) -> (i64, i64, bool) {
        if let Some(ref remote) = self.connected_session_state {
            let elapsed_s = self.remote_pos_at.elapsed().as_secs_f64();
            let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
            // Some Emby clients always report IsPaused=true even while playing.
            // Trust the API position advancing as the authoritative "actually playing" signal.
            let api_active = self.remote_api_pos_advanced_at.elapsed().as_secs() < 22;
            let is_paused = remote.is_paused && !api_active;
            (
                (pos_s * TICKS_PER_SECOND as f64) as i64,
                remote.runtime_s * TICKS_PER_SECOND,
                is_paused,
            )
        } else {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused)
        }
    }

    fn remote_status_spans(
        &self,
        remote_state: super::RemoteSlotState,
        daemon_endpoint: &str,
    ) -> Vec<Span<'static>> {
        let remote_on = matches!(
            remote_state,
            super::RemoteSlotState::AttachedSession | super::RemoteSlotState::DirectRemote
        );
        let glyph_style = Style::default()
            .bg(palette::STATUS_PILL_BG)
            .fg(ratatui::style::Color::White);

        let target = match remote_state {
            super::RemoteSlotState::Off => None,
            super::RemoteSlotState::AttachedSession => {
                self.connected_session_state.as_ref().and_then(|session| {
                    let device_name = session.device_name.trim();
                    if !device_name.is_empty() {
                        Some(device_name.to_string())
                    } else {
                        let host = session.host.trim();
                        (!host.is_empty()).then(|| host.to_string())
                    }
                })
            }
            super::RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
            super::RemoteSlotState::LocalDaemon => None,
        };
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let label = match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_core::api::device_name()),
        };
        let label_style = Style::default()
            .fg(if remote_on {
                palette::AQUA
            } else {
                ratatui::style::Color::Black
            })
            .bg(palette::STATUS_PILL_BG);

        vec![
            Span::styled(" ", Style::default().bg(palette::STATUS_PILL_BG)),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f1616}"
                } else {
                    "\u{1F5A7}"
                },
                glyph_style,
            ),
            Span::styled(label, label_style),
            Span::styled(" ", Style::default().bg(palette::STATUS_PILL_BG)),
        ]
    }

    fn playlist_status_spans(&self) -> Vec<Span<'static>> {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let (label, on) = match &self.queue_source {
            crate::config::QueueSource::Playlist { name, .. } => (format!("{gap}{name}"), true),
            _ => (format!("{gap}none"), false),
        };
        let glyph_style = Style::default()
            .bg(palette::STATUS_PILL_BG)
            .fg(ratatui::style::Color::White);
        let label_style = Style::default()
            .fg(if on { palette::YELLOW } else { palette::SUBTLE })
            .bg(palette::STATUS_PILL_BG);

        vec![
            Span::styled(" ", Style::default().bg(palette::STATUS_PILL_BG)),
            Span::styled(
                if self.use_nerd_fonts {
                    "\u{f03a}"
                } else {
                    "\u{1F5AD}"
                },
                glyph_style,
            ),
            Span::styled(label, label_style),
            Span::styled(" ", Style::default().bg(palette::STATUS_PILL_BG)),
        ]
    }

    fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
        self.playback_display_target()
            .displayed_mute(self)
            .then(|| {
                vec![
                    Span::styled(" ", Style::default().bg(palette::STATUS_PILL_BG)),
                    Span::styled(
                        "muted",
                        Style::default()
                            .fg(palette::RED)
                            .bg(palette::STATUS_PILL_BG)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default().bg(palette::STATUS_PILL_BG)),
                ]
            })
    }

    fn status_width(spans: &[Span]) -> u16 {
        spans.iter().map(|s| s.content.width() as u16).sum()
    }

    fn append_status(spans: &mut Vec<Span<'static>>, status: Vec<Span<'static>>) {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.extend(status);
    }

    fn set_status_label_color(spans: &mut [Span<'static>], color: Color) {
        if let Some(label) = spans.get_mut(2) {
            label.style = label.style.fg(color);
        }
    }

    fn set_status_pill_style(spans: &mut [Span<'static>], fg: Color, bg: Color) {
        for span in spans.iter_mut() {
            span.style = span.style.bg(bg);
        }
        Self::set_status_label_color(spans, fg);
    }

    /// Uppercase the status label span (index 2, same convention as
    /// [`Self::set_status_label_color`]) in place.
    pub(super) fn uppercase_status_label(spans: &mut [Span<'static>]) {
        let Some(label) = spans.get_mut(2) else {
            return;
        };
        label.content = label.content.to_uppercase().into();
    }

    /// Bold the status label span (index 2) in place when `bold` is set.
    pub(super) fn set_status_label_bold(spans: &mut [Span<'static>], bold: bool) {
        if bold {
            if let Some(label) = spans.get_mut(2) {
                label.style = label.style.add_modifier(Modifier::BOLD);
            }
        }
    }

    fn render_remote_status_hitbox(
        &self,
        layout: &mut LayoutPlayback,
        area: Rect,
        remote_x: Option<u16>,
        remote_w: u16,
    ) {
        if area.width == 0 {
            layout.ind_rc = Rect::default();
        } else if let Some(x) = remote_x {
            layout.ind_rc = Rect {
                x,
                y: area.y,
                width: remote_w,
                height: 1,
            };
        } else {
            layout.ind_rc = Rect::default();
        }
    }

    /// Persistent bottom status bar. Left side: connection, playlist, stay-alive,
    /// and mute status groups. Right side: queue source/save-state/scope detail.
    fn render_status_bar(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPlayback,
        show_session_pill: bool,
        show_playlist_pill: bool,
    ) {
        // Keep the row itself darker so the pills read as segments sitting on top of it.
        let bar_style = Style::default().bg(palette::DARK_BG);
        f.render_widget(Block::default().style(bar_style), area);
        layout.ind_mu = Rect::default();

        let remote_state = self.remote_slot_state();
        let (daemon_endpoint, server_url) = {
            let cfg = &self.client.lock().unwrap().config;
            (cfg.daemon_client_endpoint.clone(), cfg.server_url.clone())
        };
        let remote_status = if show_session_pill {
            self.remote_status_spans(remote_state, &daemon_endpoint)
        } else {
            Vec::new()
        };
        let playlist_status = if show_playlist_pill {
            self.playlist_status_spans()
        } else {
            Vec::new()
        };

        let alive_status: Option<Vec<Span>> = self.stay_alive_ctrl.is_some().then(|| {
            vec![
                Span::raw(" "),
                Span::styled(
                    if self.use_nerd_fonts {
                        "\u{f004}"
                    } else {
                        "\u{2665}"
                    },
                    Style::default().fg(palette::RED),
                ),
            ]
        });
        let mute_status = self.mute_status_spans();

        // Left-segment overflow priority: mute drops first if the combined
        // left segment wouldn't fit in the row, then playlist, then remote.
        let remote_w = Self::status_width(&remote_status);
        let playlist_w = Self::status_width(&playlist_status);
        let alive_w: u16 = alive_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let mute_w: u16 = mute_status
            .as_ref()
            .map(|spans| Self::status_width(spans))
            .unwrap_or(0);
        let available = area.width;
        let joined_width = |widths: &[u16]| -> u16 {
            let mut total = 0u16;
            for (count, width) in widths.iter().copied().filter(|w| *w > 0).enumerate() {
                total = total.saturating_add(width);
                if count > 0 {
                    total = total.saturating_add(1);
                }
            }
            total
        };
        let fits_all = joined_width(&[remote_w, playlist_w, alive_w, mute_w]) <= available;
        let fits_without_alive =
            !fits_all && joined_width(&[remote_w, playlist_w, mute_w]) <= available;
        let fits_without_mute =
            !fits_all && !fits_without_alive && joined_width(&[remote_w, playlist_w]) <= available;
        let fits_without_remote = !fits_all
            && !fits_without_alive
            && !fits_without_mute
            && joined_width(&[playlist_w, alive_w]) <= available;

        let show_remote = remote_w > 0 && (fits_all || fits_without_alive || fits_without_mute);
        // Playlist is present in every fit tier's width calculation (see
        // `joined_width` calls above), so its visibility should follow the
        // tiers directly rather than piggybacking on `show_remote` -- when the
        // remote pill is suppressed entirely (`show_session_pill: false`,
        // e.g. the Power View status bar), `show_remote` is always false and
        // that previously hid the playlist pill even when it fit fine.
        let show_playlist = playlist_w > 0
            && (fits_all || fits_without_alive || fits_without_mute || fits_without_remote);
        let show_alive =
            alive_status.is_some() && (fits_all || fits_without_mute || fits_without_remote);

        let mut spans: Vec<Span> = Vec::new();
        if show_alive {
            if let Some(alive) = alive_status.as_ref() {
                spans.extend(alive.iter().cloned());
            }
        }
        let remote_x =
            show_remote.then(|| area.x + Self::status_width(&spans) + u16::from(!spans.is_empty()));
        if show_remote {
            Self::append_status(&mut spans, remote_status);
        }
        if show_playlist {
            Self::append_status(&mut spans, playlist_status);
        }
        self.render_remote_status_hitbox(layout, area, remote_x, remote_w);
        if fits_all || fits_without_alive {
            if let Some(mute) = mute_status {
                let mute_x = area.x + Self::status_width(&spans);
                let mute_w = Self::status_width(&mute);
                Self::append_status(&mut spans, mute);
                layout.ind_mu = Rect {
                    x: mute_x,
                    y: area.y,
                    width: mute_w,
                    height: 1,
                };
            }
        }

        // `left_content_w` tracks how far the left segment actually extends after
        // the above priority drop, so the right-segment overlap check can compare
        // against the real left edge instead of a hardcoded constant.
        let label_w: u16 = spans.iter().map(|s| s.content.width() as u16).sum();
        let left_content_w: u16 = label_w;
        if !spans.is_empty() {
            let label_rect = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(spans)).style(bar_style),
                label_rect,
            );
        }

        {
            let mut right_spans: Vec<Span> = Vec::new();
            let source_label: Option<(String, Color)> = match &self.queue_source {
                crate::config::QueueSource::Playlist { .. } => None,
                crate::config::QueueSource::Album if self.tab_idx == 1 => {
                    Some(("ALBUM".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Series if self.tab_idx == 1 => {
                    Some(("SERIES".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Shuffle if self.tab_idx == 1 => {
                    Some(("SHUFFLE".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Remote if self.tab_idx == 1 => {
                    Some(("REMOTE Q".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Collection { collection_type } if self.tab_idx == 1 => {
                    Some((collection_type.to_uppercase(), palette::MUTED))
                }
                crate::config::QueueSource::Unknown => None,
                _ => None,
            };
            let append_right = |right_spans: &mut Vec<Span<'static>>, span: Span<'static>| {
                if !right_spans.is_empty() {
                    right_spans.push(Span::raw(" "));
                }
                right_spans.push(span);
            };
            if let Some((label, color)) = source_label {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        format!(" {label} "),
                        Style::default().fg(color).bg(palette::STATUS_PILL_BG),
                    ),
                );
            }
            let autosave_on = self.tab_idx == 1 && self.queue_is_saved_playlist() && {
                let cfg = &self.client.lock().unwrap().config;
                cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
            };
            if self.queue_dirty {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        " UNSAVED ",
                        Style::default()
                            .fg(palette::YELLOW)
                            .bg(palette::STATUS_PILL_BG)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            } else if autosave_on {
                append_right(
                    &mut right_spans,
                    Span::styled(
                        " AUTOSAVE ",
                        Style::default()
                            .fg(palette::AQUA)
                            .bg(palette::STATUS_PILL_BG),
                    ),
                );
            }
            if let Some(server) = server_url_label(&server_url) {
                if self.use_nerd_fonts {
                    if !right_spans.is_empty() {
                        right_spans.push(Span::raw(" "));
                    }
                    right_spans.push(Span::styled(
                        " \u{F06B4}",
                        Style::default()
                            .fg(palette::AQUA)
                            .bg(palette::STATUS_PILL_BG),
                    ));
                    right_spans.push(Span::styled(
                        format!(" {server} "),
                        Style::default()
                            .fg(palette::SUBTLE)
                            .bg(palette::STATUS_PILL_BG),
                    ));
                } else {
                    append_right(
                        &mut right_spans,
                        Span::styled(
                            format!(" {server} "),
                            Style::default()
                                .fg(palette::SUBTLE)
                                .bg(palette::STATUS_PILL_BG),
                        ),
                    );
                }
            }
            // Remote queue scope is omitted here: the active queue is already
            // apparent from the queue UI.
            if !right_spans.is_empty() {
                let right_w: u16 = right_spans.iter().map(|s| s.content.width() as u16).sum();
                // Compare against `left_content_w` (pill + session label, from Task 2),
                // not a hardcoded pill-only width -- otherwise this check passes while
                // the right segment still overlaps a rendered session label (e.g.
                // " ATTACHED" / " REMOTE ALIVE") on narrow terminals.
                let left_end = area.x + left_content_w;
                let right_x = area.x + area.width.saturating_sub(right_w);
                if right_x > left_end {
                    let right_rect = Rect {
                        x: right_x,
                        y: area.y,
                        width: right_w,
                        height: 1,
                    };
                    f.render_widget(
                        Paragraph::new(Line::from(right_spans)).style(bar_style),
                        right_rect,
                    );
                }
                // else: terminal too narrow for both segments -- right segment drops
                // silently rather than overlapping the pill or the session label.
                // (Design doc's open question on narrow-terminal truncation: right
                // segment yields first.)
            }
        }
    }

    /// Full-width seekbar row: green up to the playhead, gray for the remainder.
    /// No knob — the green/gray boundary marks the position. Records the click region.
    fn render_seekbar(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback) {
        if area.height == 0 || area.width == 0 {
            layout.seekbar_area = Rect::default();
            return;
        }
        let (pos_ticks, rt_ticks, _paused) = self.playback_progress();
        let ratio = if rt_ticks > 0 {
            (pos_ticks as f64 / rt_ticks as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        layout.seekbar_area = area;
        let w = area.width as usize;
        let green_len = ((ratio * w as f64).round() as usize).min(w);
        let gray_len = w - green_len;
        let spans = vec![
            Span::styled(
                "\u{2594}".repeat(green_len),
                Style::default().fg(palette::AQUA),
            ),
            Span::styled(
                "\u{2594}".repeat(gray_len),
                Style::default().fg(palette::SEEK_TRACK),
            ),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use crate::app::RemoteSlotState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn title_row_next_area_matches_rendered_next_glyph_width_and_position() {
        let mut app = make_app_stub();
        app.use_nerd_fonts = false;
        let next_glyph = ">>";
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
        }

        let backend = TestBackend::new(60, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "Title",
                palette::GREEN,
                &mut layout,
            );
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();
        let next_byte = line.find(next_glyph).unwrap();
        let next_x = line[..next_byte].width() as u16;

        assert_eq!(layout.next_area.x, next_x);
        assert_eq!(layout.next_area.width, next_glyph.width() as u16);
        assert!(
            line.starts_with("|| X >> Title"),
            "expected stop then next glyph between play/pause and title:\n{line}"
        );
    }

    #[test]
    fn title_row_next_area_matches_nerd_font_glyph_width_and_position() {
        let mut app = make_app_stub();
        app.use_nerd_fonts = true;
        let next_glyph = "\u{f051}";
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
        }

        let backend = TestBackend::new(60, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "Title",
                palette::GREEN,
                &mut layout,
            );
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();
        let next_byte = line.find(next_glyph).unwrap();
        let next_x = line[..next_byte].width() as u16;

        assert_eq!(layout.next_area.x, next_x);
        assert_eq!(layout.next_area.width, next_glyph.width() as u16);
    }

    #[test]
    fn title_row_truncates_long_title_before_transport_status_and_badges() {
        let mut app = make_app_stub();
        app.use_nerd_fonts = false;
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 2;
            st.current_idx = 0;
            st.position_ticks = 65 * TICKS_PER_SECOND;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
            st.video_height = 1080;
            st.audio_lang = "en".into();
        }

        let backend = TestBackend::new(53, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_title_row(
                f,
                Rect::new(0, 0, 53, 1),
                "This is an extremely long title that would otherwise push controls away",
                palette::GREEN,
                &mut layout,
            );
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();

        assert!(
            line.contains('\u{2026}'),
            "expected long title to be truncated with ellipsis:\n{line}"
        );
        assert!(
            line.contains("1:05 / 1:30"),
            "expected time cluster to remain visible:\n{line}"
        );
        assert!(
            line.ends_with("RES 1080p  AUD en  SUB off"),
            "expected status badges to remain right-aligned:\n{line}"
        );
        assert!(layout.next_area.x + layout.next_area.width <= 53);
    }

    #[test]
    fn status_bar_remote_hitbox_tracks_visible_pill_after_alive_marker() {
        let mut app = make_app_stub();
        let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
        app.stay_alive_ctrl = Some(crate::app::stay_alive::StayAliveCtrl::for_test(app_end));

        let backend = TestBackend::new(80, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_status_bar(f, Rect::new(0, 0, 80, 1), &mut layout, true, true);
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();
        let heart_byte = line.find('\u{2665}').unwrap();
        let remote_byte = line.find('\u{1F5A7}').unwrap();
        let heart_x = line[..heart_byte].width() as u16;
        let remote_x = line[..remote_byte].width() as u16;

        assert!(
            layout.ind_rc.contains((remote_x, 0).into()),
            "expected the remote hitbox to cover the rendered remote pill:\n{line}"
        );
        assert!(
            !layout.ind_rc.contains((heart_x, 0).into()),
            "expected the stay-alive heart to stay outside the sessions hitbox:\n{line}"
        );
    }

    #[test]
    fn status_bar_omits_alive_marker_when_overflow_chooses_without_alive() {
        let mut app = make_app_stub();
        let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
        app.stay_alive_ctrl = Some(crate::app::stay_alive::StayAliveCtrl::for_test(app_end));

        let remote_status = app.remote_status_spans(RemoteSlotState::Off, "");
        let playlist_status = app.playlist_status_spans();
        let width = App::status_width(&remote_status) + App::status_width(&playlist_status) + 1;

        let backend = TestBackend::new(width, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPlayback::default();
        term.draw(|f| {
            app.render_status_bar(f, Rect::new(0, 0, width, 1), &mut layout, true, true);
        })
        .unwrap();

        let line = buffer_to_string(&term).lines().next().unwrap().to_string();
        assert!(
            !line.contains('\u{2665}'),
            "expected overflow to drop the stay-alive marker before rendering:\n{line}"
        );
        assert!(
            line.contains('\u{1F5A7}') && line.contains('\u{1F5AD}'),
            "expected remote and playlist pills to remain visible:\n{line}"
        );
    }

    #[test]
    fn remote_status_spans_prefers_active_route_label_over_daemon_endpoint() {
        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "tcp://127.0.0.1:9000");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("music"));
    }

    #[test]
    fn status_label_style_uppercases_and_bolds_selected_label() {
        let mut spans = vec![
            Span::raw(" "),
            Span::raw("icon"),
            Span::styled("  living-room", Style::default().fg(palette::SUBTLE)),
            Span::raw(" "),
        ];

        App::uppercase_status_label(&mut spans);
        App::set_status_label_bold(&mut spans, true);

        assert_eq!(spans[2].content.as_ref(), "  LIVING-ROOM");
        assert!(spans[2].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn expired_toast_clears_before_status_bar_render_decides_overlay() {
        let mut app = make_app_stub();
        app.status = "Saved [Y]".to_string();
        app.status_expires = Some(std::time::Instant::now() - std::time::Duration::from_millis(1));

        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| app.render(f)).unwrap();

        let last_line = buffer_to_string(&term).lines().last().unwrap().to_string();
        assert!(
            !last_line.contains("Saved"),
            "expected expired toast text to clear before the status bar chooses its row:\n{last_line}"
        );
        assert!(
            last_line.contains('\u{1F5AD}'),
            "expected the persistent status bar to render after an expired toast clears:\n{last_line}"
        );
        assert!(app.status.is_empty());
        assert!(app.status_expires.is_none());
    }

    fn render_sidebar_scrollbar_column(total: usize, visible: u16, scroll: usize) -> String {
        let backend = TestBackend::new(1, visible);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            App::render_sidebar_scrollbar(f, Rect::new(0, 0, 0, visible), total, scroll);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    #[test]
    fn sidebar_scrollbar_reaches_top_and_bottom_with_paragraph_offsets() {
        let top = render_sidebar_scrollbar_column(10, 5, 0);
        let bottom = render_sidebar_scrollbar_column(10, 5, 5);

        assert!(top.lines().next().is_some_and(|line| line != "│"));
        assert!(bottom.lines().last().is_some_and(|line| line != "│"));
        assert_eq!(
            top.lines().filter(|line| *line != "│").count(),
            bottom.lines().filter(|line| *line != "│").count()
        );
        assert!(top.chars().all(|c| c == '│' || c == '▕' || c == '\n'));
        assert_ne!(top, bottom);
    }

    #[test]
    fn sidebar_scrollbar_uses_scrollbar_color() {
        let backend = TestBackend::new(1, 5);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            App::render_sidebar_scrollbar(f, Rect::new(0, 0, 0, 5), 10, 0);
        })
        .unwrap();

        let buf = term.backend().buffer();
        assert_ne!(buf[(0, 0)].symbol(), "│");
        assert_eq!(buf[(0, 0)].fg, palette::SCROLLBAR);
        assert_eq!(buf[(0, 4)].symbol(), "│");
        assert_eq!(buf[(0, 4)].fg, palette::SCROLLBAR);
    }
}
