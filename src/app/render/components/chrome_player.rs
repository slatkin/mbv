use super::chrome::play_icon;
use super::marquee;
use crate::app::palette;
use crate::app::render::arrangements::playback_transport::{
    transport_buttons_fit, transport_rows, TransportMeasure,
};
use crate::app::ui_util::*;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Default)]
pub(in crate::app) struct PlaybackStripAreas {
    pub(in crate::app) seekbar_area: Rect,
    pub(in crate::app) play_pause_area: Rect,
    pub(in crate::app) stop_area: Rect,
    pub(in crate::app) next_area: Rect,
}

pub(in crate::app) struct PlaybackRenderContext<'a> {
    pub(in crate::app) area: Rect,
    pub(in crate::app) playback: &'a mut PlaybackStripAreas,
    pub(in crate::app) player_h: u16,
    pub(in crate::app) show_controls: bool,
    pub(in crate::app) now_playing_title: Option<(String, Color)>,
    /// The panel surface this playback chrome sits on plus the site's own
    /// focus bit; the painter resolves the fill through the surface table
    /// (`surface_colors`) instead of carrying a bare colour.
    pub(in crate::app) panel: palette::Surface,
    pub(in crate::app) panel_focused: bool,
    pub(in crate::app) progress: (i64, i64, bool),
    pub(in crate::app) use_nerd_fonts: bool,
    pub(in crate::app) stop_available: bool,
    pub(in crate::app) next_available: bool,
    pub(in crate::app) status_indicators: Option<Vec<Span<'static>>>,
    pub(in crate::app) title_parts: Vec<(String, Color)>,
    pub(in crate::app) idle_feed_title: Option<(String, bool)>,
    pub(in crate::app) marquee_text: &'a mut String,
    pub(in crate::app) marquee_started_at: &'a mut std::time::Instant,
}

pub(in crate::app) fn render_player_panel(frame: &mut Frame, mut ctx: PlaybackRenderContext<'_>) {
    if ctx.player_h == 0 {
        return;
    }
    // The shared width-driven transport arrangement (task 3.5, D10): which
    // rows render in this panel, and which indicators/buttons the title row
    // shows at its width. Both playback panels route through here, so the
    // queue column and the right-column strip cannot drift at one width.
    let rows = transport_rows(ctx.area, ctx.player_h);
    // The ctx-driven sites resolve the panel surface the context carries: the
    // queue column's transport band (`QueueOnlyPlaybackPanel`) or the
    // right-column strip's own fill (`PlaybackPanel`), whose recess rects
    // share the value through this context.
    let panel_bg = palette::surface_colors(ctx.panel, ctx.panel_focused).fill;
    // The queue column splits its title band across two rows (controls +
    // pills up top, title + progress + time on the indicator row); the
    // Library strip keeps the single title row. Derived from the context's
    // panel surface so neither panel can point at the other's layout.
    let split = split_title_rows(ctx.panel);
    let mut indicator_painted = false;
    match rows.seekbar {
        Some(seek_area) if ctx.show_controls => {
            render_seekbar(frame, seek_area, ctx.playback, ctx.progress, panel_bg);
        }
        Some(seek_area) => {
            ctx.playback.seekbar_area = Rect::default();
            let bar = "\u{2594}".repeat(seek_area.width as usize);
            frame.render_widget(
                Paragraph::new(Span::styled(
                    bar,
                    Style::default().fg(palette::PROGRESS_TRACK),
                ))
                .style(Style::default().bg(panel_bg)),
                seek_area,
            );
        }
        None => {
            ctx.playback.seekbar_area = Rect::default();
        }
    }

    if let Some(title_row_area) = rows.title {
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(title_row_area.width as usize)))
                .style(Style::default().bg(panel_bg)),
            title_row_area,
        );
        let title_area = Rect {
            x: ctx.area.x + 1,
            width: ctx.area.width.saturating_sub(2),
            y: ctx.area.y + 1,
            height: 1,
        };
        if let Some((title, color)) = ctx.now_playing_title.clone() {
            if split {
                match rows.indicator_row {
                    Some(indicator_area) => {
                        render_queue_title_rows(
                            frame,
                            title_area,
                            indicator_area,
                            title.as_str(),
                            color,
                            &mut ctx,
                        );
                        indicator_painted = true;
                    }
                    // No indicator row to spill onto: fall back to the
                    // single title row rather than drop the title.
                    None => render_title_row(frame, title_area, title.as_str(), color, &mut ctx),
                }
            } else {
                render_title_row(frame, title_area, title.as_str(), color, &mut ctx);
            }
        } else if !ctx.show_controls {
            if let Some((title, _has_link)) = ctx.idle_feed_title.clone() {
                let spans = marquee_spans(
                    &mut ctx,
                    &[(title, palette::ACCENT)],
                    title_area.width as usize,
                );
                frame.render_widget(
                    Paragraph::new(Line::from(spans))
                        .style(Style::default().bg(panel_bg))
                        .alignment(Alignment::Center),
                    title_area,
                );
            }
        }
    }

    if let Some(blank_area) = rows.indicator_row {
        if indicator_painted {
            return;
        }
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(blank_area.width as usize)))
                .style(Style::default().bg(panel_bg)),
            blank_area,
        );
    }
}

