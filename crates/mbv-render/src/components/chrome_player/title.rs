use super::super::chrome::play_icon;
use super::super::marquee;
use super::PlaybackRenderContext;
use super::palette;
use super::title_part_fg;
use crate::arrangements::playback_transport::{TransportMeasure, transport_buttons_fit};
use mbv_queue::PlaybackTitleParts;
use mbv_ui_model::ui_util::fmt_duration_short;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
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

/// The queue column's band rows in paint order: the transport controls and
/// status text, the title row, then the seekbar flanked by its times.
pub struct QueueBand {
    pub controls: Rect,
    pub title: Rect,
    pub seek: Rect,
}

/// The queue column's band: the controls and status text on the top row
/// (right below the visual slot), then the title row, then the seekbar with
/// the elapsed time left and the total time right (one space between each
/// time and the bar). A two-part title shares its row:
/// the context part (the show) left-aligned and clipped without scrolling,
/// the title part right-aligned with the marquee window of the remaining
/// space. A single title always paints yellow. Hit geometry rides the
/// controls row (the glyphs) and the seekbar's bar span — the time labels
/// never seek. Pure painter over projected state.
pub fn render_queue_band(
    frame: &mut Frame,
    band: &QueueBand,
    title: &str,
    ctx: &mut PlaybackRenderContext<'_>,
) {
    if band.title.height == 0
        || band.title.width == 0
        || band.seek.height == 0
        || band.seek.width == 0
        || band.controls.height == 0
        || band.controls.width == 0
    {
        ctx.playback.play_pause = Rect::default();
        ctx.playback.stop = Rect::default();
        ctx.playback.prev = Rect::default();
        ctx.playback.next = Rect::default();
        ctx.playback.seekbar = Rect::default();
        return;
    }
    // A two-part title shares the one title row; anything else paints the
    // title alone.
    let panel_bg = palette::surface_colors(ctx.panel, ctx.controls.panel_focused).fill;
    let (_, _, paused) = ctx.controls.progress;
    let glyphs = control_glyphs(ctx, paused);
    // The top row's own fill (the slate backdrop role) plus its plain
    // indicator text, measured for the buttons-fit rule below.
    let row_bg = palette::SURFACE_BACKDROP;
    let (indicators, indicators_w) = queue_indicator_spans(ctx, row_bg);
    render_transport_controls_row(
        ctx,
        frame,
        band.controls,
        row_bg,
        &glyphs,
        indicators,
        indicators_w,
    );
    let has_context = ctx
        .title_parts
        .as_ref()
        .is_some_and(|parts| parts.context.is_some());
    if has_context {
        render_queue_combined_title(frame, band.title, title, ctx, panel_bg);
    } else {
        render_queue_title_only(frame, band.title, title, ctx, panel_bg);
    }
    render_queue_seek_row(frame, band.seek, ctx, panel_bg);
}

/// The queue band's status indicators as plain text on `row_bg`: no pill
/// wrap, so each projected span keeps its own foreground with the row fill
/// behind it, separator spans (`⧸`, `│`, `[`, `]`) are dropped, and the
/// surviving items join with one space. A chip-style span (dark text on its
/// own fill) recovers its fill as the text colour so it stays readable on
/// the row. Uppercased like the strip's cluster; one trailing space when
/// non-empty so the value never touches the row edge. Returns the spans
/// and their width.
fn queue_indicator_spans(
    ctx: &PlaybackRenderContext<'_>,
    row_bg: Color,
) -> (Vec<Span<'static>>, u16) {
    let mut items = Vec::new();
    for span in ctx.status_indicators.clone().unwrap_or_default() {
        let trimmed = span.content.trim();
        if trimmed.is_empty() || matches!(trimmed, "⧸" | "│" | "[" | "]") {
            continue;
        }
        let mut style = span.style;
        if style.fg == Some(palette::TEXT_ON_ACCENT) && style.bg.is_some() {
            style.fg = style.bg;
        }
        style.bg = Some(row_bg);
        items.push(Span::styled(trimmed.to_uppercase(), style));
    }
    let mut spans = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ", Style::default().bg(row_bg)));
        }
        spans.push(item);
    }
    if !spans.is_empty() {
        spans.push(Span::styled(" ", Style::default().bg(row_bg)));
    }
    let width: u16 = spans
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    (spans, width)
}

