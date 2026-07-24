mod album;
mod card;
mod detail;
mod home;
pub mod indicators;
mod list;
mod music;
mod overlays;
mod pills;
mod queue;

use super::ui_util::{build_queue_rows, fmt_duration, natural_sort_key, trunc_str, QueueRow};
use super::{layout::AppLayout, palette, App, PanelFocus};
use crate::app::layout::{LayoutMain, LayoutPlayback};
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use std::time::Instant;
use textwrap::wrap;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

pub(super) fn thin_vertical_thumb(mut glyphs: GlyphSet) -> GlyphSet {
    glyphs.thumb_vertical_lower = ['▕'; 8];
    glyphs.thumb_vertical_upper = ['▕'; 8];
    glyphs
}

/// Height of the tab-bar box: 1 row padding + 1 row tab + 1 row spacer.
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

    fn toast_line(s: &str) -> Line<'static> {
        let text_style = Style::default()
            .fg(palette::TOAST_FG)
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
                    spans.push(Span::styled(c.to_string(), text_style));
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
        Self::render_panel_shell_at(f, sidebar, title, hints, false)
    }

    pub(super) fn panel_content_area(sidebar: Rect) -> Rect {
        Rect {
            x: sidebar.x,
            y: sidebar.y + 1,
            width: sidebar.width.saturating_sub(1),
            height: sidebar.height.saturating_sub(3),
        }
    }

    pub(super) fn power_panel_content_area(sidebar: Rect) -> Rect {
        Rect {
            x: sidebar.x + 2,
            y: sidebar.y + 3,
            width: sidebar.width.saturating_sub(4),
            height: sidebar.height.saturating_sub(5),
        }
    }

    pub(super) fn settings_content_area(content: Rect) -> Rect {
        Rect {
            x: content.x.saturating_add(2),
            y: content.y.saturating_add(1),
            width: content.width.saturating_sub(4),
            height: content.height.saturating_sub(2),
        }
    }

    pub(super) fn render_panel_shell_at(
        f: &mut Frame,
        sidebar: Rect,
        title: &str,
        hints: &str,
        power_style: bool,
    ) -> Rect {
        f.render_widget(Clear, sidebar);
        // Too short to fit a title row, a content row, and the 2-row footer;
        // bail out rather than let `footer_y = sidebar.y + sidebar.height - 2`
        // underflow below.
        if sidebar.height < 4 || sidebar.width == 0 {
            return if power_style {
                Self::power_panel_content_area(sidebar)
            } else {
                sidebar
            };
        }
        f.render_widget(
            Block::default().style(Style::default().bg(if power_style {
                palette::PLAYBACK_PANEL_BG
            } else {
                palette::PANEL_BG
            })),
            sidebar,
        );
        if !power_style {
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
        }
        let (inner_w, ix) = if power_style {
            (sidebar.width.saturating_sub(4), sidebar.x + 2)
        } else {
            (sidebar.width.saturating_sub(1), sidebar.x)
        };
        let header_style = Style::default()
            .fg(palette::TEXT)
            .bg(if power_style {
                palette::QUEUE_BUTTON_FOCUSED_BG
            } else {
                palette::FOCUSED
            })
            .add_modifier(Modifier::BOLD);
        let header_area = if power_style {
            Rect {
                x: sidebar.x + 2,
                y: sidebar.y + 1,
                width: sidebar.width.saturating_sub(4),
                height: 1,
            }
        } else {
            Rect {
                x: sidebar.x,
                y: sidebar.y,
                width: sidebar.width.saturating_sub(1),
                height: 1,
            }
        };
        let title_text = if power_style {
            format!(" {}", title)
        } else {
            title.to_owned()
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(title_text, header_style)])).style(
                if power_style {
                    Style::default().bg(palette::QUEUE_BUTTON_FOCUSED_BG)
                } else {
                    Style::default().bg(palette::FOCUSED)
                },
            ),
            header_area,
        );
        if !power_style {
            f.render_widget(
                Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
                Rect {
                    x: sidebar.x + sidebar.width - 1,
                    y: sidebar.y,
                    width: 1,
                    height: 1,
                },
            );
        }
        let footer_y = sidebar.y + sidebar.height - 2;
        if !power_style {
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
        }
        let footer_bg = if power_style {
            palette::DARK_BG
        } else {
            palette::FOCUSED
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                trunc_str(hints, inner_w as usize),
                Style::default().fg(palette::TEXT),
            )]))
            .style(Style::default().bg(footer_bg)),
            Rect {
                x: ix,
                y: footer_y,
                width: inner_w,
                height: 1,
            },
        );
        if power_style {
            f.render_widget(
                Paragraph::new(Span::raw(""))
                    .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
                Rect {
                    x: sidebar.x,
                    y: sidebar.y + sidebar.height - 1,
                    width: sidebar.width,
                    height: 1,
                },
            );
        }
        if !power_style {
            f.render_widget(
                Paragraph::new(Span::raw(" ")).style(Style::default().bg(palette::FOCUSED)),
                Rect {
                    x: sidebar.x + sidebar.width - 1,
                    y: footer_y,
                    width: 1,
                    height: 1,
                },
            );
        }
        if power_style {
            Self::power_panel_content_area(sidebar)
        } else {
            Self::panel_content_area(sidebar)
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
            Span::styled("VOL ", Style::default().fg(palette::PLAYBACK_META_FG)),
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
        let all_names: Vec<String> = std::iter::once("Home".to_string())
            .chain(self.libs.iter().map(|l| l.library.name.clone()))
            .collect();
        let selected_tab = if self.library_tab < vis_start || self.library_tab >= vis_end {
            usize::MAX
        } else {
            self.library_tab - vis_start
        };
        let tab_titles: Vec<Line> = all_names[vis_start..vis_end]
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
                        Style::default().fg(palette::PLAYBACK_META_FG),
                    ))
                }
            })
            .collect();
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
                Paragraph::new(Span::styled(bar, Style::default().fg(palette::SEEK_TRACK)))
                    .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
                seek_area,
            );
        }
        // Title row (when panel is expanded).
        if player_h >= 2 {
            const H_PAD: u16 = 2;
            let title_row_area = Rect {
                y: area.y + 1,
                height: 1,
                ..area
            };
            f.render_widget(
                Paragraph::new(Span::raw(" ".repeat(title_row_area.width as usize)))
                    .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
                title_row_area,
            );
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

        if player_h >= 3 {
            let blank_area = Rect {
                y: area.y + 2,
                height: 1,
                ..area
            };
            f.render_widget(
                Paragraph::new(Span::raw(" ".repeat(blank_area.width as usize)))
                    .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
                blank_area,
            );
        }

        if player_h >= 4 {
            let border_area = Rect {
                y: area.y + 3,
                height: 1,
                ..area
            };
            let border = "\u{2594}".repeat(border_area.width as usize);
            f.render_widget(
                Paragraph::new(Span::styled(
                    border,
                    Style::default().fg(palette::SEEK_TRACK),
                )),
                border_area,
            );
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
        let mut codec_value_next = false;
        let right = self
            .build_status_indicator_spans()
            .unwrap_or_default()
            .into_iter()
            .map(|span| {
                let is_caption =
                    matches!(span.content.as_ref(), "CODEC " | "RES " | "AUD " | "SUB ");
                let is_codec_caption = span.content.as_ref() == "CODEC ";
                if is_codec_caption {
                    codec_value_next = true;
                    Span::styled(
                        span.content.to_string(),
                        span.style.fg(palette::PLAYBACK_META_FG),
                    )
                } else if codec_value_next {
                    codec_value_next = false;
                    Span::styled(
                        span.content.to_string(),
                        span.style.fg(palette::PLAYBACK_CONTENT_FG),
                    )
                } else if is_caption {
                    Span::styled(
                        span.content.to_string(),
                        span.style.fg(palette::PLAYBACK_META_FG),
                    )
                } else {
                    span
                }
            })
            .collect::<Vec<_>>();

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
            Style::default().fg(palette::PLAYBACK_META_FG),
        ));

        left.push(Span::raw(post_time_gap));

        let left_w: u16 = left.iter().map(|s| s.content.width() as u16).sum();
        let gap = area.width.saturating_sub(left_w + right_w) as usize;

        let mut spans = left;
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right);
        f.render_widget(
            Paragraph::new(Line::from(spans))
                .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
            area,
        );
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
        let (daemon_endpoint, server_url, username) = {
            let cfg = &self.client.lock().unwrap().config;
            (
                cfg.daemon_client_endpoint.clone(),
                cfg.server_url.clone(),
                cfg.username.clone(),
            )
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
                crate::config::QueueSource::Album
                    if matches!(self.panel_focus, super::PanelFocus::Queue) =>
                {
                    Some(("ALBUM".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Series
                    if matches!(self.panel_focus, super::PanelFocus::Queue) =>
                {
                    Some(("SERIES".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Shuffle
                    if matches!(self.panel_focus, super::PanelFocus::Queue) =>
                {
                    Some(("SHUFFLE".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Remote
                    if matches!(self.panel_focus, super::PanelFocus::Queue) =>
                {
                    Some(("REMOTE Q".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Collection { collection_type }
                    if matches!(self.panel_focus, super::PanelFocus::Queue) =>
                {
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
            let autosave_on = matches!(self.panel_focus, super::PanelFocus::Queue)
                && self.queue_is_saved_playlist()
                && {
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
            if !username.is_empty() {
                if !right_spans.is_empty() {
                    right_spans.push(Span::raw(" "));
                }
                right_spans.push(Span::styled(
                    " 🯅",
                    Style::default()
                        .fg(palette::FOAM)
                        .bg(palette::STATUS_PILL_BG),
                ));
                right_spans.push(Span::styled(
                    format!(" {username} "),
                    Style::default()
                        .fg(palette::PLAYBACK_META_FG)
                        .bg(palette::STATUS_PILL_BG),
                ));
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
        f.render_widget(
            Paragraph::new(Line::from(spans))
                .style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
            area,
        );
    }
}

// Power View re-renders frequently while scrolling; prefer a cheaper filter in
// these hot paths to reduce terminal image preparation stalls.
pub(super) const POWER_RENDER_FILTER: ratatui_image::FilterType =
    ratatui_image::FilterType::Triangle;

// Configured music albums need the image worker's child-audio lookup; their
// album containers do not reliably expose usable Primary images.
const MUSIC_ALBUM_IMAGE_TYPES: &[&str] = &["AudioChild"];

/// Columns of empty space between the left and right panels in power view.
const POWER_VIEW_GAP: u16 = 0;

/// Left-edge padding applied once to every power-view tab's content area
/// (Home, library lists, music groups, albums, series, home-video, feed
/// groups) plus the music-group pills row, so all tabs share a consistent
/// gutter. Applied at the single dispatch chokepoint in the main render
/// fn; individual tab renderers add only their own content-level gutters
/// (marker columns, banner indents) relative to this padded edge.
///
/// Detail surfaces that need additional internal alignment can add their own
/// indentation relative to this padded edge.
pub(super) const POWER_TAB_LEFT_PAD: u16 = 2;

fn power_right_panel_content_area(area: Rect, left_collapsed: bool) -> Rect {
    if left_collapsed {
        Rect {
            width: area.width.saturating_sub(1),
            ..area
        }
    } else {
        Rect {
            x: area.x + POWER_TAB_LEFT_PAD,
            width: area
                .width
                .saturating_sub(POWER_TAB_LEFT_PAD.saturating_mul(2)),
            ..area
        }
    }
}

pub(super) fn render_power_scrollbar(f: &mut Frame, area: Rect, max_offset: usize, offset: usize) {
    let visible = area.height as usize;
    render_power_scrollbar_with_viewport(
        f,
        area,
        max_offset.saturating_add(visible),
        visible,
        offset,
    );
}

pub(super) fn render_power_scrollbar_with_viewport(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) {
    render_power_scrollbar_with_viewport_at(
        f,
        area,
        content_length,
        viewport_content_length,
        offset,
        area.x + area.width.saturating_sub(1),
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SCROLLBAR,
    );
}

pub(super) fn render_power_right_scrollbar(
    f: &mut Frame,
    area: Rect,
    max_offset: usize,
    offset: usize,
) {
    let visible = area.height as usize;
    let x = if area.right() < f.area().right() {
        area.right()
    } else {
        area.x + area.width.saturating_sub(1)
    };
    render_power_scrollbar_with_viewport_at(
        f,
        area,
        max_offset.saturating_add(visible),
        visible,
        offset,
        x,
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SCROLLBAR,
    );
}

pub(super) fn render_power_right_scrollbar_with_viewport(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) {
    let x = if area.right() < f.area().right() {
        area.right()
    } else {
        area.x + area.width.saturating_sub(1)
    };
    render_power_scrollbar_with_viewport_at(
        f,
        area,
        content_length,
        viewport_content_length,
        offset,
        x,
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SCROLLBAR,
    );
}

fn render_power_scrollbar_with_viewport_at(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
    x: u16,
    glyph_set: GlyphSet,
    scrollbar_color: Color,
) {
    if area.height == 0 || viewport_content_length == 0 || content_length <= viewport_content_length
    {
        return;
    }
    let max_offset = content_length.saturating_sub(viewport_content_length);
    let scrollbar = ScrollBar::vertical(ScrollLengths {
        content_len: content_length,
        viewport_len: viewport_content_length,
    })
    .offset(offset.min(max_offset))
    .glyph_set(glyph_set)
    .track_style(Style::default().fg(scrollbar_color))
    .thumb_style(Style::default().fg(scrollbar_color));
    f.render_widget(
        &scrollbar,
        Rect {
            x,
            width: 1,
            ..area
        },
    );
}

/// Paints a colored background block spanning display rows `[top_pad_abs, bottom_pad_abs]`
/// (absolute/unscrolled indices into the complete display row sequence), clamped to the
/// visible scroll window `[offset, offset+visible)`. The block fills the full row width
/// supplied by `area.x` and `area.width` (interior content can indent itself further).
/// Call before rendering list/row content so the background shows through.
pub(super) fn render_selected_block_background(
    f: &mut Frame,
    area: Rect,
    offset: usize,
    visible: usize,
    top_pad_abs: usize,
    bottom_pad_abs: usize,
    bg: Color,
) {
    let vis_top = top_pad_abs.max(offset);
    let vis_bot = bottom_pad_abs.min(offset + visible.saturating_sub(1));
    if vis_top <= vis_bot {
        let block_y = area.y + (vis_top - offset) as u16;
        let block_h = (vis_bot - vis_top + 1) as u16;
        f.render_widget(
            Block::default().style(Style::default().bg(bg)),
            Rect {
                x: area.x,
                y: block_y,
                width: area.width,
                height: block_h,
            },
        );
    }
}

/// Paints the ▁/▔ border rows on the reserved rows one position outside
/// the colored block's padding rows `[top_pad_abs, bottom_pad_abs]`.
/// The padding rows are inserted with extra detail rule rows for border space.
/// Call *after* the block's own content and scrollbar render, so borders paint on top.
pub(super) fn render_selected_block_borders(
    f: &mut Frame,
    area: Rect,
    offset: usize,
    visible: usize,
    top_pad_abs: usize,
    bottom_pad_abs: usize,
) {
    let border_style = Style::default().fg(palette::SEEK_TRACK);
    // Top border: paint one row before the colored block padding
    if let Some(top_border) = top_pad_abs.checked_sub(1) {
        if top_border >= offset && top_border < offset + visible {
            let top_y = area.y + (top_border - offset) as u16;
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2581}".repeat(area.width as usize),
                    border_style,
                ))),
                Rect {
                    x: area.x,
                    y: top_y,
                    width: area.width,
                    height: 1,
                },
            );
        }
    }
    // Bottom border: paint one row after the colored block padding
    let bot_border = bottom_pad_abs + 1;
    if bot_border >= offset && bot_border < offset + visible {
        let bot_y = area.y + (bot_border - offset) as u16;
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "\u{2594}".repeat(area.width as usize),
                border_style,
            ))),
            Rect {
                x: area.x,
                y: bot_y,
                width: area.width,
                height: 1,
            },
        );
    }
}

fn render_power_queue_panel_frame(
    f: &mut Frame,
    area: Rect,
    desired_rows: u16,
    focused: bool,
) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    let bg = if focused {
        palette::MEDIA_SELECTED_BG
    } else {
        palette::LIBRARY_SIDE_BG
    };
    f.render_widget(Block::default().style(Style::default().bg(bg)), area);

    let border_style = Style::default().fg(palette::SEEK_TRACK);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "\u{2594}".repeat(area.width as usize),
            border_style,
        ))),
        Rect { height: 1, ..area },
    );
    if area.height > 1 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "\u{2581}".repeat(area.width as usize),
                border_style,
            ))),
            Rect {
                y: area.y + area.height - 1,
                height: 1,
                ..area
            },
        );
    }

    let border_rows = area.height.min(2);
    let use_padding = area.height >= desired_rows.saturating_add(4);
    let top_decoration = 1 + u16::from(use_padding);
    let height = area
        .height
        .saturating_sub(border_rows)
        .saturating_sub(if use_padding { 2 } else { 0 });

    Rect {
        y: area.y + top_decoration,
        height,
        ..area
    }
}