fn render_seekbar(
    frame: &mut Frame,
    area: Rect,
    playback: &mut PlaybackStripAreas,
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

/// Whether the panel behind `surface` splits its title band across two rows:
/// the queue column's transport moves the title and time onto the blank
/// indicator row, while the Library strip keeps the single title row.
fn split_title_rows(surface: palette::Surface) -> bool {
    surface == palette::Surface::QueueOnlyPlaybackPanel
}

/// The transport control glyphs and their colours for one render context.
/// Shared by the single title row and the queue column's split rows so the
/// glyphs cannot drift between the two presentations.
struct TransportGlyphs {
    play: (&'static str, Color),
    stop: (&'static str, Color),
    next: (&'static str, Color),
}

fn control_glyphs(ctx: &PlaybackRenderContext<'_>, paused: bool) -> TransportGlyphs {
    let play = if paused {
        (play_icon(ctx.use_nerd_fonts), palette::ACCENT)
    } else {
        (
            if ctx.use_nerd_fonts { "\u{f04c}" } else { "||" },
            palette::TEXT_FOCUS_ACCENT,
        )
    };
    let stop = (
        if ctx.use_nerd_fonts { "\u{f04d}" } else { "X" },
        if ctx.stop_available {
            palette::STATUS_ERROR
        } else {
            palette::TEXT_MUTED
        },
    );
    let next = (
        if ctx.use_nerd_fonts { "\u{f051}" } else { ">>" },
        if ctx.next_available {
            palette::TEXT_STRONG
        } else {
            palette::TEXT_MUTED
        },
    );
    TransportGlyphs { play, stop, next }
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
    glyphs: &TransportGlyphs,
) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        glyph_text.to_string(),
        Style::default()
            .fg(glyphs.play.1)
            .add_modifier(Modifier::BOLD),
    )];
    let mut x = x0;
    ctx.playback.play_pause_area = Rect {
        x,
        y: row_y,
        width: glyph_w,
        height: 1,
    };
    x += glyph_w;
    if show_buttons {
        ctx.playback.stop_area = Rect {
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
        ctx.playback.next_area = Rect {
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
        ctx.playback.stop_area = Rect::default();
        ctx.playback.next_area = Rect::default();
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

/// The queue column's split title band: the upper row keeps the transport
/// controls and the status pills, while the title and the `pos / dur` time
/// move one row down onto the indicator row — the title left with one space
/// of indent, the time right with one space of indent. The title keeps
/// the shared marquee window, sized to the wider lower row. Hit geometry
/// stays on the upper row, where the glyphs paint.
fn render_queue_title_rows(
    frame: &mut Frame,
    upper: Rect,
    lower: Rect,
    title: &str,
    title_color: Color,
    ctx: &mut PlaybackRenderContext<'_>,
) {
    if upper.height == 0 || upper.width == 0 || lower.height == 0 || lower.width == 0 {
        ctx.playback.play_pause_area = Rect::default();
        ctx.playback.stop_area = Rect::default();
        ctx.playback.next_area = Rect::default();
        return;
    }
    let panel_bg = palette::surface_colors(ctx.panel, ctx.panel_focused).fill;
    let (pos_ticks, rt_ticks, paused) = ctx.progress;
    let glyphs = control_glyphs(ctx, paused);
    // The pill is padded on both sides before measuring: without the
    // trailing pad the value (e.g. FLAC) touches the panel fill on the
    // right. `render_title_row` pads the same way after its own merge.
    let pills = padded_status_pill(ctx);
    let pills_w: u16 = pills.iter().map(|span| span.content.width() as u16).sum();
    let glyph_text = format!("{} ", glyphs.play.0);
    let glyph_w = glyph_text.width() as u16;
    let stop_w = glyphs.stop.0.width() as u16;
    let next_w = glyphs.next.0.width() as u16;
    let buttons_w = stop_w as usize + 1 + next_w as usize + 1;
    // No title competes on the upper row, so the buttons show whenever the
    // glyphs, buttons and pills fit.
    let show_buttons = upper.width as usize >= glyph_w as usize + buttons_w + pills_w as usize;
    let mut upper_spans = render_transport_glyphs(
        ctx,
        upper.y,
        upper.x,
        show_buttons,
        &glyph_text,
        glyph_w,
        stop_w,
        next_w,
        &glyphs,
    );
    let upper_left_w: u16 = upper_spans
        .iter()
        .map(|span| span.content.width() as u16)
        .sum();
    let upper_gap = (upper.width as usize).saturating_sub(upper_left_w as usize + pills_w as usize);
    upper_spans.push(Span::raw(" ".repeat(upper_gap)));
    upper_spans.extend(pills);
    frame.render_widget(
        Paragraph::new(Line::from(upper_spans)).style(Style::default().bg(panel_bg)),
        upper,
    );
    // The lower row: ` <title> ... <pos / dur> `.
    let pos_str = fmt_duration_short(pos_ticks / mbv_core::api::TICKS_PER_SECOND);
    let dur_str = fmt_duration_short(rt_ticks / mbv_core::api::TICKS_PER_SECOND);
    let time_text = format!("{pos_str} / {dur_str}");
    let time_w = time_text.width() as u16;
    // One left indent, at least one gap cell before the time, one right
    // indent.
    let title_max = lower.width.saturating_sub(1 + 1 + time_w + 1) as usize;
    let title_parts = if ctx.title_parts.is_empty() {
        vec![(title.to_string(), title_color)]
    } else {
        ctx.title_parts.clone()
    };
    let mut row = vec![Span::styled(" ", Style::default().bg(panel_bg))];
    row.extend(marquee_spans(ctx, &title_parts, title_max));
    let row_w: u16 = row.iter().map(|span| span.content.width() as u16).sum();
    let gap = (lower.width as usize).saturating_sub(row_w as usize + time_w as usize + 1);
    row.push(Span::styled(" ".repeat(gap), Style::default().bg(panel_bg)));
    row.push(Span::styled(
        time_text,
        Style::default().fg(palette::PLAYBACK_META_FG).bg(panel_bg),
    ));
    row.push(Span::styled(" ", Style::default().bg(panel_bg)));
    frame.render_widget(
        Paragraph::new(Line::from(row)).style(Style::default().bg(panel_bg)),
        lower,
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

    let (pos_ticks, _rt_ticks, paused) = ctx.progress;
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
    let right_w: u16 = right.iter().map(|span| span.content.width() as u16).sum();
    let glyph_text = format!("{} ", glyphs.play.0);
    let glyph_w = glyph_text.width() as u16;
    let stop_w = glyphs.stop.0.width() as u16;
    let next_w = glyphs.next.0.width() as u16;
    let buttons_w = stop_w as usize + 1 + next_w as usize + 1;
    let available = area.width as usize;
    // The width-driven decision (task 3.5): whether the transport buttons
    // fit beside the title and the elapsed time, from the shared transport
    // arrangement.
    let show_buttons = transport_buttons_fit(
        area.width,
        title.width() as u16,
        TransportMeasure {
            glyph_w,
            buttons_w: buttons_w as u16,
            indicators_w: right_w,
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
        &glyphs,
    );
    let fixed_w = glyph_w as usize + right_w as usize + if show_buttons { buttons_w } else { 0 };
    let title_parts = if ctx.title_parts.is_empty() {
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
        Paragraph::new(Line::from(left))
            .style(Style::default().bg(palette::surface_colors(ctx.panel, ctx.panel_focused).fill)),
        area,
    );
}

fn marquee_spans(
    ctx: &mut PlaybackRenderContext<'_>,
    parts: &[(String, Color)],
    max_width: usize,
) -> Vec<Span<'static>> {
    let key: String = parts.iter().map(|(text, _)| text.as_str()).collect();
    marquee::marquee_spans(
        &key,
        parts,
        max_width,
        ctx.marquee_text,
        ctx.marquee_started_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// The queue column's split upper row pads the status pill on both
    /// sides: the value (e.g. FLAC) must not touch the panel fill on the
    /// right, mirroring the leading pad `status_pill_spans` opens with.
    #[test]
    fn split_upper_row_pads_the_status_pill_on_both_sides() {
        let pill_bg = palette::surface_colors(palette::Surface::PlaybackStatusPill, false).fill;
        let panel_bg =
            palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
        assert_ne!(
            pill_bg, panel_bg,
            "the test locates the pill by its fill, so the fills must differ"
        );
        let mut playback = PlaybackStripAreas::default();
        let mut marquee_text = String::new();
        let mut marquee_started_at = std::time::Instant::now();
        let mut ctx = PlaybackRenderContext {
            area: Rect::new(0, 0, 40, 2),
            playback: &mut playback,
            player_h: 3,
            show_controls: true,
            now_playing_title: None,
            panel: palette::Surface::QueueOnlyPlaybackPanel,
            panel_focused: false,
            progress: (0, 0, false),
            use_nerd_fonts: false,
            stop_available: false,
            next_available: false,
            status_indicators: Some(vec![Span::raw("CODEC "), Span::raw("FLAC")]),
            title_parts: Vec::new(),
            idle_feed_title: None,
            marquee_text: &mut marquee_text,
            marquee_started_at: &mut marquee_started_at,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 2)).unwrap();
        terminal
            .draw(|f| {
                render_queue_title_rows(
                    f,
                    Rect::new(0, 0, 40, 1),
                    Rect::new(0, 1, 40, 1),
                    "Title",
                    palette::TEXT_STRONG,
                    &mut ctx,
                )
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let pill_cells = (0..40)
            .filter(|&x| buf[(x, 0)].bg == pill_bg)
            .collect::<Vec<_>>();
        assert!(!pill_cells.is_empty(), "no pill painted on the upper row");
        let row_text = (0..40)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect::<String>();
        assert!(
            row_text.contains("FLAC"),
            "expected the codec value: {row_text:?}"
        );
        assert_eq!(
            buf[(pill_cells[0], 0)].symbol(),
            " ",
            "leading pill pad: {row_text:?}"
        );
        assert_eq!(
            buf[(*pill_cells.last().unwrap(), 0)].symbol(),
            " ",
            "trailing pill pad: {row_text:?}"
        );
        assert_eq!(
            *pill_cells.last().unwrap(),
            39,
            "the padded pill runs flush to the row edge: {row_text:?}"
        );
    }
}
