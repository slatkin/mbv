use super::super::ui_util::*;
use super::indicators;
use crate::app::layout::LayoutPlayback;
use crate::app::{
    palette, App, PanelFocus, RemoteSlotState, TABBAR_LEFT_RESERVE, TABBAR_RIGHT_RESERVE,
};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

pub(super) fn thin_vertical_thumb(mut glyphs: GlyphSet) -> GlyphSet {
    glyphs.thumb_vertical_lower = ['▕'; 8];
    glyphs.thumb_vertical_upper = ['▕'; 8];
    glyphs
}

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
    pub(super) fn toast_line(s: &str) -> Line<'static> {
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
    pub(super) fn render_tabs(
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
            .saturating_sub(2 * pb_h + TABBAR_LEFT_RESERVE + TABBAR_RIGHT_RESERVE);
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
    pub(super) fn render_player_panel(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPlayback,
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
    pub(super) fn render_title_row(
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
    pub(super) fn playback_progress(&self) -> (i64, i64, bool) {
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

    pub(super) fn remote_status_spans(
        &self,
        remote_state: RemoteSlotState,
        daemon_endpoint: &str,
    ) -> Vec<Span<'static>> {
        let remote_on = matches!(
            remote_state,
            RemoteSlotState::AttachedSession | RemoteSlotState::DirectRemote
        );
        let glyph_style = Style::default()
            .bg(palette::STATUS_PILL_BG)
            .fg(ratatui::style::Color::White);

        let target = match remote_state {
            RemoteSlotState::Off => None,
            RemoteSlotState::AttachedSession => {
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
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| daemon_endpoint_label(daemon_endpoint)),
            RemoteSlotState::LocalDaemon => None,
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

    pub(super) fn playlist_status_spans(&self) -> Vec<Span<'static>> {
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

    pub(super) fn mute_status_spans(&self) -> Option<Vec<Span<'static>>> {
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

    pub(super) fn status_width(spans: &[Span]) -> u16 {
        spans.iter().map(|s| s.content.width() as u16).sum()
    }

    pub(super) fn append_status(spans: &mut Vec<Span<'static>>, status: Vec<Span<'static>>) {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.extend(status);
    }

    pub(super) fn set_status_label_color(spans: &mut [Span<'static>], color: Color) {
        if let Some(label) = spans.get_mut(2) {
            label.style = label.style.fg(color);
        }
    }

    pub(super) fn set_status_pill_style(spans: &mut [Span<'static>], fg: Color, bg: Color) {
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

    pub(super) fn render_remote_status_hitbox(
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
    pub(super) fn render_status_bar(
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
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("ALBUM".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Series
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("SERIES".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Shuffle
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("SHUFFLE".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Remote
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
                {
                    Some(("REMOTE Q".to_string(), palette::MUTED))
                }
                crate::config::QueueSource::Collection { collection_type }
                    if matches!(self.panel_focus, PanelFocus::Queue) =>
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
            let autosave_on = matches!(self.panel_focus, PanelFocus::Queue)
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
    pub(super) fn render_seekbar(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPlayback,
    ) {
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
