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

/// The block-character `Q` emblem painted one row above the empty-queue
/// text. Every line has the same width so the whole unit centers as one
/// glyph when each line centers in the content area.
const EMPTY_QUEUE_EMBLEM: [&str; 8] = [
    " ██████ ",
    "██    ██",
    "██    ██",
    "██    ██",
    "██    ██",
    "██  █ ██",
    " ██████ ",
    "      ██",
];

/// Paints the empty-queue placeholder centered in the list content area in
/// bold mauve, over the panel's own fill. The block `Q` emblem sits one row
/// above the text; the emblem-plus-text unit centers vertically. A content
/// area too short for the whole unit keeps the text alone, centered.
pub fn render_empty_queue(frame: &mut Frame, content: Rect, text: &str) {
    if content.width == 0 || content.height == 0 {
        return;
    }
    let style = Style::default()
        .fg(palette::Role::EmptyQueueFg.color())
        .add_modifier(Modifier::BOLD);
    let emblem_rows = u16::try_from(EMPTY_QUEUE_EMBLEM.len()).unwrap_or(u16::MAX);
    let unit_height = emblem_rows + 1;
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
        top + emblem_rows
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
            Style::default().fg(palette::Role::TextFocusAccent.color()),
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
