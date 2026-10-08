use super::super::super::chrome::{pause_icon, play_icon};
use super::super::PlaybackRenderContext;
use super::palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthStr;
/// The transport control glyphs and their colours for one render context.
/// Shared by the single title row and the queue column's split rows so the
/// glyphs cannot drift between the two presentations.
pub(super) struct TransportGlyphs {
    pub(super) play: (&'static str, Color),
    pub(super) stop: (&'static str, Color),
    pub(super) prev: (&'static str, Color),
    pub(super) next: (&'static str, Color),
}

pub(super) fn width_u16(width: usize) -> u16 {
    u16::try_from(width).unwrap_or(u16::MAX)
}

pub(super) fn control_glyphs(ctx: &PlaybackRenderContext<'_>, paused: bool) -> TransportGlyphs {
    let play = if paused {
        (play_icon(ctx.controls.use_nerd_fonts), palette::ACCENT)
    } else {
        (
            pause_icon(ctx.controls.use_nerd_fonts),
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
pub(super) fn render_transport_glyphs(
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
/// surface): the Library strip's single title row. Opens with one
/// pill-background space so the resolution pill never touches the panel
/// fill on the left. No other trailing space; callers pad after merging
/// (`padded_status_pill`). The queue band paints its own plain-text cluster
/// (`queue_indicator_spans`) instead.
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
pub(super) fn padded_status_pill(ctx: &PlaybackRenderContext<'_>) -> Vec<Span<'static>> {
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

/// One band row inset one column each side: the controls row's paint rect.
fn inset_row(row: Rect) -> Rect {
    Rect {
        x: row.x + 1,
        width: row.width.saturating_sub(2),
        ..row
    }
}

/// The queue band's top controls row: glyphs left, indicators flush
/// right, the whole row on the slate fill edge to edge. The buttons show
/// whenever the glyphs, buttons and indicators fit — no title competes on
/// the row. Hit geometry lands on the inset cells, where the glyphs paint.
pub(super) fn render_transport_controls_row(
    ctx: &mut PlaybackRenderContext<'_>,
    frame: &mut Frame,
    row: Rect,
    row_bg: Color,
    glyphs: &TransportGlyphs,
    indicators: Vec<Span<'static>>,
    indicators_w: u16,
) {
    if row.height == 0 || row.width == 0 {
        ctx.playback.clear();
        return;
    }
    frame.render_widget(Block::default().style(Style::default().bg(row_bg)), row);
    let inner = inset_row(row);
    if inner.width == 0 {
        ctx.playback.clear();
        return;
    }
    let glyph_text = format!("{} ", glyphs.play.0);
    let glyph_w = width_u16(glyph_text.width());
    let stop_w = width_u16(glyphs.stop.0.width());
    let prev_w = width_u16(glyphs.prev.0.width());
    let next_w = width_u16(glyphs.next.0.width());
    let buttons_w = stop_w as usize + 1 + prev_w as usize + 1 + next_w as usize + 1;
    let show_buttons = inner.width as usize >= glyph_w as usize + buttons_w + indicators_w as usize;
    let mut spans = render_transport_glyphs(
        ctx,
        inner.y,
        inner.x,
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
    let gap = (inner.width as usize).saturating_sub(left_w as usize + indicators_w as usize);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(indicators);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(row_bg)),
        inner,
    );
}