fn rendered_power_queue_rows_for_padding(items: &[MediaItem], panel_area: Rect) -> u16 {
    if items.is_empty() {
        return 1;
    }

    let (display, group_for_header) = build_power_queue_rows(items);
    let padded_visible = panel_area.height.saturating_sub(4) as usize;
    let has_sb = display.len() > padded_visible;
    let render_w = panel_area.width.saturating_sub(u16::from(has_sb)) as usize;
    let wrap_w = render_w.saturating_sub(1).max(1);
    let mut header_idx = 0;
    let mut rows = 0u16;

    for entry in display {
        match entry {
            QueueRow::Header => {
                let group = group_for_header
                    .get(header_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                header_idx += 1;
                rows = rows.saturating_add(wrap(group, wrap_w).len().max(1) as u16);
            }
            QueueRow::Spacer | QueueRow::Track { .. } => rows = rows.saturating_add(1),
        }
    }

    rows
}

pub(super) fn build_power_queue_rows(items: &[MediaItem]) -> (Vec<QueueRow>, Vec<String>) {
    let (display, group_for_header) = build_queue_rows(items, true);
    let mut rows = Vec::with_capacity(display.len().saturating_add(group_for_header.len()));

    for row in display {
        rows.push(row.clone());
        if matches!(row, QueueRow::Header) {
            rows.push(QueueRow::Spacer);
        }
    }

    (rows, group_for_header)
}

/// Style for a selector pill (group/section/artist tab row): dark active text
/// on YELLOW, yellow inactive text on the dark pill background. Shared by
/// every power-view pill row (home's group/section pills, music's group
/// pills) so they can't drift apart on the selected-vs-unselected look.
pub(super) fn selector_pill_style(selected: bool) -> Style {
    if selected {
        Style::default().fg(palette::PILL_DARK).bg(palette::YELLOW)
    } else {
        Style::default()
            .fg(palette::YELLOW)
            .bg(palette::LIBRARY_SIDE_BG)
    }
}

/// Draws the shared " {count} items" header (SUBTLE) on the first row of
/// `area` and returns `area` shrunk by that one row, so callers can render
/// their list into the remaining space. Used by the home-video tab to keep
/// the label styling and the one-row consumption identical to other tabs
/// that once shared it (movies/tv show library lists no longer show this
/// row; see `render_power_list`).
pub(super) fn render_power_count_label(f: &mut Frame, area: Rect, count: usize) -> Rect {
    if area.width == 0 || area.height == 0 {
        return area;
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {} items", count),
            Style::default().fg(palette::SUBTLE),
        )),
        Rect { height: 1, ..area },
    );
    Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    }
}

/// The shared left cursor marker span used by every power-view list row.
/// `active` (row is both selected and focused) renders the AQUA half-block
/// `▌`; otherwise a single blank space so unselected rows stay aligned.
/// Only the marker glyph is unified here -- each renderer keeps its own
/// row *text* coloring, which varies by tab.
pub(super) fn selection_marker(active: bool) -> Span<'static> {
    if active {
        Span::styled("\u{258c}", Style::default().fg(palette::AQUA))
    } else {
        Span::raw(" ")
    }
}

/// Width in columns reserved for a power-view list's scrollbar gutter.
pub(super) const POWER_SCROLLBAR_GUTTER: u16 = 1;

/// Usable text width of a list column of the given `width` once the
/// scrollbar gutter is reserved (when `needs_scrollbar`). Centralizes the
/// `width - gutter` arithmetic every scrolling list repeats.
pub(super) fn power_content_width(width: u16, needs_scrollbar: bool) -> usize {
    let gutter = if needs_scrollbar {
        POWER_SCROLLBAR_GUTTER
    } else {
        0
    };
    width.saturating_sub(gutter) as usize
}

/// What to draw behind a pill bar before the pills are overlaid.
pub(super) enum PillUnderlay {
    /// No divider. `fill` clears the row's trailing cells with blanks so the
    /// pills float on the panel background (used by the music-group tabs);
    /// `fill: false` leaves the trailing cells untouched (feed-group tabs).
    Blank { fill: bool },
}

/// A horizontally-scrolling row of selector pills, shared by every power-view
/// pill bar (Home's "Newest" section pills, feed-group tabs, music-group
/// tabs) so their scroll/overflow/selection behavior can't drift apart.
/// Callers pre-truncate `labels`, supply the parallel `ids` recorded as click
/// targets, mark which position is `selected_pos`, and choose an optional
/// leading `prefix` label and the `underlay`.
pub(super) struct PillBar<'a> {
    pub labels: &'a [String],
    pub ids: &'a [usize],
    pub selected_pos: usize,
    pub prefix: Option<&'a str>,
    pub underlay: PillUnderlay,
}

/// Renders `bar` into `area`, scrolling the visible window so `selected_pos`
/// stays on screen with `‹`/`›` chevrons when the pills overflow, and returns
/// the on-screen hitboxes as `(rect, id)` pairs for `layout.selector_tabs`.
pub(super) fn render_pill_bar(f: &mut Frame, area: Rect, bar: PillBar) -> Vec<(Rect, usize)> {
    // `ids` runs parallel to `labels`; a mismatch would panic on the slice
    // below, so assert the contract up front rather than fail cryptically.
    debug_assert_eq!(
        bar.labels.len(),
        bar.ids.len(),
        "render_pill_bar: labels and ids must be parallel"
    );
    let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();
    if area.width == 0 || area.height == 0 || bar.labels.is_empty() {
        return selector_tabs;
    }
    let n = bar.labels.len();
    let bar_w = area.width as usize;
    let prefix_w = bar.prefix.map(|p| p.width()).unwrap_or(0);
    // Display width of each pill is " label " = label width + 2.
    let pill_widths: Vec<usize> = bar.labels.iter().map(|l| l.width() + 2).collect();

    // Greedy: how many pills fit starting at `start` within `avail` columns
    // (1-column gap between consecutive pills).
    let count_fitting = |start: usize, avail: usize| -> usize {
        let mut used = 0usize;
        let mut count = 0usize;
        for width in pill_widths.iter().skip(start) {
            let need = if count == 0 { *width } else { 1 + *width };
            if used + need > avail {
                break;
            }
            used += need;
            count += 1;
        }
        count
    };

    // Advance the scroll window until the selected pill is visible.
    let mut scroll_start = 0usize;
    loop {
        let avail = bar_w
            .saturating_sub(prefix_w)
            .saturating_sub(if scroll_start > 0 { 2 } else { 0 }) // "‹ "
            .saturating_sub(2); // reserve for " ›"
        let cnt = count_fitting(scroll_start, avail);
        if cnt == 0 || scroll_start + cnt > bar.selected_pos {
            break;
        }
        scroll_start += 1;
    }

    let has_left = scroll_start > 0;
    let avail_pills = bar_w
        .saturating_sub(prefix_w)
        .saturating_sub(if has_left { 2 } else { 0 })
        .saturating_sub(2); // reserve for " ›"
    let cnt = count_fitting(scroll_start, avail_pills);
    let scroll_end = (scroll_start + cnt).min(n);
    let has_right = scroll_end < n;

    let mut spans: Vec<Span> = Vec::new();
    let mut x_cursor = area.x;
    if let Some(prefix) = bar.prefix {
        // White label, no background, so an underlay rule shows around it.
        spans.push(Span::styled(
            prefix.to_string(),
            Style::default().fg(Color::White),
        ));
        x_cursor += prefix_w as u16;
    }
    if has_left {
        let chunk = "\u{2039} ";
        spans.push(Span::styled(chunk, Style::default().fg(palette::GREEN)));
        x_cursor += chunk.width() as u16;
    }
    for (offset, (label, &id)) in bar.labels[scroll_start..scroll_end]
        .iter()
        .zip(bar.ids[scroll_start..scroll_end].iter())
        .enumerate()
    {
        if offset > 0 {
            // Single blank gap so the pills float free rather than sitting on
            // a continuous divider.
            spans.push(Span::raw(" "));
            x_cursor += 1;
        }
        let abs_idx = scroll_start + offset;
        let style = selector_pill_style(abs_idx == bar.selected_pos);
        let pill = format!(" {} ", label);
        let pill_w = pill.width() as u16;
        selector_tabs.push((
            Rect {
                x: x_cursor,
                y: area.y,
                width: pill_w,
                height: 1,
            },
            id,
        ));
        spans.push(Span::styled(pill, style));
        x_cursor += pill_w;
    }
    if has_right {
        let chunk = " \u{203a}";
        spans.push(Span::styled(chunk, Style::default().fg(palette::GREEN)));
        x_cursor += chunk.width() as u16;
    }

    // With no rule underlay, optionally clear the rest of the row with blanks.
    if let PillUnderlay::Blank { fill: true } = bar.underlay {
        let used_w = x_cursor.saturating_sub(area.x) as usize;
        let remaining = bar_w.saturating_sub(used_w);
        if remaining > 0 {
            spans.push(Span::raw(" ".repeat(remaining)));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
    selector_tabs
}

/// Draws a shared empty/loading placeholder message (MUTED) at `area`.
/// Callers pass the exact text (`" (empty)"`, `" Loading…"`, or a
/// context-specific string like `"Indexing music library..."`) so the
/// wording stays local, but the placeholder styling is defined once.
pub(super) fn render_power_placeholder(f: &mut Frame, area: Rect, msg: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            msg.to_string(),
            Style::default().fg(palette::MUTED),
        )),
        area,
    );
}

