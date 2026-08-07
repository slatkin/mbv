#![allow(unused_imports)]

use super::super::ui_util::*;
use super::chrome::play_icon;
use super::indicators;
use crate::app::layout::LayoutPlayback;
use crate::app::{
    palette, App, PanelFocus, RemoteSlotState, TABBAR_LEFT_RESERVE, TABBAR_RIGHT_RESERVE,
};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

fn uppercase_playback_span(span: Span<'static>) -> Span<'static> {
    Span::styled(span.content.to_uppercase(), span.style)
}

impl App {
    pub(super) fn render_player_panel(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPlayback,
        player_h: u16,
        show_controls: bool,
        now_playing_title: &Option<(String, Color)>,
        panel_bg: Color,
    ) {
        if player_h == 0 {
            return;
        }
        layout.idle_feed_link_area = Rect::default();
        // Seekbar row (always present when player_h > 0).
        let seek_area = Rect { height: 1, ..area };
        if show_controls {
            self.render_seekbar(f, seek_area, layout, panel_bg);
        } else {
            layout.seekbar_area = Rect::default();
            let bar = "\u{2594}".repeat(seek_area.width as usize);
            f.render_widget(
                Paragraph::new(Span::styled(bar, Style::default().fg(palette::SEEK_TRACK)))
                    .style(Style::default().bg(panel_bg)),
                seek_area,
            );
        }
        // Title row (when panel is expanded).
        if player_h >= 2 {
            let title_row_area = Rect {
                y: area.y + 1,
                height: 1,
                ..area
            };
            f.render_widget(
                Paragraph::new(Span::raw(" ".repeat(title_row_area.width as usize)))
                    .style(Style::default().bg(panel_bg)),
                title_row_area,
            );
            let title_area = Rect {
                x: area.x + 1,
                width: area.width.saturating_sub(2),
                y: area.y + 1,
                height: 1,
            };
            if let Some((ref title, color)) = now_playing_title {
                self.render_title_row(f, title_area, title, *color, layout, panel_bg);
            } else if !show_controls {
                // Idle state: show feed item title if available
                if let Some(ref idle_feed) = self.idle_feed {
                    if let Some(item) = idle_feed.items.get(idle_feed.current_index) {
                        let truncated_title = trunc_str(&item.title, title_area.width as usize);
                        if item.link.as_deref().is_some_and(|link| !link.is_empty()) {
                            layout.idle_feed_link_area = title_area;
                        }
                        f.render_widget(
                            Paragraph::new(Span::styled(
                                truncated_title,
                                Style::default().fg(palette::AQUA),
                            ))
                            .style(Style::default().bg(panel_bg))
                            .alignment(Alignment::Center),
                            title_area,
                        );
                    }
                }
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
                    .style(Style::default().bg(panel_bg)),
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
        panel_bg: Color,
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
        let mut right = self
            .build_status_indicator_spans()
            .unwrap_or_default()
            .into_iter()
            .map(|span| {
                let span = uppercase_playback_span(span);
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
            .map(|span| {
                Span::styled(
                    span.content.to_string(),
                    span.style.bg(palette::PLAYBACK_INDICATOR_BG),
                )
            })
            .collect::<Vec<_>>();
        let pill_bg = palette::PLAYBACK_INDICATOR_BG;
        let pct_str = fmt_playback_pct(pos_ticks, rt_ticks);
        let throbber = self.now_playing_throbber_span();
        let mut progress_spans: Vec<Span<'static>> = vec![
            Span::styled(throbber.content, throbber.style.bg(pill_bg)),
            Span::styled(
                pct_str,
                Style::default().fg(palette::FOAM).bg(pill_bg),
            ),
        ];
        if right.is_empty() {
            right = progress_spans;
        } else {
            progress_spans.push(Span::styled(
                " \u{29F8} ",
                Style::default().fg(palette::OVERLAY).bg(pill_bg),
            ));
            progress_spans.extend(right);
            right = progress_spans;
        }
        if !right.is_empty() {
            right.push(Span::styled(
                " ",
                Style::default().bg(palette::PLAYBACK_INDICATOR_BG),
            ));
        }

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

        let time_text = format!("{pos_str} / {dur_str}");
        let post_time_gap = "  ";
        let right_w: u16 = right.iter().map(|s| s.content.width() as u16).sum();
        let fixed_w = glyph_w as usize
            + stop_w as usize
            + stop_gap.width()
            + next_w as usize
            + next_gap.width()
            + 2 // spaces between title and time
            + time_text.width()
            + post_time_gap.width()
            + right_w as usize;
        let title_w = (area.width as usize).saturating_sub(fixed_w);

        left.extend(self.playback_title_spans(title, title_color, title_w));

        left.push(Span::raw("  "));

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
            Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
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

    fn playback_title_spans(
        &mut self,
        title: &str,
        title_color: Color,
        max_width: usize,
    ) -> Vec<Span<'static>> {
        let playback = self.effective_playback_state();
        let parts = playback
            .active
            .then(|| self.playback_queue().items.get(playback.active_idx))
            .flatten()
            .filter(|item| item.item_type == "Episode" && !item.series_name.is_empty())
            .filter(|item| item.display_name() == title)
            .map(|item| {
                vec![
                    (item.series_name.clone(), palette::YELLOW),
                    (format!(" {}", item.name), palette::GREEN),
                ]
            })
            .unwrap_or_else(|| vec![(title.to_string(), title_color)]);

        let mut remaining = max_width;
        let mut spans = Vec::new();
        for (text, color) in parts {
            if remaining == 0 {
                break;
            }
            let clipped = trunc_str(&text, remaining);
            let width = clipped.width();
            spans.push(Span::styled(clipped, Style::default().fg(color)));
            remaining = remaining.saturating_sub(width);
            if width < text.width() {
                break;
            }
        }
        spans
    }
}
