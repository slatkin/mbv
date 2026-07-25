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
#[path = "tests.rs"]
mod tests;