/// For folder-based music libraries where albums are stored as directories named
/// "Artist (YYYY) Album Title", parse out the three components.
/// Returns `(artist, year, album_title)` on success.
pub(super) fn parse_album_folder_name(name: &str) -> Option<(String, u32, String)> {
    let mut search_from = 0;
    while let Some(rel) = name[search_from..].find(" (") {
        let sp_pos = search_from + rel; // position of the space before '('
        let after_open = sp_pos + 2; // position of first char after '('
        if let Some(close_rel) = name[after_open..].find(')') {
            let year_str = &name[after_open..after_open + close_rel];
            if year_str.len() == 4 {
                if let Ok(year) = year_str.parse::<u32>() {
                    let close_pos = after_open + close_rel; // position of ')'
                    if name[close_pos..].starts_with(") ") {
                        let artist = name[..sp_pos].to_string();
                        let album = name[close_pos + 2..].to_string();
                        return Some((artist, year, album));
                    }
                }
            }
        }
        search_from = sp_pos + 2;
    }
    None
}

/// Strips a leading article ("The ", "A ", "An ") from `s` (case-insensitive).
/// Returns a slice of the original string starting after the article.
fn strip_article(s: &str) -> &str {
    for prefix in &["the ", "a ", "an "] {
        // `s.get(..prefix.len())` returns `None` (rather than panicking, as a
        // byte-index slice would) when `prefix.len()` doesn't land on a UTF-8
        // char boundary — e.g. an accented artist name where the boundary
        // falls inside a multi-byte character.
        if let Some(head) = s.get(..prefix.len()) {
            if head.eq_ignore_ascii_case(prefix) {
                return &s[prefix.len()..];
            }
        }
    }
    s
}

/// Best-effort natural sort key for an album's display artist, computed
/// synchronously (Emby tag or folder-name heuristic only — no network fetch,
/// no cache lookup). Used to pick a sane initial cursor position when a
/// music-group album level first loads (see `handle_lib_event`'s
/// `LibEvent::Loaded` arm in `actions.rs`), before
/// `App::resolve_group_album_artist`'s async fetch has had a chance to run.
/// Mirrors that method's synchronous fallback chain (Emby tag →
/// folder-name-parsed artist → literal "Unknown Artist"), minus the
/// cache/fetch steps, since nothing is cached yet at initial load.
pub(crate) fn initial_group_artist_sort_key(item: &mbv_core::api::MediaItem) -> String {
    let artist = if !item.artist.is_empty() {
        item.artist.clone()
    } else if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
        artist
    } else {
        "Unknown Artist".to_string()
    };
    natural_sort_key(strip_article(&artist))
}

/// Returns the effective sort key for an item: `sort_name` when Emby provides it,
/// otherwise the item's display name with any leading article stripped.
pub(super) fn effective_sort_str(item: &mbv_core::api::MediaItem) -> &str {
    if !item.sort_name.is_empty() {
        &item.sort_name
    } else {
        strip_article(&item.name)
    }
}

/// Returns the letter-group bucket label for `item` given `total` items in the list.
/// Uses `sort_name` when available (so "The Wire" → 'W'), otherwise the article-stripped
/// name. "#" for titles starting with a digit or non-letter; ranges for 50–999 items;
/// individual letters for 250+ items.
pub(super) fn letter_bucket(item: &mbv_core::api::MediaItem, total: usize) -> String {
    let key = effective_sort_str(item);
    let first = key
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('\0');
    // KNOWN LIMITATION: any non-ASCII-alphabetic first character (accented
    // letters like "Æon"/"Élan" included, codepoint > 'Z') buckets here as
    // "#". But the "#" *pill*'s Emby fetch bounds are `NameLessThan("A")`
    // -- only titles that SORT BEFORE "A" -- so an accented title with a
    // codepoint after 'Z' is actually fetched by the `V–Z` pill
    // (`name_ge = "V"`, no upper bound) yet renders under this "#" header,
    // making it unreachable from the "#" pill's scoped fetch. Fixing this
    // would mean either teaching the "#" pill to also request `V–Z`-range
    // items with a non-ASCII-alphabetic first char (an Emby-side filter
    // that doesn't exist), or bucketing accented letters under their
    // unaccented equivalent instead of "#" (a bigger behavior change than
    // this pass intends). Left as-is; flagged for a follow-up.
    if !first.is_ascii_alphabetic() {
        return "#".to_string();
    }
    if total >= 250 {
        return first.to_string();
    }
    match first {
        'A'..='C' => "A\u{2013}C",
        'D'..='F' => "D\u{2013}F",
        'G'..='I' => "G\u{2013}I",
        'J'..='L' => "J\u{2013}L",
        'M'..='O' => "M\u{2013}O",
        'P'..='R' => "P\u{2013}R",
        'S'..='U' => "S\u{2013}U",
        _ => "V\u{2013}Z",
    }
    .to_string()
}

/// Library size above which the Power View library list shows the
/// letter-range pill row (see `LetterFilter`), scoping the server fetch to
/// one range at a time. Unrelated to the 50-item in-list header threshold
/// used by `use_letter_groups` in `list.rs`.
pub(crate) const LIBRARY_PILL_THRESHOLD: usize = 300;

/// The letter-range pill buckets, in display order. Single source of truth
/// for both the pill labels and the Emby `NameStartsWithOrGreater` /
/// `NameLessThan` fetch bounds, so they can't drift apart. Mirrors the range
/// boundaries used by `letter_bucket` above.
///
/// KNOWN LIMITATION (see `letter_bucket`'s doc comment): the `"#"` pill's
/// bounds (`NameLessThan("A")`) only reach titles that sort *before* "A".
/// An accented title whose SortName starts with a codepoint after 'Z'
/// (e.g. "Æon Flux") is fetched by the `V–Z` pill but rendered under a
/// `"#"` in-list header, and so is unreachable from the `"#"` pill itself.
const LETTER_FILTER_BUCKETS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("A\u{2013}C", Some("A"), Some("D")),
    ("D\u{2013}F", Some("D"), Some("G")),
    ("G\u{2013}I", Some("G"), Some("J")),
    ("J\u{2013}L", Some("J"), Some("M")),
    ("M\u{2013}O", Some("M"), Some("P")),
    ("P\u{2013}R", Some("P"), Some("S")),
    ("S\u{2013}U", Some("S"), Some("V")),
    ("V\u{2013}Z", Some("V"), None),
    ("#", None, Some("A")),
];

/// A selected letter-range pill: which bucket, its display label, and the
/// Emby name-range bounds to fetch. Constructed only via `for_index`/`default`
/// so it always matches a row in `LETTER_FILTER_BUCKETS`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LetterFilter {
    pub index: usize,
    pub label: &'static str,
    pub name_ge: Option<&'static str>,
    pub name_lt: Option<&'static str>,
}

impl LetterFilter {
    /// Number of pill buckets (`A–C` … `V–Z`, `#`).
    pub(crate) fn count() -> usize {
        LETTER_FILTER_BUCKETS.len()
    }

    /// Builds the `LetterFilter` for bucket `index`, or `None` if out of range.
    pub(crate) fn for_index(index: usize) -> Option<Self> {
        LETTER_FILTER_BUCKETS
            .get(index)
            .map(|&(label, name_ge, name_lt)| LetterFilter {
                index,
                label,
                name_ge,
                name_lt,
            })
    }

    /// The default pill selected when a large library is first opened: the
    /// first range, `A–C`.
    pub(crate) fn default_filter() -> Self {
        Self::for_index(0).expect("LETTER_FILTER_BUCKETS is non-empty")
    }

    /// All pill labels in bucket order, for building a `PillBar`.
    pub(crate) fn labels() -> Vec<String> {
        LETTER_FILTER_BUCKETS
            .iter()
            .map(|&(label, _, _)| label.to_string())
            .collect()
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

    fn render_power_library(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        // If a music-group library's nav_stack was truncated to just the group
        // level (e.g., stale breadcrumb click), immediately re-push the album level.
        if self.library_tab > 0 {
            self.ensure_music_group_album_level(self.library_tab - 1);
            self.ensure_feed_home_video_group_level(self.library_tab - 1);
        }

        if self.library_tab == 0 {
            self.render_power_home_list(f, area, focused, layout);
            return;
        }
        let lib_idx = self.library_tab.saturating_sub(1);
        let is_feed_group = self.library_tab > 0 && self.is_feed_home_video_group_view(lib_idx);
        let is_music_group = self.library_tab > 0 && self.is_music_group_view(lib_idx);
        let is_album_folders = self.library_tab > 0 && self.is_viewing_album_folders(lib_idx);
        let is_home_video = self.library_tab > 0 && self.is_home_video_view(lib_idx);
        if is_feed_group {
            self.render_power_feed_home_video_group_view(f, area, lib_idx, focused, layout);
        } else if is_album_folders && is_music_group {
            self.render_power_music_group_view(f, area, lib_idx, focused, layout);
        } else if is_album_folders {
            self.render_power_list(f, area, focused, layout);
        } else if is_home_video {
            self.render_power_home_video_list(f, area, lib_idx, focused, layout);
        } else {
            self.render_power_list(f, area, focused, layout);
        }
    }

    /// Returns the currently cursor-selected item at the album-folder-listing
    /// nav_stack level (i.e. the level where `is_viewing_album_folders`
    /// holds), if any. The cursor field always indexes into the raw
    /// `items` array in the order it was fetched (SortName-by-album-title)
    /// -- *not* the artist-grouped display order that
    /// `render_power_music_group_view` builds for rendering -- so a plain
    /// `items.get(cursor)` is correct even for the grouped music view.
    pub(in crate::app) fn selected_album_item(
        &self,
        lib_idx: usize,
    ) -> Option<mbv_core::api::MediaItem> {
        let lvl = self.libs[lib_idx].nav_stack.last()?;
        lvl.items.get(lvl.cursor).cloned()
    }

    /// Resolves the display artist for an album item in the grouped power
    /// music views. Priority order:
    /// 1. `item.artist` (Emby's Album-entity metadata) if non-empty.
    /// 2. `album_artist_cache` entry if non-empty (fetched from the album's
    ///    first few tracks — see `fetch_album_artist` in `images.rs`).
    /// 3. `parse_album_folder_name` heuristic as an interim guess — and if
    ///    the cache has neither a value nor an empty-tombstone yet, and no
    ///    fetch is already in flight, triggers `fetch_album_artist`.
    /// 4. Literal "Unknown Artist".
    pub(super) fn resolve_group_album_artist(&mut self, item: &mbv_core::api::MediaItem) -> String {
        if !item.artist.is_empty() {
            return item.artist.clone();
        }
        if let Some(cached) = self.album_artist_cache.get(&item.id) {
            if !cached.is_empty() {
                return cached.clone();
            }
        } else if !self.album_artist_loading.contains(&item.id) {
            self.fetch_album_artist(item.id.clone());
        }
        if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
            return artist;
        }
        "Unknown Artist".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState};
    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    use mbv_core::api::MediaItem;
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

    // The following `remote_status_spans` tests moved here from
    // `app::tests` (issue #361, commit 1): they used to render the full app
    // and scrape the bottom row, which only worked because the deleted
    // Standard view's status bar passed `show_session_pill: true`. The
    // status bar (`render/mod.rs`) has always passed `show_session_pill:
    // false` -- unchanged by this diff -- because it shows the same
    // remote/session info via the queue column's Local/Remote title pills
    // instead (`render_power_queue_title` in `render/queue.rs`, which calls
    // this same shared helper). Testing `remote_status_spans` directly, as
    // `remote_status_spans_prefers_..._` above already does, covers the
    // underlying logic without depending on which caller happens to display it.

