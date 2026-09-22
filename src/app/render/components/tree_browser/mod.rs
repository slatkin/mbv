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
    claim_rect: Rect,
    content_rect: Rect,
    rows: &[TreePaintRow],
    total_rows: usize,
    viewport_offset: usize,
    focused: bool,
    marquee_text: &mut String,
    marquee_started_at: &mut Instant,
) {
    let zebra = palette::surface_colors(palette::Surface::QueueColumn, focused).fill;
    let base = palette::surface_colors(palette::Surface::QueuePanel, focused).fill;
    let left_inset = usize::from(content_rect.x.saturating_sub(claim_rect.x));
    let right_inset = usize::from(claim_rect.right().saturating_sub(content_rect.right()));
    let content_width = usize::from(content_rect.width);

    for (index, row) in rows.iter().enumerate() {
        let Some(y) = content_rect.y.checked_add(index as u16) else {
            break;
        };
        if y >= content_rect.bottom() || content_width == 0 {
            break;
        }
        // Selected and marked rows claim the complete panel width. Ordinary
        // rows remain inside the parent's text insets, so the panel—not a
        // destination—owns the side bands around a tree.
        let full_width = row.selected || row.marked || row.aggregate_marked;
        let row_area = if full_width {
            Rect::new(claim_rect.x, y, claim_rect.width, 1)
        } else {
            Rect::new(content_rect.x, y, content_rect.width, 1)
        };
        let fill = if full_width {
            palette::SELECTED_ROW_BG
        } else if row.root_index % 2 == 1 {
            zebra
        } else {
            base
        };
        let prefix = " ".repeat(row.depth.saturating_mul(2));
        let gutter = row.trailing.as_deref().map_or(0, |text| text.width() + 2);
        let budget = content_width.saturating_sub(prefix.width() + gutter);
        let title_color = if full_width {
            palette::SELECTED_ROW_FG
        } else {
            title_color(row)
        };
        let mut spans = if full_width && left_inset > 0 {
            vec![Span::raw(" ".repeat(left_inset))]
        } else {
            Vec::new()
        };
        spans.push(Span::raw(prefix));
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
        let target_width = left_inset + content_width.saturating_sub(gutter);
        if painted < target_width {
            spans.push(Span::raw(" ".repeat(target_width - painted)));
        }
        if let Some(trailing) = &row.trailing {
            spans.push(Span::styled(
                format!("{:>width$}  ", trailing, width = trailing.width()),
                Style::default().fg(if full_width {
                    palette::SELECTED_ROW_FG
                } else {
                    palette::STATUS_AVAILABLE
                }),
            ));
        }
        if full_width && right_inset > 0 {
            spans.push(Span::raw(" ".repeat(right_inset)));
        }
        let mut style = Style::default().bg(fill);
        if full_width {
            style = style.fg(palette::SELECTED_ROW_FG);
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(style), row_area);
    }

    // The tree uses the same focus-gated scrollbar policy as the flat list.
    if focused && total_rows > usize::from(content_rect.height) && claim_rect.width > 0 {
        crate::app::render::components::widgets::render_right_scrollbar_with_viewport(
            frame,
            claim_rect,
            total_rows,
            usize::from(content_rect.height),
            viewport_offset,
            palette::SCROLLBAR,
        );
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
        if output.width() + unicode_width::UnicodeWidthChar::width(character).unwrap_or(0) >= width
        {
            break;
        }
        output.push(character);
    }
    output.push('…');
    output
}
