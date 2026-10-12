use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthStr;

/// The block-character `mbv` emblem painted above the empty-queue text.
/// Every line has the same width so the whole unit centers as one glyph
/// when each line centers in the content area.
const EMPTY_QUEUE_EMBLEM: [&str; 6] = [
    "        ██          ",
    "        ██          ",
    "████████████████  ██",
    "██ ██ ████  ████  ██",
    "██ ██ ████  ██ ████ ",
    "██ ██ ████████  ██  ",
];

/// Paints the empty-queue placeholder centered in the list content area in
/// bold mauve, over the panel's own fill. The block `mbv` emblem sits two rows
/// above the text (one blank spacer row between); the whole unit centers
/// vertically. A content area too short for the whole unit keeps the text
/// alone, centered.
pub fn render_empty_queue(frame: &mut Frame, content: Rect, text: &str) {
    if content.width == 0 || content.height == 0 {
        return;
    }
    let style = Style::default()
        .fg(palette::Role::EmptyQueueFg.color())
        .add_modifier(Modifier::BOLD);
    let emblem_rows = u16::try_from(EMPTY_QUEUE_EMBLEM.len()).unwrap_or(u16::MAX);
    let unit_height = emblem_rows + 2;
    let text_row = if content.height >= unit_height {
        let top = content.y + (content.height - unit_height) / 2;
        for (offset, line) in (0..emblem_rows).zip(EMPTY_QUEUE_EMBLEM) {
            frame.render_widget(
                Paragraph::new(line)
                    .alignment(Alignment::Center)
                    .style(style),
                Rect {
                    y: top + offset,
                    height: 1,
                    ..content
                },
            );
        }
        top + emblem_rows + 1
    } else {
        content.y + content.height / 2
    };
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(style),
        Rect {
            y: text_row,
            height: 1,
            ..content
        },
    );
}

/// Paints the queue footer's status pills: the playlist source pill
/// left-anchored and the autosave pill right-anchored, over the footer
/// band's own fill.
pub fn render_queue_status(
    frame: &mut Frame,
    area: Rect,
    playlist: Vec<Span<'static>>,
    autosave: Option<Vec<Span<'static>>>,
) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::QueuePanelBand, false).fill),
        ),
        area,
    );
    frame.render_widget(Paragraph::new(Line::from(playlist)), area);
    if let Some(spans) = autosave {
        let width = spans
            .iter()
            .map(|span| u16::try_from(span.content.width()).unwrap_or(u16::MAX))
            .sum::<u16>();
        let x = area.x + area.width.saturating_sub(width);
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
