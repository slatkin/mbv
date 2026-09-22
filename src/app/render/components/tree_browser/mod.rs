//! Destination-neutral painting for the embedded [`TreeBrowser`].
//!
//! This module receives semantic row data only.  It owns indentation, depth
//! roles, selected and aggregate bars, zebra fills, trailing metadata, and
//! the selected-title marquee.

use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::components::list::tree_browser::TreePaintRow;
use crate::app::components::media_list::MediaSemanticState;
use crate::app::palette;
use crate::app::render::components::marquee::marquee_spans;

#[allow(dead_code)]
pub(in crate::app) fn render_tree_browser(
    frame: &mut Frame,
    area: Rect,
    rows: &[TreePaintRow],
    focused: bool,
    marquee_text: &mut String,
    marquee_started_at: &mut Instant,
) {
    let zebra = palette::surface_colors(palette::Surface::QueueColumn, focused).fill;
    let base = palette::surface_colors(palette::Surface::QueuePanel, focused).fill;

    for (index, row) in rows.iter().enumerate() {
        let Some(y) = area.y.checked_add(index as u16) else {
            break;
        };
        if y >= area.bottom() {
            break;
        }
        let row_area = Rect::new(area.x, y, area.width, 1);
        let fill = if row.selected || row.marked || row.aggregate_marked {
            palette::SELECTED_ROW_BG
        } else if row.root_index % 2 == 1 {
            zebra
        } else {
            base
        };
        let prefix = " ".repeat(row.depth.saturating_mul(2));
        let gutter = row.trailing.as_deref().map_or(0, |text| text.width() + 2);
        let budget = usize::from(area.width).saturating_sub(prefix.width() + gutter);
        let title_color = title_color(row);
        let mut spans = vec![Span::raw(prefix)];
        if focused && row.selected {
            spans.extend(marquee_spans(
                &row.title,
                &[(row.title.clone(), title_color)],
                budget,
                marquee_text,
                marquee_started_at,
                false,
            ));
        } else {
            spans.push(Span::styled(
                truncate(&row.title, budget),
                Style::default().fg(title_color),
            ));
        }
        let painted = spans.iter().map(|span| span.content.width()).sum::<usize>();
        if painted < usize::from(area.width).saturating_sub(gutter) {
            spans.push(Span::raw(
                " ".repeat(usize::from(area.width).saturating_sub(gutter) - painted),
            ));
        }
        if let Some(trailing) = &row.trailing {
            spans.push(Span::styled(
                format!("{:>width$}  ", trailing, width = trailing.width()),
                Style::default().fg(palette::STATUS_AVAILABLE),
            ));
        }
        let mut style = Style::default().bg(fill);
        if row.selected || row.marked || row.aggregate_marked {
            style = style.fg(palette::SELECTED_ROW_FG);
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(style), row_area);
    }

    // The tree owns one fixed presentation.  When its visible flow exceeds the
    // supplied area, reserve the final column for the shared semantic scrollbar.
    if rows.len() > usize::from(area.height) && area.width > 0 {
        let scrollbar_x = area.right() - 1;
        for y in area.y..area.bottom() {
            frame.render_widget(
                Paragraph::new("│").style(Style::default().fg(palette::SCROLLBAR)),
                Rect::new(scrollbar_x, y, 1, 1),
            );
        }
    }
}

#[allow(dead_code)]
fn title_color(row: &TreePaintRow) -> ratatui::style::Color {
    if row.marked {
        return palette::STATUS_AVAILABLE;
    }
    if row.aggregate_marked {
        return palette::TEXT_ACCENT_MUTED;
    }
    if matches!(
        row.semantic_state,
        MediaSemanticState::Active { .. } | MediaSemanticState::NowPlaying { .. }
    ) {
        return palette::TEXT_EMPHASIS;
    }
    match row.depth {
        0 => palette::MUSIC_HEADER,
        1 => palette::TEXT_FOCUS_ACCENT,
        _ => palette::ACCENT,
    }
}

#[allow(dead_code)]
fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let mut output = String::new();
    for character in text.chars() {
        if output.width() + character.len_utf8() >= width {
            break;
        }
        output.push(character);
    }
    output.push('…');
    output
}
