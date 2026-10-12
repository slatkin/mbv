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

/// The queue footer's retained pointer regions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueueFooterRegions {
    /// The playlist pill's icon plus name area, without outer pads.
    /// The mounted `QueueComponent` resolves clicks against it.
    pub playlist: Rect,
}

fn spans_width(spans: &[Span<'_>]) -> u16 {
    spans
        .iter()
        .map(|span| u16::try_from(span.content.width()).unwrap_or(u16::MAX))
        .fold(0, u16::saturating_add)
}

/// Paints the queue footer's status pills over the footer band's own fill
/// and returns the playlist pill's hit area.
///
/// Left side: the autosave indicator (when present), one space delimiter,
/// then the playlist source pill. Right side: the dirty indicator (when
/// present). Content spans carry no outer pads; this painter owns them.
#[must_use]
pub fn render_queue_status(
    frame: &mut Frame,
    area: Rect,
    playlist: Vec<Span<'static>>,
    autosave: Option<Vec<Span<'static>>>,
    dirty: Option<Vec<Span<'static>>>,
) -> QueueFooterRegions {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::QueuePanelBand, false).fill),
        ),
        area,
    );
    if area.width == 0 || area.height == 0 {
        return QueueFooterRegions::default();
    }
    let pill_bg = palette::surface_colors(palette::Surface::StatusBarPill, false).fill;
    let pad = Span::styled(" ", Style::default().bg(pill_bg));
    let mut left = vec![pad.clone()];
    if let Some(spans) = autosave {
        left.extend(spans);
        left.push(Span::styled(" ", Style::default().bg(pill_bg)));
    }
    let playlist_start_x = area.x.saturating_add(spans_width(&left));
    let playlist_width = spans_width(&playlist);
    left.extend(playlist);
    left.push(pad.clone());
    let left_width = spans_width(&left);
    frame.render_widget(Paragraph::new(Line::from(left)), area);
    if let Some(spans) = dirty {
        let mut right = vec![pad.clone()];
        right.extend(spans);
        right.push(pad);
        let width = spans_width(&right);
        let x = area.x.saturating_add(area.width.saturating_sub(width));
        let left_end = area.x.saturating_add(left_width);
        if width > 0 && x >= left_end {
            frame.render_widget(
                Paragraph::new(Line::from(right)),
                Rect {
                    x,
                    y: area.y,
                    width,
                    height: 1,
                },
            );
        }
    }
    let playlist_hit = if playlist_width == 0 || playlist_start_x >= area.right() {
        Rect::default()
    } else {
        Rect {
            x: playlist_start_x,
            y: area.y,
            width: playlist_width.min(area.right().saturating_sub(playlist_start_x)),
            height: 1,
        }
    };
    QueueFooterRegions {
        playlist: playlist_hit,
    }
}

#[cfg(test)]
mod tests;
