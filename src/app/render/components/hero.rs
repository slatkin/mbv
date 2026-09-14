//! Shared inline-hero flow accounting and search-row painting.

use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Rows the inline detail block spends outside its text: one blank spacer
/// row above the content and one below. The block's former `▁`/`▔` frame
/// rows are gone, so the budget no longer pays for them.
pub(in crate::app) const HERO_BLOCK_EXTRA_ROWS: u16 = 2;

pub(in crate::app) fn wrap_overview_lines(
    text: &str,
    mut width_for_line: impl FnMut(usize) -> usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let available = width_for_line(lines.len());
        if current.is_empty() {
            current.push_str(word);
        } else if current.width() + 1 + word.width() <= available {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub(in crate::app) struct InlineDetailFlow {
    pub offset: usize,
}

pub(in crate::app) enum InlineDisplayRow {
    Replacement,
    Source(usize),
}

pub(in crate::app) fn inline_display_row_count(
    source_rows: usize,
    selected_row: usize,
    detail_rows: u16,
) -> usize {
    if detail_rows == 0 || selected_row >= source_rows {
        source_rows
    } else {
        source_rows - 1 + detail_rows as usize
    }
}

pub(in crate::app) fn inline_display_row(
    source_rows: usize,
    selected_row: usize,
    detail_rows: u16,
    display_row: usize,
) -> Option<InlineDisplayRow> {
    let total = inline_display_row_count(source_rows, selected_row, detail_rows);
    if display_row >= total || selected_row >= source_rows {
        return None;
    }
    let detail_rows = detail_rows as usize;
    if detail_rows == 0 {
        Some(InlineDisplayRow::Source(display_row))
    } else if (selected_row..selected_row + detail_rows).contains(&display_row) {
        Some(InlineDisplayRow::Replacement)
    } else if display_row < selected_row {
        Some(InlineDisplayRow::Source(display_row))
    } else {
        Some(InlineDisplayRow::Source(display_row - detail_rows + 1))
    }
}

pub(in crate::app) fn inline_detail_flow(
    cursor_row: usize,
    detail_rows: u16,
    visible_rows: u16,
    stored_offset: usize,
) -> Option<InlineDetailFlow> {
    let detail_rows = detail_rows as usize;
    let visible_rows = visible_rows as usize;
    if detail_rows == 0 || detail_rows >= visible_rows {
        return None;
    }
    let lower_bound = (cursor_row + detail_rows)
        .saturating_sub(visible_rows)
        .min(cursor_row);
    Some(InlineDetailFlow {
        offset: stored_offset.clamp(lower_bound, cursor_row),
    })
}

pub(in crate::app) fn render_search_box(f: &mut Frame, area: Rect, query: &str, loading: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().bg(palette::surface_colors(palette::Surface::PillRow, false).fill);
    f.render_widget(Block::default().style(style), area);
    let input = if loading {
        format!("{query}█ [loading…]")
    } else {
        format!("{query}█")
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" SEARCH: {input}"),
            Style::default().fg(palette::TEXT_PRIMARY),
        ))),
        area,
    );
}