/// The queue band's status indicators as plain text on `row_bg`: no pill
/// wrap, so each projected span keeps its own foreground with the row fill
/// behind it, separator spans (`⧸`, `│`, `[`, `]`) are dropped, and the
/// surviving items join with one space. A chip-style span (dark text on its
/// own fill) recovers its fill as the text colour so it stays readable on
/// the row. Uppercased like the strip's cluster; one trailing space when
/// non-empty so the value never touches the row edge. Returns the spans
/// and their width.
/// One queue title row without its time: ` <title> ` with the marquee
/// window sized to the row minus its two indent cells. Only called when no
/// context part projects (a two-part title shares the combined row); the
/// lone title always paints yellow.
fn render_queue_title_only(
    frame: &mut Frame,
    row: Rect,
    title: &str,
    ctx: &mut PlaybackRenderContext<'_>,
    panel_bg: Color,
) {
    let text = match ctx.title_parts.as_ref() {
        Some(parts) => parts.title.text.clone(),
        None => title.to_string(),
    };
    let title_parts = vec![(text, palette::PLAYBACK_CONTEXT_FG)];
    let mut spans = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    spans.extend(marquee_spans(
        ctx,
        &title_parts,
        row.width.saturating_sub(2) as usize,
    ));
    let row_w: u16 = spans
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    let gap = (row.width as usize).saturating_sub(row_w as usize);
    if gap > 0 {
        spans.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
    );
}

/// A two-part title on one row: ` <show> ... <title> ` — the context part
/// left-aligned and clipped without scrolling (no marquee), the title part
/// right-aligned with the marquee window of the remaining space. The show
/// keeps priority: it clips only past `content - 2`, so the title always
/// keeps at least a one-cell marquee floor and no part is ever dropped.
/// Falls back to the single title row when no context part projects.
fn render_queue_combined_title(
    frame: &mut Frame,
    row: Rect,
    title: &str,
    ctx: &mut PlaybackRenderContext<'_>,
    panel_bg: Color,
) {
    let Some((show_text, show_fg, title_text, title_fg)) =
        ctx.title_parts.as_ref().and_then(|parts| {
            parts.context.as_ref().map(|context| {
                (
                    context.text.clone(),
                    title_part_fg(context.role),
                    parts.title.text.clone(),
                    title_part_fg(parts.title.role),
                )
            })
        })
    else {
        render_queue_title_only(frame, row, title, ctx, panel_bg);
        return;
    };
    let content = row.width.saturating_sub(2) as usize;
    let mut show = show_text.as_str();
    let show_max = content.saturating_sub(2);
    while show.width() > show_max {
        show = &show[..show.len() - show.chars().last().map_or(1, char::len_utf8)];
    }
    let sw = width_u16(show.width());
    let title_win = content.saturating_sub(sw as usize + 1);
    let mut spans = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    spans.push(Span::styled(
        show.to_string(),
        Style::default().fg(show_fg).bg(panel_bg),
    ));
    spans.extend(marquee_spans(ctx, &[(title_text, title_fg)], title_win));
    let mid_w: u16 = spans
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    // Right-anchor the title: every spare cell lands in the middle gap;
    // the trailing indent is kept outside the gap math.
    let gap = (row.width as usize).saturating_sub(mid_w as usize + 1);
    if gap > 0 {
        spans.insert(
            2,
            Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)),
        );
    }
    spans.push(Span::styled(" ", Style::default().bg(panel_bg)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(panel_bg)),
        row,
    );
}

