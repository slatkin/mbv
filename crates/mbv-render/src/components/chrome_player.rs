use crate::arrangements::playback_transport::{TransportRows, transport_rows};
use mbv_queue::{PlaybackTitlePartRole, PlaybackTitleParts};
use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

mod title;

pub use title::render_title_row;
use title::{QueueBand, marquee_spans, render_queue_band};

#[derive(Clone, Default, Debug)]
pub struct PlaybackStripAreas {
    pub seekbar: Rect,
    pub play_pause: Rect,
    pub stop: Rect,
    pub next: Rect,
    pub prev: Rect,
}

#[derive(Debug)]
pub struct PlaybackControls {
    pub show: bool,
    pub use_nerd_fonts: bool,
    /// The transport-availability bundle shared with the playback
    /// projection (branch D10): the painter and the panels' hit-testing read
    /// the same three bits, never duplicate fields.
    pub availability: TransportAvailability,
    pub panel_focused: bool,
    pub progress: (i64, i64, bool),
    pub idle_feed_title: Option<(String, bool)>,
}

#[derive(Debug)]
pub struct PlaybackRenderContext<'a> {
    pub area: Rect,
    pub playback: &'a mut PlaybackStripAreas,
    pub player_h: u16,
    pub controls: PlaybackControls,
    pub now_playing_title: Option<(String, Color)>,
    /// The panel surface this playback chrome sits on plus the site's own
    /// focus bit; the painter resolves the fill through the surface table
    /// (`surface_colors`) instead of carrying a bare colour.
    pub panel: palette::Surface,
    pub status_indicators: Option<Vec<Span<'static>>>,
    /// The typed now-playing title parts with their closed roles (D6); the
    /// painter resolves a role to a colour, never the producer. `None` when
    /// the attached target is not addressable as a local queue item (a cast
    /// receiver or remote Session) — the plain `now_playing_title` carries
    /// that case.
    pub title_parts: Option<PlaybackTitleParts>,
    pub marquee_text: &'a mut String,
    pub marquee_started_at: &'a mut std::time::Instant,
}

pub fn render_player_panel(frame: &mut Frame, mut ctx: PlaybackRenderContext<'_>) {
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
    let panel_bg = palette::surface_colors(ctx.panel, ctx.controls.panel_focused).fill;
    // The queue column's band paints the controls row first (right below
    // the visual slot), one blank row, the title row, then the seekbar with
    // its flanking times; the Library strip keeps the seekbar on top with
    // the single title row below. Derived from the context's panel surface
    // so neither panel can point at the other's layout.
    let split = split_title_rows(ctx.panel);
    if split {
        render_queue_panel(frame, &mut ctx, rows, panel_bg);
        return;
    }
    match rows.seekbar {
        Some(seek_area) if ctx.controls.show => {
            render_seekbar(
                frame,
                seek_area,
                ctx.playback,
                ctx.controls.progress,
                panel_bg,
            );
        }
        Some(seek_area) => {
            ctx.playback.seekbar = Rect::default();
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
            ctx.playback.seekbar = Rect::default();
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
            render_title_row(frame, title_area, title.as_str(), color, &mut ctx);
        } else if !ctx.controls.show
            && let Some((title, _has_link)) = ctx.controls.idle_feed_title.clone()
        {
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

    if let Some(blank_area) = rows.indicator_row {
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(blank_area.width as usize)))
                .style(Style::default().bg(panel_bg)),
            blank_area,
        );
    }
}

/// The queue column's transport: the controls and status text on the top
/// row (right below the visual slot), the title row, the seekbar with its
/// flanking times, then one blank row. The shared [`transport_rows`]
/// positions still name the rows top-down; the queue band reinterprets
/// R0..R3 — the controls ride R0, R1 is the title row, the seekbar rides
/// R2, R3 stays blank — while the Library strip keeps the seekbar on R0.
/// Anything short of the four rows degrades to the rows present (controls
/// keep painting, missing rows clear their hit geometry).
fn render_queue_panel(
    frame: &mut Frame,
    ctx: &mut PlaybackRenderContext<'_>,
    rows: TransportRows,
    panel_bg: Color,
) {
    /// Blank one band row with the panel fill.
    fn blank(frame: &mut Frame, row: Rect, panel_bg: Color) {
        frame.render_widget(
            Paragraph::new(Span::raw(" ".repeat(row.width as usize)))
                .style(Style::default().bg(panel_bg)),
            row,
        );
    }
    /// Clear every hit rect: the band painted nothing actionable.
    fn clear_hits(ctx: &mut PlaybackRenderContext<'_>) {
        ctx.playback.play_pause = Rect::default();
        ctx.playback.stop = Rect::default();
        ctx.playback.prev = Rect::default();
        ctx.playback.next = Rect::default();
        ctx.playback.seekbar = Rect::default();
    }
    let (Some(controls_row), Some(title_row), Some(seek_row), Some(gap_row)) =
        (rows.seekbar, rows.title, rows.indicator_row, rows.extra_row)
    else {
        // Short of a full band: keep whatever top row exists painted and
        // clear the rest rather than borrow a neighbour's row.
        clear_hits(ctx);
        if let Some(first) = rows
            .seekbar
            .or(rows.title)
            .or(rows.indicator_row)
            .or(rows.extra_row)
        {
            blank(frame, first, panel_bg);
        }
        return;
    };
    if let Some((title, _)) = ctx.now_playing_title.clone() {
        render_queue_band(
            frame,
            &QueueBand {
                controls: controls_row,
                title: title_row,
                seek: seek_row,
                gap: gap_row,
            },
            title.as_str(),
            ctx,
        );
    } else {
        clear_hits(ctx);
        blank(frame, controls_row, panel_bg);
        blank(frame, title_row, panel_bg);
        blank(frame, seek_row, panel_bg);
        blank(frame, gap_row, panel_bg);
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
        playback.seekbar = Rect::default();
        return;
    }
    let ratio = if runtime > 0 {
        (mbv_emby_model::ticks_to_seconds(position) / mbv_emby_model::ticks_to_seconds(runtime))
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    playback.seekbar = area;
    let width = area.width as usize;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "seek fraction through f64; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    let filled = ((ratio * f64::from(area.width)).round() as usize).min(width);
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

/// Whether the panel behind `surface` paints the queue band layout (title
/// row(s), seekbar with flanking times, controls + pills) rather than the
/// Library strip's single title row.
fn split_title_rows(surface: palette::Surface) -> bool {
    surface == palette::Surface::QueueOnlyPlaybackPanel
}

/// Resolves a now-playing title part role to its theme role.
#[must_use]
pub fn title_part_fg(role: PlaybackTitlePartRole) -> Color {
    match role {
        PlaybackTitlePartRole::Title => palette::PLAYBACK_TITLE_FG,
        PlaybackTitlePartRole::Context => palette::PLAYBACK_CONTEXT_FG,
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransportAvailability {
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
}
