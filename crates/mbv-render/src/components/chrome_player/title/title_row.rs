use super::super::PlaybackRenderContext;
use super::super::title_part_fg;
use super::header::marquee_spans_at;
use super::palette;
use super::transport::width_u16;
use super::transport::{control_glyphs, padded_status_pill, render_transport_glyphs};
use crate::arrangements::playback_transport::{TransportMeasure, transport_buttons_fit};
use mbv_queue::PlaybackTitleParts;
use mbv_ui_model::ui_util::fmt_duration_short;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;
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
        Span::styled(
            pos_str,
            Style::default().fg(palette::Role::PlaybackMetaFg.color()),
        ),
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
    let borrowed: Vec<(&str, Color)> = parts
        .iter()
        .map(|(text, color)| (text.as_str(), *color))
        .collect();
    marquee_spans_at(
        &borrowed,
        max_width,
        ctx.marquee_text,
        ctx.marquee_started_at,
    )
}