/// The queue seekbar row: ` <elapsed> <bar> <total> ` — one outer indent
/// each side plus one space between each time and the bar. The bar is a
/// full-cell shade run: dense `▓` for the played span, sparse `░` for the
/// unplayed span (same ACCENT / track colours as before), spanning whatever
/// columns remain; only the bar seeks (the time labels never do). Too
/// narrow for any bar paints the two times left-aligned with no hit;
/// hidden controls (`!show`) keep the legacy track-only bar with no hit.
fn render_queue_seek_row(
    frame: &mut Frame,
    row: Rect,
    ctx: &mut PlaybackRenderContext<'_>,
    panel_bg: Color,
) {
    if !ctx.controls.show {
        ctx.playback.seekbar = Rect::default();
        let bar = "\u{2591}".repeat(row.width as usize);
        frame.render_widget(
            Paragraph::new(Span::styled(
                bar,
                Style::default().fg(palette::PROGRESS_TRACK),
            ))
            .style(Style::default().bg(panel_bg)),
            row,
        );
        return;
    }
    let (pos_ticks, rt_ticks, _paused) = ctx.controls.progress;
    let pos_str = fmt_duration_short(pos_ticks / mbv_emby_model::TICKS_PER_SECOND);
    let dur_str = fmt_duration_short(rt_ticks / mbv_emby_model::TICKS_PER_SECOND);
    let elapsed_w = width_u16(pos_str.width());
    let total_w = width_u16(dur_str.width());
    let bar_w = row
        .width
        .saturating_sub(1 + elapsed_w + 1 + 1 + total_w + 1);
    if bar_w == 0 {
        ctx.playback.seekbar = Rect::default();
        let text = format!("{pos_str} {dur_str}");
        let mut clipped = text.as_str();
        while clipped.width() > row.width as usize && !clipped.is_empty() {
            clipped = &clipped[..clipped.len() - clipped.chars().last().map_or(1, char::len_utf8)];
        }
        let gap = (row.width as usize).saturating_sub(clipped.width());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    clipped.to_string(),
                    Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
                ),
                Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)),
            ]))
            .style(Style::default().bg(panel_bg)),
            row,
        );
        return;
    }
    let ratio = if rt_ticks > 0 {
        (mbv_emby_model::ticks_to_seconds(pos_ticks) / mbv_emby_model::ticks_to_seconds(rt_ticks))
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "seek fraction through f64; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    let filled = ((ratio * f64::from(bar_w)).round() as usize).min(bar_w as usize);
    let rest = (bar_w as usize).saturating_sub(filled);
    ctx.playback.seekbar = Rect {
        x: row.x + 1 + elapsed_w + 1,
        y: row.y,
        width: bar_w,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default().bg(panel_bg)),
            Span::styled(
                pos_str,
                Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
            ),
            Span::styled(" ", Style::default().bg(panel_bg)),
            Span::styled(
                "\u{2593}".repeat(filled),
                Style::default().fg(palette::ACCENT),
            ),
            Span::styled(
                "\u{2591}".repeat(rest),
                Style::default().fg(palette::PROGRESS_TRACK),
            ),
            Span::styled(" ", Style::default().bg(panel_bg)),
            Span::styled(
                dur_str,
                Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
            ),
            Span::styled(" ", Style::default().bg(panel_bg)),
        ]))
        .style(Style::default().bg(panel_bg)),
        row,
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

/// The queue band's bottom controls row: glyphs left, indicators flush
/// right, the whole row on the slate fill edge to edge. The buttons show
/// whenever the glyphs, buttons and indicators fit — no title competes on
/// the row. Hit geometry lands on the inset cells, where the glyphs paint.
fn render_transport_controls_row(
    ctx: &mut PlaybackRenderContext<'_>,
    frame: &mut Frame,
    row: Rect,
    row_bg: Color,
    glyphs: &TransportGlyphs,
    indicators: Vec<Span<'static>>,
    indicators_w: u16,
) {
    if row.height == 0 || row.width == 0 {
        ctx.playback.play_pause = Rect::default();
        ctx.playback.stop = Rect::default();
        ctx.playback.next = Rect::default();
        return;
    }
    frame.render_widget(
        Paragraph::new(Span::raw(" ".repeat(row.width as usize)))
            .style(Style::default().bg(row_bg)),
        row,
    );
    let inner = inset_row(row);
    if inner.width == 0 {
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

pub fn render_title_row(
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
    let pos_str = fmt_duration_short(pos_ticks / mbv_emby_model::TICKS_PER_SECOND);
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

pub fn marquee_spans(
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
