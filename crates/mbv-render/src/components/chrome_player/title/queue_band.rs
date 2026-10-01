use super::super::PlaybackRenderContext;
use super::super::seek_ratio;
use super::header::clip_to_width;
use super::palette;
use super::transport::width_u16;
use super::transport::{control_glyphs, render_transport_controls_row};
use mbv_ui_model::ui_util::fmt_duration_short;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};
use unicode_width::UnicodeWidthStr;
/// The queue column's painted band rows (the blank title row between the
/// controls and the seekbar is painted by the caller).
pub struct QueueBand {
    pub controls: Rect,
    pub seek: Rect,
    pub gap: Rect,
}

/// The queue column's band: the controls and status text on the top row
/// (right below the visual slot), then the title row, then the seekbar with
/// the elapsed time left and the total time right (one space between each
/// time and the bar). The band's former title row now paints blank: the
/// title lives on the header row (`render_header_title`). Hit geometry
/// rides the controls row (the glyphs) and the seekbar's bar span — the
/// time labels never seek. Pure painter over projected state.
pub fn render_queue_band(frame: &mut Frame, band: &QueueBand, ctx: &mut PlaybackRenderContext<'_>) {
    if band.controls.height == 0
        || band.controls.width == 0
        || band.seek.height == 0
        || band.seek.width == 0
        || band.gap.height == 0
        || band.gap.width == 0
    {
        ctx.playback.clear();
        return;
    }
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
    render_queue_seek_row(frame, band.seek, ctx, panel_bg);
    // The blank row below the seekbar: panel fill, no hit geometry.
    blank_row(frame, band.gap, panel_bg);
}

/// The queue band's status indicators as plain text on `row_bg`: no pill
/// wrap, so each projected span keeps its own foreground with the row fill
/// behind it, separator spans (`⧸`, `│`, `[`, `]`) are dropped, and the
/// surviving items join with one space. A chip-style span (dark text on its
/// own fill) recovers its fill as the text colour so it stays readable on
/// the row. Uppercased like the strip's cluster; no trailing space — the
/// controls row's one-column inset owns the edge pad, so a trailing space
/// here would double it. Returns the spans and their width.
fn queue_indicator_spans(
    ctx: &PlaybackRenderContext<'_>,
    row_bg: Color,
) -> (Vec<Span<'static>>, u16) {
    let mut spans = Vec::new();
    for span in ctx.status_indicators.as_deref().unwrap_or_default() {
        let trimmed = span.content.trim();
        if trimmed.is_empty() || matches!(trimmed, "⧸" | "│" | "[" | "]") {
            continue;
        }
        let mut style = span.style;
        if style.fg == Some(palette::TEXT_ON_ACCENT) && style.bg.is_some() {
            style.fg = style.bg;
        }
        style.bg = Some(row_bg);
        if !spans.is_empty() {
            spans.push(Span::styled(" ", Style::default().bg(row_bg)));
        }
        spans.push(Span::styled(trimmed.to_uppercase(), style));
    }
    let width: u16 = spans
        .iter()
        .map(|span| width_u16(span.content.width()))
        .sum();
    (spans, width)
}

/// The header row's state icon: the play glyph while playing, the pause
/// glyph while paused (same nerd-font/fallback pairs as the transport
/// controls), aqua for play and yellow for pause.
pub(in crate::components::chrome_player) fn blank_row(
    frame: &mut Frame,
    row: Rect,
    panel_bg: Color,
) {
    frame.render_widget(
        Paragraph::new(Span::raw(" ".repeat(row.width as usize)))
            .style(Style::default().bg(panel_bg)),
        row,
    );
}

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
        let clipped = clip_to_width(&text, row.width as usize);
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
    ctx.playback.seekbar = Rect {
        x: row.x + 1 + elapsed_w + 1,
        y: row.y,
        width: bar_w,
        height: 1,
    };
    // The whole row is repainted each pass, with the bar span precleared to
    // the panel fill: Gauge does not clear every old symbol in its unfilled
    // area, so a backward seek or resize would otherwise leave stale fill.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default().bg(panel_bg)),
            Span::styled(
                pos_str,
                Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
            ),
            Span::styled(" ", Style::default().bg(panel_bg)),
            Span::styled(" ".repeat(bar_w as usize), Style::default().bg(panel_bg)),
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
    // The active track's unplayed background is the band's own backdrop
    // slate (#272e33), matching the controls row above it; the Library strip
    // keeps the shared `PROGRESS_TRACK` grey.
    frame.render_widget(
        Gauge::default()
            .ratio(seek_ratio(pos_ticks, rt_ticks))
            .use_unicode(true)
            .label("")
            .gauge_style(
                Style::default()
                    .fg(palette::ACCENT)
                    .bg(palette::SURFACE_BACKDROP),
            ),
        ctx.playback.seekbar,
    );
}

#[cfg(test)]
mod tests;