    #[test]
    fn remote_status_spans_uses_daemon_endpoint_host_without_folding_in_server_url() {
        let app = make_app_stub();
        let spans =
            app.remote_status_spans(RemoteSlotState::DirectRemote, "tcp://music.local:8097");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("music.local"),
            "expected the remote glyph label to be the daemon endpoint host:\n{text}"
        );
        assert!(
            !text.contains("music.local@emby.local"),
            "the Emby server host must not be folded into the daemon-endpoint remote label:\n{text}"
        );
    }

    #[test]
    fn remote_status_spans_uses_attached_session_device_name_not_loopback_host() {
        let mut app = make_app_stub();
        app.connected_session_id = Some("sess-1".into());
        app.connected_session_state = Some(crate::app::tests::make_session("music", "Emby"));
        let spans = app.remote_status_spans(RemoteSlotState::AttachedSession, "");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("music"),
            "expected attached session status to use the F3-visible device name:\n{text}"
        );
        assert!(
            !text.contains("local"),
            "attached remote session should not render as local:\n{text}"
        );
    }

    #[test]
    fn remote_status_spans_keeps_direct_upgrade_session_name_after_state_is_cleared() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
        let sess = crate::app::tests::make_session("music", "mbv");

        app.switch_to_direct_remote(&sess, remote, remote_rx);
        assert!(app.connected_session_id.is_none());
        assert!(app.connected_session_state.is_none());

        let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("music"),
            "direct-upgraded remote should keep the F3-visible session name:\n{text}"
        );
        assert!(
            !text.contains("local"),
            "direct-upgraded remote should not fall back to local after clearing session state:\n{text}"
        );
    }

    #[test]
    fn remote_status_spans_shows_local_device_name_when_off() {
        let app = make_app_stub();
        let spans = app.remote_status_spans(RemoteSlotState::Off, "");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains(&mbv_core::api::device_name()),
            "expected the local device name when no remote is connected:\n{text}"
        );
        assert!(!text.contains("remote:"));
    }

    #[test]
    fn remote_status_spans_colors_icon_white_and_label_black_when_off_or_aqua_when_remote() {
        let app = make_app_stub();
        let spans = app.remote_status_spans(RemoteSlotState::Off, "");
        assert_eq!(spans[1].style.fg, Some(ratatui::style::Color::White));
        assert_eq!(spans[2].style.fg, Some(ratatui::style::Color::Black));

        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "");
        assert_eq!(spans[1].style.fg, Some(ratatui::style::Color::White));
        assert_eq!(spans[2].style.fg, Some(palette::AQUA));
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

    #[test]
    fn power_view_uses_triangle_resampling() {
        assert_eq!(POWER_RENDER_FILTER, ratatui_image::FilterType::Triangle);
    }

    fn render_power_scrollbar_column(height: u16, max_offset: usize, offset: usize) -> String {
        let backend = TestBackend::new(1, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_power_scrollbar(f, Rect::new(0, 0, 1, height), max_offset, offset);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    fn render_power_scrollbar_column_with_viewport(
        height: u16,
        content_length: usize,
        viewport_content_length: usize,
        offset: usize,
    ) -> String {
        let backend = TestBackend::new(1, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_power_scrollbar_with_viewport(
                f,
                Rect::new(0, 0, 1, height),
                content_length,
                viewport_content_length,
                offset,
            );
        })
        .unwrap();
        buffer_to_string(&term)
    }

    #[test]
    fn power_scrollbar_is_proportional_and_reaches_both_ends() {
        let top = render_power_scrollbar_column(7, 3, 0);
        let bottom = render_power_scrollbar_column(7, 3, 3);

        assert!(top.lines().next().is_some_and(|line| line != " "));
        assert!(bottom.lines().last().is_some_and(|line| line != " "));
        assert!(top.chars().filter(|&c| c == '▕').count() > 2);
        assert!(top.chars().all(|c| c == '▕' || c == ' ' || c == '\n'));
    }

    #[test]
    fn power_scrollbar_respects_custom_viewport_units() {
        let top = render_power_scrollbar_column_with_viewport(7, 10, 2, 0);
        let bottom = render_power_scrollbar_column_with_viewport(7, 10, 2, 8);

        assert!((1..=2).contains(&top.matches('▕').count()));
        assert!((1..=2).contains(&bottom.matches('▕').count()));
        assert!(top.chars().all(|c| c == '▕' || c == ' ' || c == '\n'));
        assert!(bottom.chars().all(|c| c == '▕' || c == ' ' || c == '\n'));
    }

    #[test]
    fn queue_scrollbar_uses_queue_unfocused_color() {
        let backend = TestBackend::new(1, 7);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_power_scrollbar(f, Rect::new(0, 0, 1, 7), 3, 0);
        })
        .unwrap();

        assert_eq!(term.backend().buffer()[(0, 0)].fg, palette::SCROLLBAR);
    }

    /// Renders a pill bar of the given labels/ids into a `width`-wide row and
    /// returns the resulting `(rect, id)` hitboxes.
    fn render_pill_bar_hitboxes(
        labels: &[String],
        ids: &[usize],
        selected_pos: usize,
        width: u16,
    ) -> Vec<(Rect, usize)> {
        let backend = TestBackend::new(width, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut tabs = Vec::new();
        term.draw(|f| {
            tabs = render_pill_bar(
                f,
                Rect::new(0, 0, width, 1),
                PillBar {
                    labels,
                    ids,
                    selected_pos,
                    prefix: None,
                    underlay: PillUnderlay::Blank { fill: true },
                },
            );
        })
        .unwrap();
        tabs
    }

    #[test]
    fn pill_bar_hitboxes_carry_caller_ids_not_display_positions() {
        // ids are deliberately offset from positions (mirroring Home's
        // section_idx = position + 10 here) so a regression that returned the
        // display offset instead of the id would be caught.
        let labels: Vec<String> = ["Alpha", "Beta", "Gamma"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ids = vec![10usize, 11, 12];

        // Wide enough to show every pill: all ids returned, in order.
        let tabs = render_pill_bar_hitboxes(&labels, &ids, 0, 60);
        assert_eq!(
            tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
            vec![10, 11, 12],
        );
        // Hitboxes are left-to-right and non-overlapping.
        for pair in tabs.windows(2) {
            assert!(pair[0].0.x + pair[0].0.width <= pair[1].0.x);
        }
    }

    #[test]
    fn pill_bar_scrolls_to_keep_selected_visible_and_maps_its_id() {
        // Six pills in a narrow row force horizontal scrolling; selecting the
        // last one must scroll it into view and report its caller id (25).
        let labels: Vec<String> = (0..6).map(|i| format!("Group{i}")).collect();
        let ids: Vec<usize> = (0..6).map(|i| 20 + i).collect();

        let tabs = render_pill_bar_hitboxes(&labels, &ids, 5, 18);

        assert!(!tabs.is_empty(), "expected at least one visible pill");
        // The selected pill (id 25) must be among the visible hitboxes.
        assert!(
            tabs.iter().any(|(_, id)| *id == 25),
            "selected pill's id should be visible after scrolling, got {:?}",
            tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        );
        // Every visible id is one we supplied (never a bare display offset).
        assert!(tabs.iter().all(|(_, id)| (20..=25).contains(id)));
        // Overflow occurred, so scrolling dropped at least one leading pill.
        assert!(
            tabs.len() < labels.len(),
            "narrow row should not fit all six pills"
        );
    }

    fn render_power_library_to_terminal(
        app: &mut App,
        layout: &mut LayoutMain,
    ) -> Terminal<TestBackend> {
        render_power_library_to_terminal_focused(app, layout, true)
    }

    fn render_power_library_to_terminal_focused(
        app: &mut App,
        layout: &mut LayoutMain,
        focused: bool,
    ) -> Terminal<TestBackend> {
        // 20 rows is comfortably enough for the " N items" header row (that
        // `render_power_list` draws unconditionally for a focused library
        // panel) plus the selected row and the compact banner's
        // content-dependent height (#263) for the short test overviews used
        // by callers of this helper.
        let backend = TestBackend::new(60, 20);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_library(f, Rect::new(0, 0, 60, 20), focused, layout);
        })
        .unwrap();
        term
    }

    fn render_power_library_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
        let term = render_power_library_to_terminal(app, layout);
        buffer_to_string(&term)
    }

    fn render_power_view_to_terminal(
        app: &mut App,
        width: u16,
        height: u16,
    ) -> (Terminal<TestBackend>, LayoutMain) {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_main(
                f,
                Rect::new(0, 0, width, height),
                &mut layout,
                &mut LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        (term, layout)
    }

    fn render_power_view(app: &mut App, width: u16, height: u16) -> LayoutMain {
        render_power_view_to_terminal(app, width, height).1
    }

    #[test]
    fn expanded_power_view_tab_panel_has_two_column_side_gutters() {
        let mut app = make_app_stub();
        app.queue_column_width = 40;

        let layout = render_power_view(&mut app, 80, 24);

        assert_eq!(layout.left_area.x, 40 + POWER_TAB_LEFT_PAD);
        assert_eq!(layout.left_area.width, 40 - 2 * POWER_TAB_LEFT_PAD);
    }

    #[test]
    fn expanded_power_panel_bounds_follow_sidebar_resize() {
        let mut app = make_app_stub();
        app.queue_column_width = 31;
        let first = render_power_view(&mut app, 80, 24);
        assert_eq!(first.panel_area, Rect::new(0, 0, 31, 24));
        assert_eq!(first.panel_content_area, Rect::new(2, 3, 27, 19));

        app.queue_column_width = 47;
        let second = render_power_view(&mut app, 80, 24);
        assert_eq!(second.panel_area, Rect::new(0, 0, 47, 24));
        assert_eq!(second.panel_content_area, Rect::new(2, 3, 43, 19));
    }

    #[test]
    fn power_panel_shell_paints_opaque_sidebar_and_active_local_header() {
        let backend = TestBackend::new(20, 8);
        let mut term = Terminal::new(backend).unwrap();
        let sidebar = Rect::new(3, 1, 10, 6);
        term.draw(|f| {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::IRIS)),
                sidebar,
            );
            App::render_panel_shell_at(f, sidebar, "HELP", "Esc Close", true);
        })
        .unwrap();

        let buffer = term.backend().buffer();
        for y in sidebar.y..sidebar.bottom() {
            for x in sidebar.x..sidebar.right() {
                if y >= sidebar.bottom() - 2
                    || (y == sidebar.y + 1 && (sidebar.x + 2..sidebar.right()).contains(&x))
                {
                    continue;
                }
                assert_eq!(buffer[(x, y)].bg, palette::PLAYBACK_PANEL_BG);
            }
        }
        assert_eq!(
            buffer[(sidebar.x + 2, sidebar.y + 1)].bg,
            palette::QUEUE_BUTTON_FOCUSED_BG
        );
        assert_eq!(buffer[(sidebar.x + 2, sidebar.y + 1)].fg, palette::TEXT);
        assert!(buffer[(sidebar.x + 2, sidebar.y + 1)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD));
        assert_eq!(
            buffer[(sidebar.x + sidebar.width - 3, sidebar.y + 1)].bg,
            palette::QUEUE_BUTTON_FOCUSED_BG
        );
        for x in sidebar.x..sidebar.right() {
            assert_eq!(buffer[(x, sidebar.y + 2)].bg, palette::PLAYBACK_PANEL_BG);
            assert_eq!(buffer[(x, sidebar.y + 2)].symbol(), " ");
        }
    }

    #[test]
    fn settings_content_has_two_column_and_one_row_insets() {
        let mut app = make_app_stub();
        let backend = TestBackend::new(20, 10);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            app.render_settings_panel(f, &mut layout, None);
        })
        .unwrap();

        assert_eq!(layout.settings_content_area, Rect::new(2, 4, 15, 3));
        let buffer = term.backend().buffer();
        for x in 0..19 {
            assert_eq!(buffer[(x, 3)].symbol(), " ");
        }
        for x in [0, 1, 18, 19] {
            assert_eq!(buffer[(x, 4)].bg, palette::PANEL_BG);
        }
        for x in 2..17 {
            assert_eq!(buffer[(x, 7)].bg, palette::PANEL_BG);
        }
    }

    #[test]
    fn expanded_power_right_scrollbar_uses_first_right_gutter_column() {
        let backend = TestBackend::new(80, 5);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_power_right_scrollbar(f, Rect::new(2, 0, 76, 5), 3, 0);
        })
        .unwrap();

        let buffer = term.backend().buffer();
        assert_eq!(buffer[(77, 0)].symbol(), " ");
        assert_eq!(buffer[(78, 0)].symbol(), "▕");
        assert_eq!(buffer[(79, 0)].symbol(), " ");
    }

    #[test]
    fn collapsed_power_right_panel_keeps_one_column_after_scrollbar() {
        let right_panel = Rect::new(0, 0, 80, 24);
        let content = power_right_panel_content_area(right_panel, true);
        assert_eq!(content.x + content.width, right_panel.right() - 1);
    }

    fn make_power_movie_app() -> App {
        let mut app = make_app_stub();
        app.library_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut focused = make_item("Focused Movie", "Movie");
        focused.id = "movie-focused".into();
        focused.overview = "This overview should appear in the compact movie banner while the list remains visible underneath.".into();
        focused.director = "Director Hidden".into();
        focused.production_year = 1988;
        focused.genre = "Action".into();

        let mut second = make_item("Second Movie", "Movie");
        second.id = "movie-second".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![focused, second],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    fn make_power_queue_app(item_count: usize) -> App {
        let mut app = make_power_movie_app();
        app.panel_focus = PanelFocus::Queue;
        app.player_tab.set_items(
            (0..item_count)
                .map(|i| make_item(&format!("Queue Item {i}"), "Movie"))
                .collect(),
            0,
        );
        app
    }

    fn make_power_remote_queue_app() -> App {
        let local_items = vec![make_item("Local Queue Item", "Movie")];
        let remote_items = vec![make_item("Remote Queue Item", "Movie")];
        let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
        app.library_tab = 1;
        app.panel_focus = PanelFocus::Queue;
        app.queue_scope = QueueScope::Remote;
        app.player_tab.set_items(local_items, 0);
        app
    }

    #[test]
    fn movie_library_unfocused_selected_banner_keeps_text_right_of_indicator() {
        let mut app = make_power_movie_app();
        let mut layout = LayoutMain::default();

        let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.lines().collect();

        // The colored-block look removes the green `▌` indicator entirely
        // (both focused and unfocused); the selected title sits inside the
        // MEDIA_SELECTED_BG block with a 2-col leading pad instead.
        let selected_line = lines
            .iter()
            .find(|line| line.contains("Focused Movie"))
            .expect("expected selected movie row");
        assert_eq!(
            selected_line.find('▌'),
            None,
            "expected no green selected-row indicator inside the colored block while unfocused:\n{out}"
        );

        let overview_line = lines
            .iter()
            .find(|line| line.contains("compact movie banner"))
            .expect("expected compact overview line");
        assert_eq!(
            overview_line.find('▌'),
            None,
            "expected no green banner bar inside the colored block while unfocused:\n{out}"
        );
    }

    #[test]
    fn power_view_uses_configured_left_column_width() {
        let mut app = make_power_movie_app();
        app.queue_column_width = 55;

        let layout = render_power_view(&mut app, 100, 28);

        assert_eq!(layout.queue_area.width, 51);
    }

    #[test]
    fn collapsed_power_left_column_gives_library_full_width() {
        let mut app = make_power_movie_app();
        app.queue_column_width = 55;
        app.queue_column_collapsed = true;

        let layout = render_power_view(&mut app, 100, 28);

        assert_eq!(layout.queue_area, Rect::default());
        assert_eq!(layout.left_area.x, 0);
        assert_eq!(layout.left_area.width, 99);
    }

    #[test]
    fn short_power_view_keeps_queue_in_left_column() {
        let mut app = make_power_movie_app();
        app.queue_column_width = 40;

        let layout = render_power_view(&mut app, 100, 12);

        assert!(
            layout.queue_area.x < app.queue_column_width,
            "expected short-height queue to stay in the left column, got {:?}",
            layout.queue_area
        );
        assert!(
            layout.left_area.x >= app.queue_column_width,
            "expected library area to remain in the right column, got {:?}",
            layout.left_area
        );
    }

    #[test]
    fn power_queue_panel_uses_selected_media_frame_and_background() {
        let mut app = make_power_queue_app(2);

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let buf = term.backend().buffer();
        let top_y = layout.queue_area.y - 2;
        let bottom_y = layout.queue_area.y + layout.queue_area.height + 1;
        let x = layout.queue_area.x;

        assert_eq!(buf[(x, top_y)].symbol(), "\u{2594}");
        assert_eq!(buf[(x, top_y)].fg, palette::SEEK_TRACK);
        assert_eq!(buf[(x, bottom_y)].symbol(), "\u{2581}");
        assert_eq!(buf[(x, bottom_y)].fg, palette::SEEK_TRACK);
        assert_eq!(buf[(x, layout.queue_area.y)].bg, palette::MEDIA_SELECTED_BG);
        assert_eq!(
            buf[(x, layout.queue_area.y - 1)].bg,
            palette::MEDIA_SELECTED_BG
        );
        assert_eq!(
            buf[(x, layout.queue_area.y + layout.queue_area.height)].bg,
            palette::MEDIA_SELECTED_BG
        );
    }

    #[test]
    fn power_queue_panel_fills_remaining_left_column_with_short_queue() {
        let mut app = make_power_queue_app(1);

        let (_term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let bottom_y = layout.queue_area.y + layout.queue_area.height + 1;

        assert_eq!(bottom_y, 26);
        assert!(
            layout.queue_area.height > 1,
            "expected queue viewport inside full-height panel, got {:?}",
            layout.queue_area
        );
    }

    #[test]
    fn power_queue_panel_empty_state_is_inside_panel() {
        let mut app = make_power_queue_app(0);

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let out = buffer_to_string(&term);
        let empty_y = out
            .lines()
            .position(|line| line.contains("Add items with p"))
            .expect("expected queue empty-state message");

        assert_eq!(empty_y as u16, layout.queue_area.y);
        assert_eq!(
            term.backend().buffer()[(layout.queue_area.x, empty_y as u16)].bg,
            palette::MEDIA_SELECTED_BG
        );
    }

    #[test]
    fn power_queue_panel_remains_visible_when_unfocused() {
        let mut app = make_power_queue_app(1);
        app.panel_focus = PanelFocus::Library;

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let buf = term.backend().buffer();
        let top_y = layout.queue_area.y - 2;
        let bottom_y = layout.queue_area.y + layout.queue_area.height + 1;

        assert_eq!(buf[(layout.queue_area.x, top_y)].symbol(), "\u{2594}");
        assert_eq!(buf[(layout.queue_area.x, bottom_y)].symbol(), "\u{2581}");
        assert_eq!(
            buf[(layout.queue_area.x, layout.queue_area.y)].bg,
            palette::LIBRARY_SIDE_BG,
            "unfocused queue panel should use the dimmed background, not the focused MEDIA_SELECTED_BG"
        );
    }

    #[test]
    fn power_queue_title_and_scope_pills_stay_outside_panel() {
        let mut app = make_power_remote_queue_app();
        app.use_nerd_fonts = false;

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let top_y = layout.queue_area.y - 2;
        let out = buffer_to_string(&term);
        let header = out
            .lines()
            .nth(layout.queue_scope_local_area.y as usize)
            .expect("expected queue header row");
        let device_name = mbv_core::api::device_name();
        let upper_device_name = device_name.to_uppercase();

        assert!(layout.queue_scope_local_area.y < top_y);
        assert!(layout.queue_scope_remote_area.y < top_y);
        assert!(layout.queue_scope_remote_area.x > layout.queue_scope_local_area.x);
        assert_eq!(
            layout.queue_scope_local_area.width + layout.queue_scope_remote_area.width,
            layout.queue_area.width
        );
        assert!(
            header.matches(&upper_device_name).count() >= 2,
            "expected local and remote queue controls to use session-style hostname pills:\n{out}"
        );
        assert!(
            !header.contains('\u{F0AFE}'),
            "expected non-Nerd-Font queue header to avoid private-use glyphs:\n{out}"
        );
    }

    #[test]
    fn power_queue_title_does_not_render_playlist_pill() {
        let mut app = make_power_remote_queue_app();
        app.queue_source = crate::config::QueueSource::Playlist {
            id: None,
            name: "Road Mix".into(),
        };

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let out = buffer_to_string(&term);
        let header = out
            .lines()
            .nth(layout.queue_scope_local_area.y as usize)
            .expect("expected queue header row");
        let device_name = mbv_core::api::device_name();
        let upper_device_name = device_name.to_uppercase();

        assert!(
            header.contains(&upper_device_name),
            "expected session hostname pill in queue header:\n{out}"
        );
        assert!(
            !header.contains("Road Mix") && !header.contains("none"),
            "expected playlist pill to stay out of queue header:\n{out}"
        );
    }

    #[test]
    fn power_view_bottom_status_bar_shows_playlist_pill_when_queue_is_a_playlist() {
        let mut app = make_power_queue_app(2);
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".into()),
            name: "Road Mix".into(),
        };

        let (term, _layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let out = buffer_to_string(&term);
        let last_line = out.lines().last().unwrap_or_default();

        assert!(
            last_line.contains("Road Mix"),
            "expected the playlist pill to appear in the Power View status bar:\n{last_line}"
        );
        let text_x = last_line
            .find("Road Mix")
            .expect("expected playlist name position") as u16;
        assert_eq!(
            term.backend().buffer()[(text_x, 27)].fg,
            palette::YELLOW,
            "expected playlist pill text to be yellow, not green:\n{last_line}"
        );
    }

    #[test]
    fn short_power_queue_panel_drops_padding_before_rows() {
        let mut app = make_power_queue_app(20);

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 12);
        let buf = term.backend().buffer();
        let top_y = layout.queue_area.y - 1;
        let bottom_y = layout.queue_area.y + layout.queue_area.height;

        assert_eq!(buf[(layout.queue_area.x, top_y)].symbol(), "\u{2594}");
        assert_eq!(buf[(layout.queue_area.x, bottom_y)].symbol(), "\u{2581}");
        assert!(
            layout.queue_area.height >= 1,
            "expected at least one usable queue row on a short terminal, got {:?}",
            layout.queue_area
        );
    }

    #[test]
    fn power_queue_panel_counts_wrapped_group_headers_before_adding_padding() {
        let mut app = make_power_movie_app();
        app.panel_focus = PanelFocus::Queue;
        let mut item = make_item("Track", "Audio");
        item.id = "boundary-track".into();
        item.album_id = "boundary-album".into();
        item.album = "Long Album Title".into();
        item.artist = "Very Long Artist".into();
        app.player_tab.set_items(vec![item], 0);

        let panel_area = Rect::new(0, 0, 20, 6);
        let desired_rows =
            rendered_power_queue_rows_for_padding(&app.displayed_queue().items, panel_area);
        assert_eq!(desired_rows, 4);

        let backend = TestBackend::new(panel_area.width, panel_area.height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            let queue_area = render_power_queue_panel_frame(f, panel_area, desired_rows, true);
            app.render_power_queue(f, queue_area, true, &mut layout);
        })
        .unwrap();
        let out = buffer_to_string(&term);

        assert_eq!(layout.queue_area.y, 1);
        assert_eq!(layout.queue_area.height, 4);
        assert!(
            layout.queue_row_map.contains(&Some(0)),
            "expected selected track row to be mapped as visible after wrapped header: {:?}",
            layout.queue_row_map
        );
        assert!(
            out.contains("1. Track"),
            "expected selected track to remain visible below the wrapped group header:\n{out}"
        );
    }

    #[test]
    fn power_queue_panel_preserves_group_aware_scrolling() {
        let mut app = make_power_movie_app();
        app.panel_focus = PanelFocus::Queue;

        let mut items = Vec::new();
        for i in 0..4 {
            let mut item = make_item(&format!("A{i}"), "Audio");
            item.id = format!("a-{i}");
            item.album_id = "album-a".into();
            item.album = "Album A".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        for i in 0..4 {
            let mut item = make_item(&format!("B{i}"), "Audio");
            item.id = format!("b-{i}");
            item.album_id = "album-b".into();
            item.album = "Album B".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        app.player_tab.set_items(items, 4);
        app.queue_scroll = 9;

        let (_term, _layout) = render_power_view_to_terminal(&mut app, 100, 20);

        assert_eq!(app.queue_scroll, 9);
    }

    fn make_power_music_group_app() -> App {
        let mut app = make_app_stub();
        app.library_tab = 1;
        app.music_levels = vec!["group".into(), "album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        // Six groups is enough to force horizontal scrolling in a narrow test terminal.
        let group_names = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"];
        let groups: Vec<MediaItem> = group_names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut it = make_item(n, "MusicArtist");
                it.id = format!("group-{i}");
                it.is_folder = true;
                it
            })
            .collect();

        let mut album = make_item("First Album", "MusicAlbum");
        album.id = "album-1".into();
        album.artist = "Alpha".into();
        album.production_year = 2001;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-music".into(),
                    title: "Music".into(),
                    items: groups,
                    total_count: group_names.len(),
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                },
                BrowseLevel {
                    parent_id: "group-0".into(),
                    title: "Alpha".into(),
                    items: vec![album],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                },
            ],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    #[test]
    fn selectable_artist_headers_are_typed_row_targets() {
        let mut app = make_power_music_group_app();
        // Headers for groups with only one child are not selectable, so give
        // Alpha a second album to keep it eligible as a typed row target.
        let mut alpha_album2 = make_item("Second Alpha Album", "MusicAlbum");
        alpha_album2.id = "album-1b".into();
        alpha_album2.artist = "Alpha".into();
        alpha_album2.is_folder = true;
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(alpha_album2);
        let mut beta_album = make_item("Beta Album", "MusicAlbum");
        beta_album.id = "album-2".into();
        beta_album.artist = "Beta".into();
        beta_album.is_folder = true;
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(beta_album);

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);

        assert!(
            out.contains("Alpha") && out.contains("Beta"),
            "expected both artist headers to render:\n{out}"
        );
        assert!(
            matches!(
                layout.left_row_targets.first().and_then(Option::as_ref),
                Some(LibraryRowTarget::ArtistHeader(selection))
                    if selection.artist_label == "Alpha"
                        && selection.first_album_id == "album-1"
            ),
            "expected the first custom artist header to be a typed row target"
        );
        assert_eq!(
            layout.left_row_map.first(),
            Some(&None),
            "legacy row map must keep headers non-album rows"
        );
    }

    #[test]
    fn grouped_album_rows_use_styled_suffix_and_single_group_spacers() {
        let mut app = make_power_music_group_app();
        let mut alpha_album = make_item("Second Alpha Album", "MusicAlbum");
        alpha_album.id = "album-1b".into();
        alpha_album.artist = "Alpha".into();
        alpha_album.production_year = 2002;
        let mut beta_album = make_item("Beta Album", "MusicAlbum");
        beta_album.id = "album-2".into();
        beta_album.artist = "Beta".into();
        beta_album.production_year = 2003;
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        level.items.extend([alpha_album, beta_album]);
        level.cursor = 2;

        let mut layout = LayoutMain::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.lines().collect();
        let alpha_y = lines
            .iter()
            .position(|line| line.contains("Alpha") && !line.contains("Album"))
            .expect("expected Alpha artist header");
        let beta_y = lines
            .iter()
            .position(|line| line.contains("Beta") && !line.contains("Album"))
            .expect("expected Beta artist header");
        let last_alpha_album_y = lines
            .iter()
            .rposition(|line| line.contains("Alpha Album"))
            .expect("expected the final Alpha album row");
        assert_eq!(
            beta_y,
            last_alpha_album_y + 2,
            "expected exactly one spacer between artist groups:\n{out}"
        );
        let last_selectable = layout
            .left_row_targets
            .iter()
            .rev()
            .find_map(Option::as_ref)
            .expect("expected a selectable album row");
        assert!(
            matches!(last_selectable, LibraryRowTarget::Album(_)),
            "expected no trailing artist spacer after the final album"
        );

        let album_y = lines
            .iter()
            .position(|line| line.contains("Second Alpha Album"))
            .expect("expected Alpha album row");
        let title_x = lines[album_y].find("Second Alpha Album").unwrap() as u16;
        let header_x = lines[alpha_y].find("Alpha").unwrap() as u16;
        assert_eq!(
            title_x, header_x,
            "album title should align with its header"
        );
        let bullet_x = lines[album_y].find('•').unwrap() as u16;
        let year_x = lines[album_y].find("2002").unwrap() as u16;
        let buffer = term.backend().buffer();
        assert_eq!(buffer[(title_x, album_y as u16)].fg, palette::WHITE);
        assert_eq!(buffer[(bullet_x, album_y as u16)].fg, palette::YELLOW);
        assert_eq!(buffer[(year_x, album_y as u16)].fg, palette::AQUA);

        let selected_album_y = lines
            .iter()
            .position(|line| line.contains("Beta Album"))
            .expect("expected selected Beta album row");
        let selected_title_x = lines[selected_album_y].find("Beta Album").unwrap() as u16;
        assert_eq!(
            buffer[(selected_title_x, selected_album_y as u16)].fg,
            palette::WHITE,
            "selected album titles should remain white"
        );
    }

    #[test]
    fn selectable_artist_header_renders_focused() {
        let mut app = make_power_music_group_app();
        app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
            first_album_id: "album-1".into(),
            artist_label: "Alpha".into(),
        });

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();
        let header_row = lines
            .iter()
            .position(|line| line.contains("Alpha"))
            .expect("expected Alpha header");
        let header = lines[header_row];

        assert!(
            !header.contains('\u{258c}'),
            "selected artist header should no longer render the left focus gutter:\n{out}"
        );
        assert!(
            !header.contains('\u{f037b}'),
            "selected artist header should no longer render the trailing focus icon \
             (the selection block now carries the focus signal):\n{out}"
        );

        // The header should now be wrapped in the same colored-block frame
        // as a selected album: a `▁` border row (with a blank colored-bg
        // padding row directly beneath it) above the header, an action-hint
        // row directly below the header (no `ENTER` clause, unlike the
        // album hint), then a colored-bg padding row and a `▔` border row.
        assert!(
            header_row >= 2 && lines[header_row - 2].contains('\u{2581}'),
            "expected a top border row two rows above the selected header:\n{out}"
        );
        let hint_row = header_row + 1;
        assert!(
            lines[hint_row].contains("^P: Play | ^A: Enqueue | ^S: Shuffle"),
            "expected the artist action-hint row directly below the header:\n{out}"
        );
        assert!(
            !lines[hint_row].contains("ENTER"),
            "artist action hint should not include the album's ENTER clause:\n{out}"
        );
        assert!(
            lines[hint_row + 1..]
                .iter()
                .take(4)
                .any(|line| line.contains('\u{2594}')),
            "expected a bottom border row below the selected header block:\n{out}"
        );

        assert_eq!(
            layout.cursor_screen_y,
            Some(header_row as u16),
            "selected header should own the screen cursor row"
        );
    }

    #[test]
    fn music_group_pills_render_on_row_below_title_marker() {
        let mut app = make_power_music_group_app();
        app.queue_column_width = 20;
        let width = 100u16;
        let height = 20u16;
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_main(
                f,
                Rect::new(0, 0, width, height),
                &mut layout,
                &mut LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let row0 = out.lines().next().unwrap();
        let _row1 = out.lines().nth(1).unwrap();

        let row3 = out.lines().nth(3).unwrap();

        assert!(
            !row0.contains("Alpha") && !row0.contains("Beta"),
            "expected pills not on the first row:\n{out}"
        );
        assert!(
            row3.contains("Alpha") && row3.contains("Beta"),
            "expected group pills below the tab bar (no header row):\n{out}"
        );

        let _rchar_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.rfind(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };
        let char_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.find(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };

        let right_col_x = app.queue_column_width + POWER_VIEW_GAP;
        let buf = term.backend().buffer();
        assert!(
            row3.chars().take(right_col_x as usize).all(|c| c == ' '),
            "expected the pill row to be confined to the right library column:\n{out}"
        );

        let alpha_x = char_x(row3, "Alpha");
        assert!(
            alpha_x >= right_col_x,
            "expected pills confined to the right column"
        );
        assert_eq!(buf[(alpha_x, 3)].bg, palette::YELLOW);
        assert_eq!(
            buf[(alpha_x, 3)].fg,
            palette::PILL_DARK,
            "expected the selected group pill to use dark text"
        );
        let beta_x = char_x(row3, "Beta");
        assert_eq!(buf[(beta_x, 3)].bg, palette::LIBRARY_SIDE_BG);
        assert_eq!(
            buf[(beta_x, 3)].fg,
            palette::YELLOW,
            "expected a non-selected group pill to use yellow text"
        );

        let (gap_start, gap_end) = (alpha_x.min(beta_x), alpha_x.max(beta_x));
        let between: String = row3
            .chars()
            .skip(gap_start as usize)
            .take((gap_end - gap_start) as usize)
            .collect();
        assert!(
            !between.contains('\u{2501}'),
            "expected a blank gap between adjacent pills, not a dash rule:\n{between:?}"
        );

        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 3, "expected selector hitboxes on the pills row");
            assert!(
                rect.x >= right_col_x,
                "expected selector hitboxes confined to the right column"
            );
        }

        // Row 4 is a blank spacer between the pill row and the album list.
        let spacer_row = out.lines().nth(4).unwrap();
        assert!(
            spacer_row.trim().is_empty(),
            "expected a blank spacer row between the pills and the album list:\n{out}"
        );
        let album_row = out.lines().nth(5).unwrap();
        assert!(
            album_row.contains("Alpha") || album_row.contains("First Album"),
            "expected album list content to start below the pill/spacer rows:\n{out}"
        );
    }

    #[test]
    fn music_group_pills_scroll_within_reserved_space_when_overflowing() {
        let mut app = make_power_music_group_app();
        app.queue_column_width = 20;
        let width = 40u16;
        let height = 20u16;
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_main(
                f,
                Rect::new(0, 0, width, height),
                &mut layout,
                &mut LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let _row0 = out.lines().next().unwrap();

        let row3 = out.lines().nth(3).unwrap();
        let _row4 = out.lines().nth(4).unwrap();

        assert!(
            row3.contains('\u{203a}'),
            "expected a right scroll indicator on the pills row (no header gap):\n{out}"
        );

        let rchar_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.rfind(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };

        let right_indicator_x = rchar_x(row3, "\u{203a}");
        assert!(
            right_indicator_x < width,
            "expected the right scroll indicator to stay inside the pill row:\n{out}"
        );

        let right_col_x = (app.queue_column_width + POWER_VIEW_GAP) as usize;
        assert!(
            row3.chars().take(right_col_x).all(|c| c == ' '),
            "expected the pill row to be confined to the right library column:\n{out}"
        );

        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 3, "expected pill hitboxes on the pills row");
            assert!(
                rect.x as usize >= right_col_x,
                "expected pill hitboxes confined to the right column"
            );
            assert!(
                rect.x + rect.width <= width,
                "expected pill hitboxes confined to the visible pill row"
            );
        }
    }

    // ── inline album detail at the album-folder-listing level (#145, task 2) ──

    #[test]
    fn album_folder_listing_renders_list_and_inline_detail_together() {
        let mut app = make_power_music_group_app();
        // Sitting at the album-folder-listing level already (no drilldown push).
        assert_eq!(app.libs[0].nav_stack.len(), 2);

        let mut second_album = make_item("Second Album", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Alpha".into();
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(second_album);

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        // In the music-group (pill selector) view, inline tracks only render
        // once track-selection mode has been entered (Enter pressed).
        app.libs[0].album_track_focus = Some(0);

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        assert!(
            out.contains("Alpha"),
            "expected the album list (grouped by artist) to still render:\n{out}"
        );
        assert!(
            out.contains("Opening Track"),
            "expected the selected album's cached tracks to render inline, \
             without any drilldown:\n{out}"
        );

        // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
        // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
        // `─` rule + `▌` gutter.
        let title_y = lines
            .iter()
            .position(|l| l.contains("First Album"))
            .expect("expected selected album row");
        assert!(
            lines[title_y - 3].contains("\u{2581}"),
            "expected a top border three rows above the title (border, padding, then artist row):\n{out}"
        );
        assert!(
            lines[title_y - 2].trim().is_empty(),
            "expected the colored top-padding row directly above the artist row to be blank:\n{out}"
        );

        let track_y = lines
            .iter()
            .position(|l| l.contains("Opening Track"))
            .expect("expected inline track row");
        assert!(
            track_y > title_y,
            "expected the track row to render below the selected album title:\n{out}"
        );

        let second_album_y = lines
            .iter()
            .position(|l| l.contains("Second Album"))
            .expect("expected the following album row");
        assert!(
            lines[second_album_y - 1].contains("\u{2594}"),
            "expected a bottom border directly above the following album row:\n{out}"
        );
        assert!(
            second_album_y > track_y,
            "expected the following album to render after the inline track detail:\n{out}"
        );

        let title_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(0))
            .expect("expected the selected album (index 0) in the row map");
        let second_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected the following album (index 1) in the row map");
        assert!(
            second_row_idx > title_row_idx,
            "expected the following album's row-map entry after the selected album's"
        );
        assert!(
            layout.left_row_map[title_row_idx + 1..second_row_idx]
                .iter()
                .all(Option::is_none),
            "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
            layout.left_row_map
        );
        assert_eq!(
            app.libs[0].nav_stack.len(),
            2,
            "rendering the inline preview must not push a nav_stack level"
        );
    }

    #[test]
    fn flat_album_folder_listing_renders_inline_detail_under_selected_album() {
        let mut app = make_app_stub();
        app.library_tab = 1;
        app.music_levels = vec!["album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        let mut album = make_item("First Album", "MusicAlbum");
        album.id = "album-1".into();
        album.artist = "Alpha".into();
        album.is_folder = true;
        let mut second_album = make_item("Second Album", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Alpha".into();
        second_album.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: vec![album, second_album],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
        // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
        // `─` rule + `▌` gutter. Structure per block:
        //   [border ▁] [colored padding] [album title] [tracks...] [colored padding] [border ▔]
        let title_y = lines
            .iter()
            .position(|l| l.contains("First Album"))
            .expect("expected selected album title row");
        assert!(
            lines[title_y - 3].contains("\u{2581}"),
            "expected a top border three rows above the title (border, padding, then artist row):\n{out}"
        );
        assert!(
            lines[title_y - 2].trim().is_empty(),
            "expected the colored top-padding row directly above the artist row to be blank:\n{out}"
        );

        let track_y = lines
            .iter()
            .position(|l| l.contains("Opening Track"))
            .expect("expected inline track row");
        assert!(
            track_y > title_y,
            "expected the track row to render below the selected album title:\n{out}"
        );

        let second_album_y = lines
            .iter()
            .position(|l| l.contains("Second Album"))
            .expect("expected the following album row");
        assert!(
            lines[second_album_y - 1].contains("\u{2594}"),
            "expected a bottom border directly above the following album row:\n{out}"
        );
        assert!(
            second_album_y > track_y,
            "expected the following album to render after the inline track detail:\n{out}"
        );

        // Row-map: only the Album() rows (title + following album) map to a
        // selectable index; every border/padding/track-detail row is `None`.
        let title_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(0))
            .expect("expected the selected album (index 0) in the row map");
        let second_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected the following album (index 1) in the row map");
        assert!(
            second_row_idx > title_row_idx,
            "expected the following album's row-map entry after the selected album's"
        );
        assert!(
            layout.left_row_map[title_row_idx + 1..second_row_idx]
                .iter()
                .all(Option::is_none),
            "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
            layout.left_row_map
        );
        assert!(
            layout
                .left_row_targets
                .iter()
                .all(|target| !matches!(target, Some(LibraryRowTarget::ArtistHeader(_)))),
            "flat/non-custom grouped album headers must remain non-selectable"
        );
    }

    #[test]
    fn inline_album_track_selection_block_hides_its_own_scrollbar() {
        let mut app = make_app_stub();
        let mut tracks = Vec::new();
        for i in 0..20 {
            let mut track = make_item(&format!("Track {i}"), "Audio");
            track.id = format!("track-{i}");
            track.album = "Selected Album".into();
            track.index_number = i + 1;
            tracks.push(track);
        }

        let backend = TestBackend::new(30, 8);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_power_album_detail(
                f,
                Rect::new(0, 0, 30, 8),
                &tracks,
                12,
                true,
                true,
                true,
                false,
                0,
                &mut layout,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);

        assert!(
            !out.contains('\u{2590}'),
            "inline track-selection block must not draw its own scrollbar:\n{out}"
        );
    }

    #[test]
    fn album_folder_listing_fetches_and_shows_loading_on_cache_miss() {
        let mut app = make_power_music_group_app();
        let mut second_album = make_item("Second Album", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Alpha".into();
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(second_album);
        assert!(!app.album_tracks_cache.contains_key("album-1"));
        assert!(!app.album_tracks_loading.contains("album-1"));

        // In the music-group (pill selector) view, inline tracks (and the
        // fetch that populates them) only happen once track-selection mode
        // has been entered (Enter pressed).
        app.libs[0].album_track_focus = Some(0);

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        assert!(
            app.album_tracks_loading.contains("album-1"),
            "expected a cache miss to trigger fetch_album_tracks for the \
             selected album:\n{out}"
        );
        assert!(
            out.to_lowercase().contains("loading"),
            "expected a loading indicator in the detail pane while the \
             fetch is in flight:\n{out}"
        );
        // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
        // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
        // `─` rule + `▌` gutter.
        let title_y = lines
            .iter()
            .position(|l| l.contains("First Album"))
            .expect("expected selected album row");
        assert!(
            lines[title_y - 3].contains("\u{2581}"),
            "expected a top border three rows above the title (border, padding, then artist row):\n{out}"
        );
        assert!(
            lines[title_y - 2].trim().is_empty(),
            "expected the colored top-padding row directly above the artist row to be blank:\n{out}"
        );

        let loading_y = lines
            .iter()
            .position(|l| l.to_lowercase().contains("loading"))
            .expect("expected an inline loading row");
        assert!(
            loading_y > title_y,
            "expected the loading row to render below the selected album title:\n{out}"
        );

        let second_album_y = lines
            .iter()
            .position(|l| l.contains("Second Album"))
            .expect("expected the following album row");
        assert!(
            lines[second_album_y - 1].contains("\u{2594}"),
            "expected a bottom border directly above the following album row:\n{out}"
        );
        assert!(
            second_album_y > loading_y,
            "expected the following album to render after the inline loading row:\n{out}"
        );

        let title_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(0))
            .expect("expected the selected album (index 0) in the row map");
        let second_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected the following album (index 1) in the row map");
        assert!(
            second_row_idx > title_row_idx,
            "expected the following album's row-map entry after the selected album's"
        );
        assert!(
            layout.left_row_map[title_row_idx + 1..second_row_idx]
                .iter()
                .all(Option::is_none),
            "expected every row between the two albums (borders, padding, loading row) to be non-selectable:\n{:?}",
            layout.left_row_map
        );
    }

    #[test]
    fn album_folder_inline_detail_is_hidden_until_track_selection_mode() {
        let mut app = make_power_music_group_app();

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutMain::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.lines().collect();
        let buf = term.backend().buffer();

        assert_eq!(
            lines
                .iter()
                .filter(|line| line.contains("First Album"))
                .count(),
            1,
            "expected no duplicate inline album title row:\n{out}"
        );

        assert!(
            !out.contains("Opening Track"),
            "expected inline tracks to stay hidden until track-selection mode is entered \
             (Enter pressed):\n{out}"
        );

        let hint_y = lines
            .iter()
            .position(|line| line.contains("^P: Play"))
            .expect("expected inline action hint row");
        assert!(
            // The full hint text is wider than this fixture's terminal, so
            // it's truncated with an ellipsis -- just check for the
            // still-visible prefix.
            lines[hint_y].contains("ENTER: Show"),
            "expected the collapsed hint row to prompt Enter to show tracks:\n{out}"
        );
        let hint_x = lines[hint_y]
            .find("^P: Play")
            .expect("expected hint x position");
        let title_y = lines
            .iter()
            .position(|line| line.contains("First Album"))
            .expect("expected selected album title row");
        let title_x = lines[title_y]
            .find("First Album")
            .expect("expected selected album title position");
        assert!(
            hint_x == title_x,
            "expected collapsed hint content to align with the selected album title:\n{out}"
        );
        assert_eq!(
            buf[(hint_x as u16, hint_y as u16)].fg,
            palette::SOFT_WHITE,
            "expected inline action hints to render soft white:\n{out}"
        );
    }

    #[test]
    fn selected_music_group_album_shows_right_aligned_art_before_track_mode() {
        let mut app = make_power_music_group_app();
        app.image_protocol_enabled = true;

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_artist_cache
            .insert("album-1".into(), "Album Artist".into());
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutMain::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let art_rect = layout
            .inline_image_rect
            .expect("expected selected album art rect before track mode");

        assert!(
            !out.contains("Opening Track"),
            "tracks should stay hidden until track-selection mode:\n{out}"
        );
        let lines: Vec<&str> = out.lines().collect();
        let title_y = lines
            .iter()
            .position(|line| line.contains("First Album"))
            .expect("expected selected album title row");
        assert_eq!(
            lines[title_y - 1].trim(),
            "Album Artist",
            "album artist should appear immediately above the title:\n{out}"
        );
        let artist_x = lines[title_y - 1].find("Album Artist").unwrap() as u16;
        assert_eq!(
            term.backend().buffer()[(artist_x, (title_y - 1) as u16)].fg,
            palette::YELLOW
        );
        assert_eq!(
            art_rect.y, title_y as u16,
            "album artwork should start on the title row, below the artist row"
        );

        app.album_artist_cache.remove("album-1");
        let fallback_term = render_power_library_to_terminal(&mut app, &mut layout);
        let fallback = buffer_to_string(&fallback_term);
        let fallback_lines: Vec<&str> = fallback.lines().collect();
        let fallback_title_y = fallback_lines
            .iter()
            .position(|line| line.contains("First Album"))
            .expect("expected fallback album title row");
        assert_eq!(
            fallback_lines[fallback_title_y - 1].trim(),
            "Alpha",
            "item artist should be the fallback when album artist is absent:\n{fallback}"
        );
        assert_eq!(
            art_rect.x + art_rect.width,
            58,
            "album art should have two columns of right padding"
        );
        assert_eq!((art_rect.width, art_rect.height), (24, 12));
        assert!(app.card_image_loading.contains("album-1:P"));
        assert!(!app.card_image_loading.contains("track-1:P"));
        assert_eq!(
            term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
            palette::OVERLAY,
            "loading album art should reserve a right-aligned placeholder:\n{out}"
        );
    }

    #[test]
    fn selected_album_block_wraps_text_around_art_without_moving_art() {
        let mut app = make_power_music_group_app();
        app.image_protocol_enabled = true;
        app.libs[0].album_track_focus = Some(0);
        let album = &mut app.libs[0].nav_stack.last_mut().unwrap().items[0];
        album.name = "A Very Long Album Title That Wraps Before Artwork".into();
        album.artist = "Fallback Artist With A Very Long Name That Wraps Clearly".into();
        app.album_artist_cache.insert(
            "album-1".into(),
            "Preferred Album Artist With A Very Long Name That Wraps".into(),
        );
        let mut track = make_item(
            "A Very Long Track Name That Continues Below The Artwork Width",
            "Audio",
        );
        track.id = "track-1".into();
        track.album = album.name.clone();
        track.artist = album.artist.clone();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutMain::default();
        let backend = TestBackend::new(50, 35);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_library(f, Rect::new(0, 0, 50, 35), true, &mut layout);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.lines().collect();
        let art_rect = layout
            .inline_image_rect
            .expect("expected selected album artwork");
        let title_y = lines
            .iter()
            .position(|line| line.trim() == "A Very Long")
            .expect("expected wrapped album title");
        let artist_y = lines
            .iter()
            .position(|line| line.contains("Preferred Album Artist"))
            .expect("expected preferred album artist row");
        assert!(
            artist_y < title_y,
            "artist row should precede album title:\n{out}"
        );
        assert_eq!(art_rect.y, title_y as u16);
        let artist_x = lines[artist_y].find("Preferred Album Artist").unwrap() as u16;
        assert_eq!(
            term.backend().buffer()[(artist_x, artist_y as u16)].fg,
            palette::YELLOW
        );
        assert!(
            lines.iter().any(|line| line.contains("^P: Play"))
                && lines.iter().any(|line| line.contains("Shuffle")),
            "expected wrapped action hint rows:\n{out}"
        );
        assert!(
            lines.iter().any(|line| line.contains("That Continue"))
                && lines.iter().any(|line| line.trim() == "Artwork Width"),
            "expected wrapped inline track rows:\n{out}"
        );
        for line in &lines[title_y..] {
            if line.contains("Preferred Album Artist")
                || line.contains("A Very Long Album")
                || line.contains("^P: Play")
                || line.contains("Shuffle")
                || line.contains("A Very Long Track")
                || line.contains("Artwork Width")
            {
                let last_text_x = line
                    .chars()
                    .enumerate()
                    .filter(|(_, ch)| !ch.is_whitespace())
                    .map(|(x, _)| x as u16)
                    .max()
                    .unwrap();
                assert!(
                    last_text_x < art_rect.x,
                    "selected-block text must not draw beneath artwork:\n{out}"
                );
            }
        }
    }

    #[test]
    fn selected_music_group_album_keeps_right_aligned_art_in_track_mode() {
        let mut app = make_power_music_group_app();
        app.image_protocol_enabled = true;
        app.libs[0].album_track_focus = Some(0);

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.player_tab.set_items(vec![track.clone()], 0);
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 0;
            status.paused = false;
        }
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutMain::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let art_rect = layout
            .inline_image_rect
            .expect("expected selected album art rect in track mode");

        assert!(
            out.contains("Opening Track"),
            "expected inline track row:\n{out}"
        );
        let lines: Vec<&str> = out.lines().collect();
        let playing_line = lines
            .iter()
            .find(|line| line.contains("Opening Track"))
            .copied()
            .expect("expected active music track row");
        let icon = super::play_icon(app.use_nerd_fonts);
        assert!(
            playing_line.contains(&format!("1. {icon} Opening Track")),
            "expected the active track icon and following space after its number:\n{out}"
        );
        let hint_y = lines
            .iter()
            .position(|line| line.contains("^P: Play"))
            .expect("expected track-mode action hint row");
        let track_y = lines
            .iter()
            .position(|line| line.contains("Opening Track"))
            .expect("expected inline track row");
        assert!(
            lines[track_y - 1].trim().is_empty(),
            "expected a blank row between the wrapped track-mode hint and tracks:\n{out}"
        );
        assert_eq!(
            track_y.saturating_sub(hint_y),
            3,
            "expected the wrapped hint, blank separator, then track list:\n{out}"
        );
        let hint_x = lines[hint_y]
            .find("^P: Play")
            .expect("expected track-mode hint x position");
        let title_y = lines
            .iter()
            .position(|line| line.contains("First Album"))
            .expect("expected selected album title row");
        let title_x = lines[title_y]
            .find("First Album")
            .expect("expected selected album title position");
        assert_eq!(
            hint_x, title_x,
            "track-mode hint should align with album title"
        );
        assert!(
            lines[track_y].starts_with("  \u{258c}1."),
            "track list should be indented 2 columns from the album block title:\n{out}"
        );
        let icon_byte_x = playing_line
            .find(icon)
            .expect("expected active music track icon");
        let icon_x = playing_line[..icon_byte_x].chars().count() as u16;
        let title_byte_x = playing_line
            .find("Opening Track")
            .expect("expected active music track title");
        let active_title_x = playing_line[..title_byte_x].chars().count() as u16;
        let buffer = term.backend().buffer();
        assert_eq!(
            buffer[(icon_x, track_y as u16)].fg,
            palette::AQUA,
            "expected active icon to be AQUA at x={icon_x}:\n{out}"
        );
        assert_eq!(buffer[(active_title_x, track_y as u16)].fg, palette::YELLOW);
        assert_eq!(
            term.backend().buffer()[(hint_x as u16, hint_y as u16)].fg,
            palette::SOFT_WHITE,
            "expected track-mode action hints to render soft white:\n{out}"
        );
        assert_eq!(
            art_rect.x + art_rect.width,
            58,
            "album art should have two columns of right padding"
        );
        assert_eq!((art_rect.width, art_rect.height), (24, 12));
        assert!(app.card_image_loading.contains("album-1:P"));
        assert!(!app.card_image_loading.contains("track-1:P"));
        assert_eq!(
            term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
            palette::OVERLAY,
            "loading album art should reserve a right-aligned placeholder:\n{out}"
        );
    }

    #[test]
    fn album_folder_inline_detail_keeps_title_gutter_when_library_pane_unfocused() {
        // Selection now reads via a colored block + white title text, not the
        // legacy `▌` marker -- confirm that block dims (rather than
        // disappearing) and the title stays white when the pane loses focus.
        let mut app = make_power_music_group_app();

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutMain::default();
        let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
        let out = buffer_to_string(&term);
        let title_y = out
            .lines()
            .position(|line| line.contains("First Album"))
            .expect("expected selected album title row");
        let title_line = out.lines().nth(title_y).unwrap();
        let title_x = title_line
            .find("First Album")
            .expect("expected title text position") as u16;

        let buf = term.backend().buffer();
        assert_eq!(
            buf[(title_x, title_y as u16)].bg,
            palette::PLAYBACK_PANEL_BG,
            "selected album title row should keep a colored block background (dimmed) while unfocused:\n{out}"
        );
        assert_eq!(
            buf[(title_x, title_y as u16)].fg,
            palette::WHITE,
            "selected album title should keep its white text while unfocused:\n{out}"
        );
    }

    #[test]
    fn album_folder_listing_preserves_inline_track_focus_cursor() {
        let mut app = make_power_music_group_app();
        app.libs[0].album_track_focus = Some(1);

        let mut first = make_item("Opening Track", "Audio");
        first.id = "track-1".into();
        first.album = "First Album".into();
        first.artist = "Alpha".into();
        first.index_number = 1;

        let mut second = make_item("Focused Track", "Audio");
        second.id = "track-2".into();
        second.album = "First Album".into();
        second.artist = "Alpha".into();
        second.index_number = 2;

        app.album_tracks_cache
            .insert("album-1".into(), vec![first, second]);

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let focused_line = out
            .lines()
            .find(|line| line.contains("Focused Track"))
            .expect("expected focused track to render inline");
        let focused_y = out
            .lines()
            .position(|line| line.contains("Focused Track"))
            .expect("expected focused track row");
        let lines: Vec<&str> = out.lines().collect();
        let hint_y = lines
            .iter()
            .position(|line| line.contains("^P: Play"))
            .expect("expected track-mode action hint row");
        assert!(
            lines[hint_y].contains("BACK: Exit"),
            "expected track-mode hint row to show the exit hint:\n{out}"
        );
        assert!(
            lines[hint_y + 1].trim().is_empty(),
            "expected a blank row between the track-mode hint and tracks:\n{out}"
        );
        assert_eq!(
            focused_y,
            hint_y + 3,
            "expected second track after hint, blank separator, and first track:\n{out}"
        );

        assert!(
            // The AQUA `▌` cursor marker now has 2-column indent in track-selection mode.
            focused_line.starts_with("  \u{258c}2. Focused Track"),
            "expected focused track row to show the AQUA cursor marker with 2-column indent in track-selection mode:\n{out}"
        );
        assert_eq!(
            layout.cursor_screen_y,
            Some(focused_y as u16),
            "expected layout cursor to follow the focused inline track row"
        );
    }

    #[test]
    fn album_folder_track_focus_cursor_renders_when_library_pane_unfocused() {
        let mut app = make_power_music_group_app();
        app.libs[0].album_track_focus = Some(1);

        let mut first = make_item("Opening Track", "Audio");
        first.id = "track-1".into();
        first.album = "First Album".into();
        first.artist = "Alpha".into();
        first.index_number = 1;

        let mut second = make_item("Focused Track", "Audio");
        second.id = "track-2".into();
        second.album = "First Album".into();
        second.artist = "Alpha".into();
        second.index_number = 2;

        app.album_tracks_cache
            .insert("album-1".into(), vec![first, second]);

        let mut layout = LayoutMain::default();
        let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
        let out = buffer_to_string(&term);
        let focused_line = out
            .lines()
            .find(|line| line.contains("Focused Track"))
            .expect("expected focused track to render inline");

        assert!(
            // The AQUA `▌` cursor marker now has 2-column indent in track-selection mode.
            focused_line.starts_with("  \u{258c}2. Focused Track"),
            "expected track-selection row to show the AQUA cursor marker with 2-column indent while pane is unfocused:\n{out}"
        );
    }

    #[test]
    fn selected_album_item_follows_raw_cursor_not_display_order() {
        let mut app = make_power_music_group_app();

        // A second album whose artist sorts before "Alpha" -- if the cursor
        // were (mis)interpreted against the artist-grouped display order
        // instead of the raw `items` array, moving the cursor to 1 would
        // resolve to the wrong album here.
        let mut second_album = make_item("Zero Day", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Aaardvark".into();

        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.items.push(second_album);
            lvl.cursor = 1;
        }

        let selected = app
            .selected_album_item(0)
            .expect("expected a selected album at cursor 1");
        assert_eq!(
            selected.id, "album-2",
            "expected the raw items[cursor] entry, not a sorted/display-order lookup"
        );

        // In the music-group (pill selector) view, the inline-detail fetch
        // (and thus this test's target assertion) only happens once
        // track-selection mode has been entered.
        app.libs[0].album_track_focus = Some(0);

        let mut layout = LayoutMain::default();
        let _ = render_power_library_to_string(&mut app, &mut layout);
        assert!(
            app.album_tracks_loading.contains("album-2"),
            "expected the fetch triggered by rendering to target the cursor-selected \
             album (album-2), not album-1"
        );
        assert!(
            !app.album_tracks_loading.contains("album-1"),
            "album-1 is no longer selected, so it should not be (re)fetched"
        );
    }

    // ── #145 task 5: regression coverage for non-music Power View surfaces ──
    // `is_viewing_album_folders` gates on `collection_type == "music"`, so
    // this is provably unreachable for series/home-video libraries; the
    // tests below additionally prove the *render* path
    // (`render_power_library`) still picks the original single-pane
    // series/home-video renderer and never touches the new album-tracks
    // cache/track-focus machinery added in tasks 1-4.

    fn make_power_home_video_app() -> App {
        let mut app = make_app_stub();
        app.library_tab = 1;

        let mut library = make_item("Home Videos", "CollectionFolder");
        library.id = "lib-homevideos".into();
        library.is_folder = true;
        library.collection_type = "homevideos".into();

        let mut first = make_item("Birthday Clip", "Video");
        first.id = "video-1".into();
        let mut second = make_item("Vacation Clip", "Video");
        second.id = "video-2".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-homevideos".into(),
                title: "Home Videos".into(),
                items: vec![first, second],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    #[test]
    fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
        let mut app = make_power_home_video_app();
        let lib_idx = 0;

        assert!(
            !app.is_viewing_album_folders(lib_idx),
            "a homevideos library must never satisfy is_viewing_album_folders"
        );
        assert!(app.is_home_video_view(lib_idx));
        assert!(app.libs[lib_idx].album_track_focus.is_none());

        let mut layout = LayoutMain::default();
        let out = render_power_library_to_string(&mut app, &mut layout);

        assert!(
            out.contains("Birthday Clip"),
            "expected the original single-pane home-video list renderer to fire \
             unchanged:\n{out}"
        );
        assert!(
            app.album_tracks_cache.is_empty(),
            "home-video rendering must never touch the album-tracks cache added by #145"
        );
        assert!(
            app.libs[lib_idx].album_track_focus.is_none(),
            "home-video rendering must never set track-selection mode"
        );
    }

    #[test]
    fn letter_filter_buckets_match_emby_name_range_bounds() {
        // Verified empirically against a live Emby server (2026-07-22) that
        // NameStartsWithOrGreater/NameLessThan filter on SortName -- these
        // bounds must stay in lockstep with `letter_bucket`'s range labels.
        let ac = LetterFilter::for_index(0).unwrap();
        assert_eq!(ac.label, "A\u{2013}C");
        assert_eq!(ac.name_ge, Some("A"));
        assert_eq!(ac.name_lt, Some("D"));

        let vz = LetterFilter::for_index(7).unwrap();
        assert_eq!(vz.label, "V\u{2013}Z");
        assert_eq!(vz.name_ge, Some("V"));
        assert_eq!(vz.name_lt, None, "V–Z has no upper bound");

        let hash = LetterFilter::for_index(8).unwrap();
        assert_eq!(hash.label, "#");
        assert_eq!(hash.name_ge, None, "# has no lower bound");
        assert_eq!(hash.name_lt, Some("A"));

        assert!(LetterFilter::for_index(9).is_none());
        assert_eq!(LetterFilter::count(), 9);
        assert_eq!(LetterFilter::labels().len(), 9);
    }

    #[test]
    fn letter_filter_default_is_the_first_bucket() {
        assert_eq!(
            LetterFilter::default_filter(),
            LetterFilter::for_index(0).unwrap()
        );
    }

    fn make_power_large_movie_library_app(library_total: usize) -> App {
        let mut app = make_app_stub();
        app.library_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: Vec::new(),
                total_count: 0,
                cursor: 0,
                scroll: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: Some(library_total),
        });

        app
    }

    #[test]
    fn letter_pills_show_only_when_library_total_exceeds_threshold() {
        let mut small = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD);
        assert!(
            !small.should_show_letter_pills(0),
            "exactly the threshold must not qualify"
        );

        let mut large = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD + 1);
        assert!(large.should_show_letter_pills(0));

        // `render_power_library_to_string` calls `render_power_library`
        // directly, which is *below* the pill-row layout carve-out (that
        // lives in `render_main`, mirroring the music-group pills
        // row) -- go through the full view so the carve-out fires.
        let backend = TestBackend::new(60, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            large.render_main(
                f,
                Rect::new(0, 0, 60, 20),
                &mut layout,
                &mut crate::app::layout::LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);
        assert!(
            out.contains("A\u{2013}C"),
            "expected the default A–C pill to render:\n{out}"
        );
        assert!(
            !layout.selector_tabs.is_empty(),
            "expected pill hitboxes to be recorded for click dispatch"
        );

        // Rendering the small (non-qualifying) library must not show pills.
        let backend2 = TestBackend::new(60, 20);
        let mut term2 = Terminal::new(backend2).unwrap();
        let mut layout2 = LayoutMain::default();
        term2
            .draw(|f| {
                small.render_main(
                    f,
                    Rect::new(0, 0, 60, 20),
                    &mut layout2,
                    &mut crate::app::layout::LayoutPlayback::default(),
                    &mut Rect::default(),
                    &mut Rect::default(),
                    0,
                    false,
                    &None,
                );
            })
            .unwrap();
        let out2 = buffer_to_string(&term2);
        assert!(!out2.contains("A\u{2013}C"));
    }
}
