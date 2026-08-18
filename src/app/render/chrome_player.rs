#![allow(unused_imports)]

use super::super::ui_util::*;
use super::chrome::play_icon;
use super::indicators;
use crate::app::layout::LayoutPlayback;
use crate::app::{palette, App, PanelFocus, PanelMode, RemoteSlotState, TABBAR_LEFT_RESERVE};
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

/// Column offset for a slow left-right-left marquee pan `elapsed_ms` into
/// its cycle, given `overflow` extra columns beyond the visible width.
/// Holds briefly at each end before reversing.
fn marquee_col(overflow: usize, elapsed_ms: u128) -> usize {
    if overflow == 0 {
        return 0;
    }
    const STEP_MS: u128 = 300;
    const HOLD_MS: u128 = 1200;
    let scroll_ms = overflow as u128 * STEP_MS;
    let cycle = 2 * HOLD_MS + 2 * scroll_ms;
    let t = elapsed_ms % cycle;
    if t < HOLD_MS {
        0
    } else if t < HOLD_MS + scroll_ms {
        ((t - HOLD_MS) / STEP_MS) as usize
    } else if t < 2 * HOLD_MS + scroll_ms {
        overflow
    } else {
        overflow - ((t - (2 * HOLD_MS + scroll_ms)) / STEP_MS) as usize
    }
}

