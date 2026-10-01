use crate::arrangements::playback_transport::{TransportRows, transport_rows};
use mbv_queue::{PlaybackTitlePartRole, PlaybackTitleParts};
use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

mod title;

pub use title::playback_state_icon;
pub use title::render_title_row;
pub use title::{HeaderTitle, render_header_title};
use title::{QueueBand, marquee_spans, render_queue_band};

#[derive(Clone, Default, Debug)]
pub struct PlaybackStripAreas {
    pub seekbar: Rect,
    pub play_pause: Rect,
    pub stop: Rect,
    pub next: Rect,
    pub prev: Rect,
}

impl PlaybackStripAreas {
    /// Drop every hit rect: nothing actionable painted.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
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
    if ctx.panel == palette::Surface::QueueOnlyPlaybackPanel {
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
/// row (right below the visual slot), the former title row kept blank, the
/// seekbar with its flanking times, then one blank row. The shared
/// [`transport_rows`] positions still name the rows top-down; the queue band
/// reinterprets R0..R3 — the controls ride R0, R1 stays blank (the title
/// lives on the header row), the seekbar rides R2, R3 stays blank — while
/// the Library strip keeps the seekbar on R0. Anything short of the four
/// rows degrades to the rows present (controls keep painting, missing rows
/// clear their hit geometry).
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
    let (Some(controls_row), Some(title_row), Some(seek_row), Some(gap_row)) =
        (rows.seekbar, rows.title, rows.indicator_row, rows.extra_row)
    else {
        // Short of a full band: keep whatever top row exists painted and
        // clear the rest rather than borrow a neighbour's row.
        ctx.playback.clear();
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
    if ctx.now_playing_title.is_some() {
        render_queue_band(
            frame,
            &QueueBand {
                controls: controls_row,
                seek: seek_row,
                gap: gap_row,
            },
            ctx,
        );
    } else {
        ctx.playback.clear();
        blank(frame, controls_row, panel_bg);
        blank(frame, seek_row, panel_bg);
        blank(frame, gap_row, panel_bg);
    }
    // The former title row stays as a blank band row (the title lives on
    // the header row now).
    blank(frame, title_row, panel_bg);
}

/// Played cells of a `width`-cell seekbar at `position` of `runtime` ticks.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "seek fraction through f64; no lossless integer-path conversion exists (approved, issue #804)"
)]
fn seek_fill(position: i64, runtime: i64, width: u16) -> usize {
    let ratio = if runtime > 0 {
        (mbv_emby_model::ticks_to_seconds(position) / mbv_emby_model::ticks_to_seconds(runtime))
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((ratio * f64::from(width)).round() as usize).min(usize::from(width))
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
    playback.seekbar = area;
    let width = area.width as usize;
    let filled = seek_fill(position, runtime, area.width);
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
