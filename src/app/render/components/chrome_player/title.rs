use super::super::chrome::play_icon;
use super::super::marquee;
use super::palette;
use super::title_part_fg;
use super::PlaybackRenderContext;
use crate::app::render::arrangements::playback_transport::{
    transport_buttons_fit, TransportMeasure,
};
use crate::app::ui_util::fmt_duration_short;
use mbv_core::playback_queue::PlaybackTitleParts;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// The transport control glyphs and their colours for one render context.
/// Shared by the single title row and the queue column's split rows so the
/// glyphs cannot drift between the two presentations.
struct TransportGlyphs {
    play: (&'static str, Color),
    stop: (&'static str, Color),
    prev: (&'static str, Color),
    next: (&'static str, Color),
}

fn width_u16(width: usize) -> u16 {
    u16::try_from(width).unwrap_or(u16::MAX)
}

fn control_glyphs(ctx: &PlaybackRenderContext<'_>, paused: bool) -> TransportGlyphs {
    let play = if paused {
        (play_icon(ctx.controls.use_nerd_fonts), palette::ACCENT)
    } else {
        (
            if ctx.controls.use_nerd_fonts {
                "\u{f04c}"
            } else {
                "||"
            },
            palette::TEXT_FOCUS_ACCENT,
        )
    };
    let stop = (
        if ctx.controls.use_nerd_fonts {
            "\u{f04d}"
        } else {
            "X"
        },
        if ctx.controls.availability.stop {
            palette::STATUS_ERROR
        } else {
            palette::TEXT_MUTED
        },
    );
    let prev = (
        if ctx.controls.use_nerd_fonts {
            "\u{f048}"
        } else {
            "<<"
        },
        if ctx.controls.availability.previous {
            palette::TEXT_STRONG
        } else {
            palette::TEXT_MUTED
        },
    );
    let next = (
        if ctx.controls.use_nerd_fonts {
            "\u{f051}"
        } else {
            ">>"
        },
        if ctx.controls.availability.next {
            palette::TEXT_STRONG
        } else {
            palette::TEXT_MUTED
        },
    );
    TransportGlyphs {
        play,
        stop,
        prev,
        next,
    }
}

/// The play/pause(+stop/next when `show_buttons`) glyph row and its hit-area
/// rects on `ctx.playback`. Shared by the single title row and the queue
/// column's upper split row so the glyphs and hit geometry cannot drift
/// between the two presentations.
fn render_transport_glyphs(
    ctx: &mut PlaybackRenderContext<'_>,
    row_y: u16,
    x0: u16,
    show_buttons: bool,
    glyph_text: &str,
    glyph_w: u16,
    stop_w: u16,
    next_w: u16,
    prev_w: u16,
    glyphs: &TransportGlyphs,
) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        glyph_text.to_string(),
        Style::default()
            .fg(glyphs.play.1)
            .add_modifier(Modifier::BOLD),
    )];
    let mut x = x0;
    ctx.playback.play_pause = Rect {
        x,
        y: row_y,
        width: glyph_w,
        height: 1,
    };
    x += glyph_w;
    if show_buttons {
        ctx.playback.stop = Rect {
            x,
            y: row_y,
            width: stop_w,
            height: 1,
        };
        x += stop_w;
        spans.push(Span::styled(
            glyphs.stop.0,
            Style::default().fg(glyphs.stop.1),
        ));
        spans.push(Span::raw(" "));
        x += 1;
        ctx.playback.prev = Rect {
            x,
            y: row_y,
            width: prev_w,
            height: 1,
        };
        spans.push(Span::styled(
            glyphs.prev.0,
            Style::default().fg(glyphs.prev.1),
        ));
        spans.push(Span::raw(" "));
        x += prev_w + 1;
        ctx.playback.next = Rect {
            x,
            y: row_y,
            width: next_w,
            height: 1,
        };
        spans.push(Span::styled(
            glyphs.next.0,
            Style::default().fg(glyphs.next.1),
        ));
        spans.push(Span::raw(" "));
    } else {
        ctx.playback.stop = Rect::default();
        ctx.playback.prev = Rect::default();
        ctx.playback.next = Rect::default();
    }
    spans
}

/// The status-indicator pills (codec/res/aud/sub, uppercased on the pill
/// surface): the right side of the single title row and of the queue
/// column's upper split row. Opens with one pill-background space so the
/// resolution pill never touches the panel fill on the left. No other
/// trailing space; callers pad after merging (`padded_status_pill`).
fn status_pill_spans(ctx: &PlaybackRenderContext<'_>) -> Vec<Span<'static>> {
    let pill_bg = palette::surface_colors(palette::Surface::PlaybackStatusPill, false).fill;
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
    if !right.is_empty() {
        right.insert(0, Span::styled(" ", Style::default().bg(pill_bg)));
    }
    right
}

/// The status pill with a pad on both sides: `status_pill_spans` opens with
/// the leading pad, this adds the matching trailing one so the value (e.g.
/// FLAC) never touches the panel fill on the right. Empty when there is no
/// cluster to paint.
fn padded_status_pill(ctx: &PlaybackRenderContext<'_>) -> Vec<Span<'static>> {
    let mut spans = status_pill_spans(ctx);
    if !spans.is_empty() {
        spans.push(Span::styled(
            " ",
            Style::default().bg(palette::surface_colors(
                palette::Surface::PlaybackStatusPill,
                false,
            )
            .fill),
        ));
    }
    spans
}

/// The queue column's split title band: content rows first, the transport
/// controls last. With a context part and a `third` row the band is three
/// rows — the show (context) and the `pos / dur` time on the first row, the
/// title alone with its marquee window on the second, the controls and
/// status pills on the bottom row — otherwise the title and the time share
/// the first row and the controls move to the row below. The show clips to
/// its row without scrolling; the marquee belongs to the title row. Hit
/// geometry rides the bottom row, where the glyphs paint.
pub(super) fn render_queue_title_rows(
    frame: &mut Frame,
    first: Rect,
    second: Rect,
    third: Option<Rect>,
    title: &str,
    title_color: Color,
    ctx: &mut PlaybackRenderContext<'_>,
) {
    if first.height == 0 || first.width == 0 || second.height == 0 || second.width == 0 {
        ctx.playback.play_pause = Rect::default();
        ctx.playback.stop = Rect::default();
        ctx.playback.prev = Rect::default();
        ctx.playback.next = Rect::default();
        return;
    }
    let panel_bg = palette::surface_colors(ctx.panel, ctx.controls.panel_focused).fill;
    let (pos_ticks, rt_ticks, paused) = ctx.controls.progress;
    let glyphs = control_glyphs(ctx, paused);
    // The pill is padded on both sides before measuring: without the
    // trailing pad the value (e.g. FLAC) touches the panel fill on the
    // right. `render_title_row` pads the same way after its own merge.
    let pills = padded_status_pill(ctx);
    let pills_w: u16 = pills
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    let pos_str = fmt_duration_short(pos_ticks / mbv_core::api::TICKS_PER_SECOND);
    let dur_str = fmt_duration_short(rt_ticks / mbv_core::api::TICKS_PER_SECOND);
    let time_text = format!("{pos_str}/{dur_str}");
    let time_w = width_u16(time_text.width());
    let has_context = ctx
        .title_parts
        .as_ref()
        .is_some_and(|parts| parts.context.is_some());
    if let (Some(third), true) = (third, has_context) {
        render_context_title_rows(frame, first, second, third, title, title_color, ctx);
        return;
    }
    // Two rows: the title and the time share the first row, the controls
    // and pills move to the row below.
    // ` <title> ... <pos / dur> ` — one left indent, at least one gap cell
    // before the time, one right indent.
    let title_max = first.width.saturating_sub(1 + 1 + time_w + 1) as usize;
    let title_parts = playback_title_spans(ctx.title_parts.as_ref(), title, title_color);
    let mut row = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    row.extend(marquee_spans(ctx, &title_parts, title_max));
    let row_w: u16 = row.iter().map(|span| width_u16(span.content.width())).sum();
    let gap = (first.width as usize).saturating_sub(row_w as usize + time_w as usize + 1);
    row.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    row.push(Span::styled(
        time_text,
        Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
    ));
    row.push(Span::styled(" ", Style::default().bg(panel_bg)));
    frame.render_widget(
        Paragraph::new(Line::from(row)).style(Style::default().bg(panel_bg)),
        first,
    );
    render_transport_pill_row(
        ctx,
        frame,
        inset_row(second),
        panel_bg,
        &glyphs,
        pills,
        pills_w,
    );
}

fn render_context_title_rows(
    frame: &mut Frame,
    first: Rect,
    second: Rect,
    third: Rect,
    title: &str,
    title_color: Color,
    ctx: &mut PlaybackRenderContext<'_>,
) {
    if third.height == 0 || third.width == 0 {
        return;
    }
    let panel_bg = palette::surface_colors(ctx.panel, ctx.controls.panel_focused).fill;
    let (pos_ticks, rt_ticks, paused) = ctx.controls.progress;
    let glyphs = control_glyphs(ctx, paused);
    let pills = padded_status_pill(ctx);
    let pills_w: u16 = pills
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    let pos_str = fmt_duration_short(pos_ticks / mbv_core::api::TICKS_PER_SECOND);
    let dur_str = fmt_duration_short(rt_ticks / mbv_core::api::TICKS_PER_SECOND);
    let time_text = format!("{pos_str}/{dur_str}");
    let time_w = width_u16(time_text.width());
    let Some(context) = ctx
        .title_parts
        .as_ref()
        .and_then(|parts| parts.context.as_ref())
        .map(|context| (context.text.clone(), title_part_fg(context.role)))
    else {
        return;
    };
    // First row: ` <show> ... <pos / dur> ` — the show left with one
    // space of indent, the time right with one space of indent. The
    // show clips to the row (no marquee); the marquee belongs to the
    // title row below.
    let show_max = first.width.saturating_sub(1 + 1 + time_w + 1) as usize;
    let mut row = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    let mut show = context.0.as_str();
    while show.width() > show_max {
        show = &show[..show.len() - show.chars().last().map_or(1, char::len_utf8)];
    }
    row.push(Span::styled(
        show.to_string(),
        Style::default().fg(context.1).bg(panel_bg),
    ));
    let row_w: u16 = row.iter().map(|span| width_u16(span.content.width())).sum();
    let gap = (first.width as usize).saturating_sub(row_w as usize + time_w as usize + 1);
    row.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    row.push(Span::styled(
        time_text,
        Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
    ));
    row.push(Span::styled(" ", Style::default().bg(panel_bg)));
    frame.render_widget(
        Paragraph::new(Line::from(row)).style(Style::default().bg(panel_bg)),
        first,
    );
    // Second row: ` <title> ` alone, the marquee window sized to the row
    // minus its two indent cells.
    let title_parts = ctx
        .title_parts
        .as_ref()
        .map(|parts| vec![(parts.title.text.clone(), title_part_fg(parts.title.role))]);
    let fallback = [(title.to_string(), title_color)];
    let title_parts = title_parts.as_deref().unwrap_or(&fallback[..]);
    let mut row = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    row.extend(marquee_spans(
        ctx,
        title_parts,
        second.width.saturating_sub(2) as usize,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(row)).style(Style::default().bg(panel_bg)),
        second,
    );
    // Bottom row: the transport controls and the status pills.
    render_transport_pill_row(
        ctx,
        frame,
        inset_row(third),
        panel_bg,
        &glyphs,
        pills,
        pills_w,
    );
}

/// One band row inset one column each side: the controls row's paint rect.
fn inset_row(row: Rect) -> Rect {
    Rect {
        x: row.x + 1,
        width: row.width.saturating_sub(2),
        ..row
    }
}

/// The transport controls and status pills on one row: glyphs left, pills
/// flush right. The buttons show whenever the glyphs, buttons and pills
/// fit — no title competes on the controls row. Hit geometry lands on this
/// row.
fn render_transport_pill_row(
    ctx: &mut PlaybackRenderContext<'_>,
    frame: &mut Frame,
    row: Rect,
    panel_bg: Color,
    glyphs: &TransportGlyphs,
    pills: Vec<Span<'static>>,
    pills_w: u16,
) {
    if row.height == 0 || row.width == 0 {
        ctx.playback.play_pause = Rect::default();
        ctx.playback.stop = Rect::default();
        ctx.playback.next = Rect::default();
        return;
    }
    let glyph_text = format!("{} ", glyphs.play.0);
    let glyph_w = width_u16(glyph_text.width());
    let stop_w = width_u16(glyphs.stop.0.width());
    let prev_w = width_u16(glyphs.prev.0.width());
    let next_w = width_u16(glyphs.next.0.width());
    let buttons_w = stop_w as usize + 1 + prev_w as usize + 1 + next_w as usize + 1;
    let show_buttons = row.width as usize >= glyph_w as usize + buttons_w + pills_w as usize;
    let mut spans = render_transport_glyphs(
        ctx,
        row.y,
        row.x,
        show_buttons,
        &glyph_text,
        glyph_w,
        stop_w,
        next_w,
        prev_w,
        glyphs,
    );
    let left_w: u16 = spans
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    let gap = (row.width as usize).saturating_sub(left_w as usize + pills_w as usize);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(pills);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
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
        ctx.playback.play_pause = Rect::default();
        ctx.playback.stop = Rect::default();
        ctx.playback.prev = Rect::default();
        ctx.playback.next = Rect::default();
        return;
    }

    let (pos_ticks, _rt_ticks, paused) = ctx.controls.progress;
    let pos_str = fmt_duration_short(pos_ticks / mbv_core::api::TICKS_PER_SECOND);
    let glyphs = control_glyphs(ctx, paused);
    // The strip's right side is the elapsed time and the status pill alone:
    // no progress cluster (the seekbar above already carries progress) and no
    // total, so the narrow strip spends its columns on the title instead.
    let mut right = padded_status_pill(ctx);
    right.insert(0, Span::raw(" "));
    right.insert(
        0,
        Span::styled(pos_str, Style::default().fg(palette::PLAYBACK_META_FG)),
    );
    let right_w: u16 = right
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    let glyph_text = format!("{} ", glyphs.play.0);
    let glyph_w = width_u16(glyph_text.width());
    let stop_w = width_u16(glyphs.stop.0.width());
    let prev_w = width_u16(glyphs.prev.0.width());
    let next_w = width_u16(glyphs.next.0.width());
    let buttons_w = stop_w as usize + 1 + prev_w as usize + 1 + next_w as usize + 1;
    let available = area.width as usize;
    // The width-driven decision (task 3.5): whether the transport buttons
    // fit beside the title and the elapsed time, from the shared transport
    // arrangement.
    let show_buttons = transport_buttons_fit(
        area.width,
        width_u16(title.width()),
        TransportMeasure {
            glyph: glyph_w,
            buttons: width_u16(buttons_w),
            indicators: right_w,
        },
    );

    let mut left = render_transport_glyphs(
        ctx,
        area.y,
        area.x,
        show_buttons,
        &glyph_text,
        glyph_w,
        stop_w,
        next_w,
        prev_w,
        &glyphs,
    );
    let fixed_w = glyph_w as usize + right_w as usize + if show_buttons { buttons_w } else { 0 };
    let title_parts = playback_title_spans(ctx.title_parts.as_ref(), title, title_color);
    left.extend(marquee_spans(
        ctx,
        &title_parts,
        available.saturating_sub(fixed_w + 1),
    ));
    let left_w: u16 = left
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    let gap = available.saturating_sub(left_w as usize + right_w as usize);
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    frame.render_widget(
        Paragraph::new(Line::from(left)).style(
            Style::default()
                .bg(palette::surface_colors(ctx.panel, ctx.controls.panel_focused).fill),
        ),
        area,
    );
}

/// The painted (text, fg) spans for one now-playing title: the typed parts
/// resolved through `title_part_fg` when the shell projected them, otherwise
/// the attached target's plain title in its own colour. The context part
/// (the container: show, artist, feed) paints first; its trailing space
/// rides in its own span so the one-space delineation (D3) paints in the
/// context role.
fn playback_title_spans(
    parts: Option<&PlaybackTitleParts>,
    title: &str,
    title_color: Color,
) -> Vec<(String, Color)> {
    match parts {
        Some(parts) => {
            let mut spans = Vec::new();
            if let Some(context) = &parts.context {
                spans.push((format!("{} ", context.text), title_part_fg(context.role)));
            }
            spans.push((parts.title.text.clone(), title_part_fg(parts.title.role)));
            spans
        }
        None => vec![(title.to_string(), title_color)],
    }
}

pub(super) fn marquee_spans(
    ctx: &mut PlaybackRenderContext<'_>,
    parts: &[(String, Color)],
    max_width: usize,
) -> Vec<Span<'static>> {
    let key: String = parts.iter().map(|(text, _)| text.as_str()).collect();
    let borrowed: Vec<(&str, Color)> = parts
        .iter()
        .map(|(text, color)| (text.as_str(), *color))
        .collect();
    marquee::marquee_spans(
        &key,
        &borrowed,
        max_width,
        ctx.marquee_text,
        ctx.marquee_started_at,
        false,
    )
}
