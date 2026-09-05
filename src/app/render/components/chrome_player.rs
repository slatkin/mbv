use super::chrome::play_icon;
use crate::app::layout::LayoutPlayback;
use crate::app::palette;
use crate::app::ui_util::*;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub(in crate::app) struct PlaybackRenderContext<'a> {
    pub(in crate::app) area: Rect,
    pub(in crate::app) playback: &'a mut LayoutPlayback,
    pub(in crate::app) player_h: u16,
    pub(in crate::app) show_controls: bool,
    pub(in crate::app) now_playing_title: Option<(String, Color)>,
    pub(in crate::app) panel_bg: Color,
    pub(in crate::app) narrow_player: bool,
    pub(in crate::app) progress: (i64, i64, bool),
    pub(in crate::app) use_nerd_fonts: bool,
    pub(in crate::app) stop_available: bool,
    pub(in crate::app) next_available: bool,
    pub(in crate::app) status_indicators: Option<Vec<Span<'static>>>,
    pub(in crate::app) throbber: Span<'static>,
    pub(in crate::app) title_parts: Vec<(String, Color)>,
    pub(in crate::app) idle_feed_title: Option<(String, bool)>,
    pub(in crate::app) marquee_text: &'a mut String,
    pub(in crate::app) marquee_started_at: &'a mut std::time::Instant,
}

pub(in crate::app) fn render_player_panel(frame: &mut Frame, mut ctx: PlaybackRenderContext<'_>) {
    if ctx.player_h == 0 {
        return;
    }
    ctx.playback.idle_feed_link_area = Rect::default();

    let seek_area = Rect {
        height: 1,
        ..ctx.area
    };
    if ctx.show_controls {
        render_seekbar(frame, seek_area, ctx.playback, ctx.progress, ctx.panel_bg);
    } else {
        ctx.playback.seekbar_area = Rect::default();
        let bar = "\u{2594}".repeat(seek_area.width as usize);
        frame.render_widget(
            Paragraph::new(Span::styled(
                bar,
                Style::default().fg(palette::PROGRESS_TRACK),
            ))
            .style(Style::default().bg(ctx.panel_bg)),
            seek_area,
        );
    }

    if ctx.player_h >= 2 {
        let title_row_area = Rect {
            y: ctx.area.y + 1,
            height: 1,
            ..ctx.area
        };
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(title_row_area.width as usize)))
                .style(Style::default().bg(ctx.panel_bg)),
            title_row_area,
        );
        let title_area = Rect {
            x: ctx.area.x + 1,
            width: ctx.area.width.saturating_sub(2),
            y: ctx.area.y + 1,
            height: 1,
        };
        if let Some((title, color)) = ctx.now_playing_title.clone() {
            let row_title = if ctx.narrow_player {
                ""
            } else {
                title.as_str()
            };
            render_title_row(frame, title_area, row_title, color, &mut ctx);
        } else if !ctx.show_controls {
            if let Some((title, has_link)) = ctx.idle_feed_title.clone() {
                if has_link {
                    ctx.playback.idle_feed_link_area = title_area;
                }
                let spans = marquee_spans(
                    &mut ctx,
                    &[(title, palette::ACCENT)],
                    title_area.width as usize,
                );
                frame.render_widget(
                    Paragraph::new(Line::from(spans))
                        .style(Style::default().bg(ctx.panel_bg))
                        .alignment(Alignment::Center),
                    title_area,
                );
            }
        }
    }

    if ctx.player_h >= 3 {
        let blank_area = Rect {
            y: ctx.area.y + 2,
            height: 1,
            ..ctx.area
        };
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(blank_area.width as usize)))
                .style(Style::default().bg(ctx.panel_bg)),
            blank_area,
        );
    }

    if ctx.player_h >= 4 {
        let bottom_area = Rect {
            y: ctx.area.y + 3,
            height: 1,
            ..ctx.area
        };
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(bottom_area.width as usize)))
                .style(Style::default().bg(palette::SURFACE_BACKDROP)),
            bottom_area,
        );
        if ctx.narrow_player && ctx.show_controls {
            if let Some((title, color)) = ctx.now_playing_title.clone() {
                let prefix = "On Now: ";
                let inset_area = Rect {
                    x: bottom_area.x + 1,
                    width: bottom_area.width.saturating_sub(2),
                    ..bottom_area
                };
                let avail = inset_area.width as usize;
                let title_avail = avail.saturating_sub(prefix.width());
                let style = Style::default().fg(color);
                let label = format!("{prefix}{title}");
                let (line, alignment) = if label.width() <= avail || title_avail == 0 {
                    (
                        Line::from(Span::styled(trunc_str(&label, avail), style)),
                        Alignment::Center,
                    )
                } else {
                    let scrolled = marquee_spans(&mut ctx, &[(title, color)], title_avail)
                        .into_iter()
                        .next()
                        .map(|span| span.content.to_string())
                        .unwrap_or_default();
                    (
                        Line::from(vec![
                            Span::styled(prefix, style),
                            Span::styled(scrolled, style),
                        ]),
                        Alignment::Left,
                    )
                };
                frame.render_widget(
                    Paragraph::new(line)
                        .style(Style::default().bg(palette::SURFACE_BACKDROP))
                        .alignment(alignment),
                    inset_area,
                );
            }
        }
    }
}

