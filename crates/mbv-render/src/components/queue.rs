use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthStr;

/// The remote-attachment indicator the queue footer paints: `Some` names
/// the playback target while a remote owner (daemon or session) owns
/// playback, `None` for a bare local player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueTitleModel {
    pub remote_pill: Option<String>,
}

pub fn render_queue_status(
    frame: &mut Frame,
    area: Rect,
    playlist: Vec<Span<'static>>,
    autosave: Option<Vec<Span<'static>>>,
    remote_pill: Option<&str>,
) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::QueuePanelBand, false).fill),
        ),
        area,
    );
    frame.render_widget(Paragraph::new(Line::from(playlist)), area);
    // The remote-attachment indicator pill: far right, winning over
    // autosave and the playlist on narrow footers.
    let pill = remote_pill.map(|content| {
        Span::styled(
            content.to_string(),
            Style::default().fg(palette::TEXT_FOCUS_ACCENT),
        )
    });
    let pill_w = pill
        .as_ref()
        .map_or(0, |s| u16::try_from(s.content.width()).unwrap_or(u16::MAX));
    if let Some(pill) = pill
        && pill_w > 0
        && pill_w < area.width
    {
        frame.render_widget(
            Paragraph::new(Line::from(vec![pill])),
            Rect {
                x: area.x + area.width - pill_w,
                y: area.y,
                width: pill_w,
                height: 1,
            },
        );
    }
    if let Some(spans) = autosave {
        let width = spans
            .iter()
            .map(|span| u16::try_from(span.content.width()).unwrap_or(u16::MAX))
            .sum::<u16>();
        // Autosave yields the far right to the indicator pill when shown.
        let x = area.x + area.width.saturating_sub(pill_w).saturating_sub(width);
        if x > area.x {
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect {
                    x,
                    y: area.y,
                    width,
                    height: 1,
                },
            );
        }
    }
}