/// Like `width_window`, but over color-tagged text segments (e.g. a series
/// name in one color followed by an episode name in another): slices the
/// same `width`-wide window starting at `start_col`, emitting one `Span` per
/// contiguous same-color run so the marquee preserves per-segment styling.
fn colored_width_window(
    parts: &[(String, Color)],
    start_col: usize,
    width: usize,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut col = 0usize;
    let mut taken = 0usize;
    let mut current: Option<(String, Color)> = None;
    'parts: for (text, color) in parts {
        for c in text.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if col + cw <= start_col {
                col += cw;
                continue;
            }
            if taken + cw > width {
                break 'parts;
            }
            match &mut current {
                Some((s, cur_color)) if cur_color == color => s.push(c),
                _ => {
                    if let Some((s, cur_color)) = current.take() {
                        spans.push(Span::styled(s, Style::default().fg(cur_color)));
                    }
                    current = Some((c.to_string(), *color));
                }
            }
            taken += cw;
            col += cw;
        }
    }
    if let Some((s, cur_color)) = current {
        spans.push(Span::styled(s, Style::default().fg(cur_color)));
    }
    spans
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
        // Narrow queue-only fork: the title leaves the crowded control row
        // and lives here on its own "Now Playing:" row, which is otherwise an
        // empty #2d353b fill (see explanation below the `player_h >= 4` block).
        let narrow_player = self.effective_panel_mode() == PanelMode::QueueOnly;

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
                if narrow_player {
                    // Narrow queue-only fork: the title moves out of the
                    // crowded control row to the bottom "Now Playing:" row
                    // (see the `player_h >= 4` block below); the control row
                    // keeps glyphs, time, and pills, just no title text.
                    self.render_title_row(f, title_area, "", *color, layout, panel_bg);
                } else {
                    self.render_title_row(f, title_area, title, *color, layout, panel_bg);
                }
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
            let bottom_area = Rect {
                y: area.y + 3,
                height: 1,
                ..area
            };
            // Plain #2d353b fill: in the two-pane view the library hero's top
            // border overwrites this row; in narrow queue-only mode it was
            // dead space, now repurposed for the "Now Playing:" title line.
            f.render_widget(
                Paragraph::new(Span::raw(" ".repeat(bottom_area.width as usize)))
                    .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                bottom_area,
            );
            if narrow_player && show_controls {
                if let Some((ref title, color)) = now_playing_title {
                    let prefix = "On Now: ";
                    let label = format!("{prefix}{title}");
                    // Indent: never let the label touch the panel edges.
                    let inset_area = Rect {
                        x: bottom_area.x + 1,
                        width: bottom_area.width.saturating_sub(2),
                        ..bottom_area
                    };
                    let avail = inset_area.width as usize;
                    let title_avail = avail.saturating_sub(prefix.width());
                    let style = Style::default().fg(*color);
                    let (line, alignment) = if label.width() <= avail || title_avail == 0 {
                        (
                            Line::from(Span::styled(trunc_str(&label, avail), style)),
                            Alignment::Center,
                        )
                    } else {
                        // Only the title pans; the "On Now: " prefix stays put.
                        self.sync_marquee_clock(title);
                        let overflow = title.width().saturating_sub(title_avail);
                        let col = marquee_col(
                            overflow,
                            self.now_playing_marquee_started_at.elapsed().as_millis(),
                        );
                        let scrolled =
                            colored_width_window(&[(title.clone(), *color)], col, title_avail)
                                .into_iter()
                                .next()
                                .map(|s| s.content.to_string())
                                .unwrap_or_default();
                        (
                            Line::from(vec![
                                Span::styled(prefix, style),
                                Span::styled(scrolled, style),
                            ]),
                            Alignment::Left,
                        )
                    };
                    f.render_widget(
                        Paragraph::new(line)
                            .style(Style::default().bg(palette::SURFACE_BACKDROP))
                            .alignment(alignment),
                        inset_area,
                    );
                }
            }
        }
    }

    /// Resets the shared marquee clock whenever the text it's tracking
    /// changes, so a newly-scrolling title always starts from the beginning
    /// instead of picking up mid-cycle.
    fn sync_marquee_clock(&mut self, text: &str) {
        if self.now_playing_marquee_text != text {
            self.now_playing_marquee_text = text.to_string();
            self.now_playing_marquee_started_at = std::time::Instant::now();
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
        let pos_str = fmt_duration_short(pos_ticks / TICKS_PER_SECOND);
        let dur_str = fmt_duration_short(rt_ticks / TICKS_PER_SECOND);
        // Narrow queue-only mode declutters the control row: no title text
        // and elapsed-only time (no duration).
        let narrow_player = self.effective_panel_mode() == PanelMode::QueueOnly;

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
                    span.style.bg(palette::SURFACE_BACKDROP),
                )
            })
            .collect::<Vec<_>>();
        let pill_bg = palette::SURFACE_BACKDROP;
        let pct_str = fmt_playback_pct(pos_ticks, rt_ticks);
        let throbber = self.now_playing_throbber_span();
        let mut progress_spans: Vec<Span<'static>> = vec![
            Span::styled(throbber.content, throbber.style.bg(pill_bg)),
            Span::styled(pct_str, Style::default().fg(palette::FOAM).bg(pill_bg)),
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
                Style::default().bg(palette::SURFACE_BACKDROP),
            ));
        }

        // Left: glyph  stop  next  title. Right (gap-filled): elapsed / total  pills
        // A running `x` cursor tracks where each clickable glyph lands in the
        // rendered `Line`, so `layout.*_area` exactly matches what's on screen
        // rather than an estimate.
        let time_sep = " ";
        let mut right_full = right; // status pills (with trailing space)
        let mut right_elapsed = right_full.clone();
        right_full.insert(
            0,
            Span::styled(
                format!("{pos_str} / {dur_str}"),
                Style::default().fg(palette::PLAYBACK_META_FG),
            ),
        );
        right_full.insert(1, Span::raw(time_sep));
        right_elapsed.insert(
            0,
            Span::styled(
                pos_str.clone(),
                Style::default().fg(palette::PLAYBACK_META_FG),
            ),
        );
        right_elapsed.insert(1, Span::raw(time_sep));
        let right_full_w: u16 = right_full.iter().map(|s| s.content.width() as u16).sum();
        let right_elapsed_w: u16 = right_elapsed.iter().map(|s| s.content.width() as u16).sum();

        let glyph_text = format!("{glyph} ");
        let glyph_w = glyph_text.width() as u16;
        let stop_w = stop_glyph.width() as u16;
        let next_w = next_glyph.width() as u16;
        let buttons_w = stop_w as usize + stop_gap.width() + next_w as usize + next_gap.width();
        let av = area.width as usize;

        // Sacrifice ladder for the normal panel, most intact first: keep both
        // the two buttons and the duration, then drop the buttons, then the
        // duration (elapsed only), and only split/truncate the title when none
        // of those fit. Narrow queue-only mode uses elapsed-only time and an
        // empty title (rendered separately on the "On Now:" bottom row).
        let mut show_buttons = true;
        let (mut right, mut right_w) = (right_full, right_full_w);
        if narrow_player {
            show_buttons = true;
            (right, right_w) = (right_elapsed, right_elapsed_w);
        } else if av.saturating_sub(glyph_w as usize + right_full_w as usize + buttons_w)
            < title.width()
        {
            // Drop the two buttons first...
            show_buttons = false;
            if av.saturating_sub(glyph_w as usize + right_full_w as usize) < title.width() {
                // ...then the duration (elapsed only) before truncating the title.
                (right, right_w) = (right_elapsed, right_elapsed_w);
            }
        }

        let mut left: Vec<Span> = Vec::new();
        let mut x = area.x;

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

        if show_buttons {
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

            layout.next_area = Rect {
                x,
                y: area.y,
                width: next_w,
                height: 1,
            };
            left.push(Span::styled(next_glyph, Style::default().fg(next_color)));
            left.push(Span::raw(next_gap));
        } else {
            layout.stop_area = Rect::default();
            layout.next_area = Rect::default();
        }

        let fixed_w =
            glyph_w as usize + right_w as usize + if show_buttons { buttons_w } else { 0 };
        let title_w = av.saturating_sub(fixed_w);

        left.extend(self.playback_title_spans(title, title_color, title_w));

        let left_w: u16 = left.iter().map(|s| s.content.width() as u16).sum();
        let gap = av.saturating_sub(left_w as usize + right_w as usize);

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
            .then(|| self.playback_queue().emby_item_at(playback.active_idx))
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

        let total_width: usize = parts.iter().map(|(text, _)| text.width()).sum();
        if max_width == 0 || total_width <= max_width {
            return parts
                .into_iter()
                .map(|(text, color)| Span::styled(text, Style::default().fg(color)))
                .collect();
        }

        // Too wide for the sacrifice ladder above to fully honor: showcase
        // it (slow pan across the full text) instead of truncating with "…".
        let marquee_key: String = parts.iter().map(|(text, _)| text.as_str()).collect();
        self.sync_marquee_clock(&marquee_key);
        let overflow = total_width - max_width;
        let col = marquee_col(
            overflow,
            self.now_playing_marquee_started_at.elapsed().as_millis(),
        );
        colored_width_window(&parts, col, max_width)
    }
}
