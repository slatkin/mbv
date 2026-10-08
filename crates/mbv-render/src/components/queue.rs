use crate::arrangements::queue::empty_queue_box;
use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
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

/// Paints the empty-queue placeholder centered in the list content area: the
/// text row wrapped in one row of padding above and below and two columns of
/// padding on each side, in bold mauve. The box fill is the `QueueColumn`
/// surface (`#2e383c` while the queue holds focus, `#2b3238` while it does
/// not); the placeholder's stored leading indent is dropped because the box
/// already pads both sides.
pub fn render_empty_queue(frame: &mut Frame, content: Rect, focused: bool, text: &str) {
    if content.width == 0 || content.height == 0 {
        return;
    }
    let label = text.trim_start();
    let text_width = u16::try_from(label.width()).unwrap_or(u16::MAX);
    let box_area = empty_queue_box(content, text_width);
    if box_area.width == 0 || box_area.height == 0 {
        return;
    }
    let bg = palette::surface_colors(palette::Surface::QueueColumn, focused).fill;
    frame.render_widget(Block::default().style(Style::default().bg(bg)), box_area);
    let row = Rect {
        x: box_area.x,
        y: box_area.y + box_area.height / 2,
        width: box_area.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(label).alignment(Alignment::Center).style(
            Style::default()
                .fg(palette::EMPTY_QUEUE_FG)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        row,
    );
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