fn render_seekbar(
    frame: &mut Frame,
    area: Rect,
    playback: &mut LayoutPlayback,
    (position, runtime, _paused): (i64, i64, bool),
    panel_bg: Color,
) {
    if area.height == 0 || area.width == 0 {
        playback.seekbar_area = Rect::default();
        return;
    }
    let ratio = if runtime > 0 {
        (position as f64 / runtime as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    playback.seekbar_area = area;
    let width = area.width as usize;
    let filled = ((ratio * width as f64).round() as usize).min(width);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "\u{2594}".repeat(filled),
                Style::default().fg(palette::ACCENT),
            ),
            Span::styled(
                "\u{2594}".repeat(width - filled),
                Style::default().fg(palette::PROGRESS_TRACK),
            ),
        ]))
        .style(Style::default().bg(panel_bg)),
        area,
    );
}

pub(in crate::app) fn render_title_row(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    title_color: Color,
    ctx: &mut PlaybackRenderContext<'_>,
) {
    if area.height == 0 || area.width == 0 {
        ctx.playback.play_pause_area = Rect::default();
        ctx.playback.stop_area = Rect::default();
        ctx.playback.next_area = Rect::default();
        return;
    }

    let (pos_ticks, rt_ticks, paused) = ctx.progress;
    let pos_str = fmt_duration_short(pos_ticks / mbv_core::api::TICKS_PER_SECOND);
    let dur_str = fmt_duration_short(rt_ticks / mbv_core::api::TICKS_PER_SECOND);
    let (glyph, gcolor): (&str, Color) = if paused {
        (play_icon(ctx.use_nerd_fonts), palette::ACCENT)
    } else {
        (
            if ctx.use_nerd_fonts { "\u{f04c}" } else { "||" },
            palette::TEXT_FOCUS_ACCENT,
        )
    };
    let stop_glyph = if ctx.use_nerd_fonts { "\u{f04d}" } else { "X" };
    let next_glyph = if ctx.use_nerd_fonts { "\u{f051}" } else { ">>" };
    let next_color = if ctx.next_available {
        palette::TEXT_STRONG
    } else {
        palette::TEXT_MUTED
    };
    let stop_color = if ctx.stop_available {
        palette::STATUS_ERROR
    } else {
        palette::TEXT_MUTED
    };
    let pill_bg = palette::SURFACE_BACKDROP;
    let mut codec_value_next = false;
    let mut right = ctx
        .status_indicators
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|span| Span::styled(span.content.to_uppercase(), span.style))
        .map(|span| {
            let is_caption = matches!(span.content.as_ref(), "CODEC " | "RES " | "AUD " | "SUB ");
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
                    span.style.fg(palette::PLAYBACK_VALUE_FG),
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
    for span in &mut right {
        *span = Span::styled(span.content.to_string(), span.style.bg(pill_bg));
    }
    let pct_str = fmt_playback_pct(pos_ticks, rt_ticks);
    let mut progress_spans = vec![
        Span::styled(
            ctx.throbber.content.to_string(),
            ctx.throbber.style.bg(pill_bg),
        ),
        Span::styled(
            pct_str,
            Style::default().fg(palette::TEXT_METADATA).bg(pill_bg),
        ),
    ];
    if right.is_empty() {
        right = progress_spans;
    } else {
        progress_spans.push(Span::styled(
            " \u{29F8} ",
            Style::default().fg(palette::BORDER_UNFOCUSED).bg(pill_bg),
        ));
        progress_spans.extend(right);
        right = progress_spans;
    }
    if !right.is_empty() {
        right.push(Span::styled(" ", Style::default().bg(pill_bg)));
    }

    let time_sep = " ";
    let right_full = {
        let mut spans = right.clone();
        spans.insert(
            0,
            Span::styled(
                format!("{pos_str} / {dur_str}"),
                Style::default().fg(palette::PLAYBACK_META_FG),
            ),
        );
        spans.insert(1, Span::raw(time_sep));
        spans
    };
    let right_elapsed = {
        let mut spans = right;
        spans.insert(
            0,
            Span::styled(
                pos_str.clone(),
                Style::default().fg(palette::PLAYBACK_META_FG),
            ),
        );
        spans.insert(1, Span::raw(time_sep));
        spans
    };
    let right_full_w: u16 = right_full
        .iter()
        .map(|span| span.content.width() as u16)
        .sum();
    let right_elapsed_w: u16 = right_elapsed
        .iter()
        .map(|span| span.content.width() as u16)
        .sum();
    let glyph_text = format!("{glyph} ");
    let glyph_w = glyph_text.width() as u16;
    let stop_w = stop_glyph.width() as u16;
    let next_w = next_glyph.width() as u16;
    let buttons_w = stop_w as usize + 1 + next_w as usize + 1;
    let available = area.width as usize;
    let mut show_buttons = true;
    let (right, right_w) = if ctx.narrow_player {
        (right_elapsed, right_elapsed_w)
    } else if available.saturating_sub(glyph_w as usize + right_full_w as usize + buttons_w)
        < title.width()
    {
        show_buttons = false;
        if available.saturating_sub(glyph_w as usize + right_full_w as usize) < title.width() {
            (right_elapsed, right_elapsed_w)
        } else {
            (right_full, right_full_w)
        }
    } else {
        (right_full, right_full_w)
    };

    let mut left = vec![Span::styled(
        glyph_text,
        Style::default().fg(gcolor).add_modifier(Modifier::BOLD),
    )];
    let mut x = area.x;
    ctx.playback.play_pause_area = Rect {
        x,
        y: area.y,
        width: glyph_w,
        height: 1,
    };
    x += glyph_w;
    if show_buttons {
        ctx.playback.stop_area = Rect {
            x,
            y: area.y,
            width: stop_w,
            height: 1,
        };
        x += stop_w;
        left.push(Span::styled(stop_glyph, Style::default().fg(stop_color)));
        left.push(Span::raw(" "));
        x += 1;
        ctx.playback.next_area = Rect {
            x,
            y: area.y,
            width: next_w,
            height: 1,
        };
        left.push(Span::styled(next_glyph, Style::default().fg(next_color)));
        left.push(Span::raw(" "));
    } else {
        ctx.playback.stop_area = Rect::default();
        ctx.playback.next_area = Rect::default();
    }
    let fixed_w = glyph_w as usize + right_w as usize + if show_buttons { buttons_w } else { 0 };
    let title_parts = if ctx.narrow_player || ctx.title_parts.is_empty() {
        vec![(title.to_string(), title_color)]
    } else {
        ctx.title_parts.clone()
    };
    left.extend(marquee_spans(
        ctx,
        &title_parts,
        available.saturating_sub(fixed_w + 1),
    ));
    let left_w: u16 = left.iter().map(|span| span.content.width() as u16).sum();
    let gap = available.saturating_sub(left_w as usize + right_w as usize);
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    frame.render_widget(
        Paragraph::new(Line::from(left)).style(Style::default().bg(ctx.panel_bg)),
        area,
    );
}

fn marquee_spans(
    ctx: &mut PlaybackRenderContext<'_>,
    parts: &[(String, Color)],
    max_width: usize,
) -> Vec<Span<'static>> {
    let total_width: usize = parts.iter().map(|(text, _)| text.width()).sum();
    if max_width == 0 || total_width <= max_width {
        return parts
            .iter()
            .map(|(text, color)| Span::styled(text.clone(), Style::default().fg(*color)))
            .collect();
    }
    let key: String = parts.iter().map(|(text, _)| text.as_str()).collect();
    if *ctx.marquee_text != key {
        *ctx.marquee_text = key;
        *ctx.marquee_started_at = std::time::Instant::now();
    }
    let overflow = total_width - max_width;
    colored_width_window(
        parts,
        marquee_col(overflow, ctx.marquee_started_at.elapsed().as_millis()),
        max_width,
    )
}

fn marquee_col(overflow: usize, elapsed_ms: u128) -> usize {
    if overflow == 0 {
        return 0;
    }
    const STEP_MS: u128 = 200;
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
            let char_width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if col + char_width <= start_col {
                col += char_width;
                continue;
            }
            if taken + char_width > width {
                break 'parts;
            }
            match &mut current {
                Some((value, current_color)) if current_color == color => value.push(c),
                _ => {
                    if let Some((value, current_color)) = current.take() {
                        spans.push(Span::styled(value, Style::default().fg(current_color)));
                    }
                    current = Some((c.to_string(), *color));
                }
            }
            taken += char_width;
            col += char_width;
        }
    }
    if let Some((value, color)) = current {
        spans.push(Span::styled(value, Style::default().fg(color)));
    }
    spans
}

#[cfg(test)]
mod tests {
    #[test]
    fn marquee_advances_five_columns_per_second() {
        assert_eq!(super::marquee_col(10, 1_200 + 200 * 5), 5);
    }
}
