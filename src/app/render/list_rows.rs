//! The `List` component (design.md "Component catalogue"): row rendering,
//! the shared `SelectionMarker`, and the row/cell padding it composes with,
//! extracted from and shared by movies/TV's list renderers
//! (`list_letter_groups.rs`, `list_plain.rs`, both consumers of
//! `item_cell_spans`/`draw_column_selection_markers` below) and reused by
//! the audiobookshelf show grid. `ListRenderCtx`/`DisplayRow` are its row
//! model; `render_right_scrollbar` (`widgets.rs`) is its `Scrollbar`.
//! Screens still call these functions directly and record their own row hit
//! targets on `LayoutMain` rather than getting one back from a single
//! entry point -- unifying that return shape, and folding in grouped
//! Music's structurally different row model, is design.md's phase
//! 8 ("Unified mouse hit targets"), not this extraction phase.

use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

/// Standard inset for every selected detail block.
pub(super) const SELECTED_BLOCK_SIDE_PADDING: u16 = 2;

/// Returns `palette::SOFT_WHITE` when `focused`, `palette::SUBTLE` otherwise.
pub(super) fn focused_or_subtle(focused: bool) -> Color {
    if focused {
        palette::SOFT_WHITE
    } else {
        palette::SUBTLE
    }
}

/// Returns `palette::YELLOW` when `focused`, `palette::MUTED` otherwise.
pub(super) fn focused_or_muted(focused: bool) -> Color {
    if focused {
        palette::YELLOW
    } else {
        palette::MUTED
    }
}

/// Returns `palette::SOFT_WHITE` when `focused`, `palette::MUTED` otherwise.
pub(super) fn focused_or_muted_soft_white(focused: bool) -> Color {
    if focused {
        palette::SOFT_WHITE
    } else {
        palette::MUTED
    }
}

pub(super) enum DisplayRow {
    Spacer,
    LetterHeader(String),
    /// One display row: the item indices occupying it, in column order. In
    /// one-column mode every such row carries exactly one index, so both
    /// modes share a single rendering path with no `cols == 1` branch.
    Item(Vec<usize>),
}

/// Shared inputs to the per-kind row-rendering bodies of `render_list`
/// (`render_letter_grouped_rows`, `render_plain_rows`): the
/// prelude values both kinds' bodies read, factored out so each callee takes
/// one struct instead of the same six-plus positional arguments.
pub(super) struct ListRenderCtx<'a> {
    /// The list's own area: `render_list` splits `content_area` into
    /// this (the top slice, above the hero) and a separate `hero_area` (the
    /// bottom slice) -- the row renderer only ever sees `list_area` and has
    /// no notion of the hero at all.
    pub(super) content_area: Rect,
    pub(super) items: &'a [mbv_core::api::EmbyItem],
    pub(super) cursor: usize,
    pub(super) stored_scroll: usize,
    /// Column count for this frame's list pane width (1 or 2).
    pub(super) cols: usize,
    pub(super) focused: bool,
}

/// Builds the title (+ optional duration) spans for one list row, shared by
/// both the letter-grouped and plain-list rendering branches (identical
/// styling logic, only how `title`/`dur_str`/`avail` are computed differs
/// between the two call sites). Every cell starts with a 1-column leading
/// space; the selected cell carries a `palette::SURFACE_RESTING`
/// background, in both one- and two-column mode. The marker glyph itself
/// (the shared `SelectionMarker` component) is drawn separately, at the
/// list's outer edge, by `draw_column_selection_markers`.
pub(super) fn build_list_row_spans(
    title: String,
    dur_str: String,
    selected: bool,
    fg: Color,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = if selected {
        let bg = palette::SURFACE_RESTING;
        let title_style = Style::default().fg(palette::YELLOW).bg(bg);
        vec![
            Span::styled(" ", Style::default().bg(bg)),
            Span::styled(title, title_style),
        ]
    } else {
        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
    };
    if !dur_str.is_empty() {
        let dur_style = if selected {
            Style::default()
                .fg(palette::FOAM)
                .bg(palette::SURFACE_RESTING)
        } else {
            Style::default().fg(palette::FOAM)
        };
        spans.push(Span::styled(dur_str, dur_style));
    }
    spans
}

/// Builds the padded spans for one item rendered into a `cell_width`-wide
/// cell: the existing marker/title/metadata/truncation logic operating
/// against the narrower cell width. Returns the cell's spans plus trailing
/// padding so the next cell starts at its own x offset; `pad_to` is the
/// total width to fill (cell width, plus the inter-column gap for every
/// cell except the last in its row).
pub(super) fn item_cell_spans(
    title: String,
    dur_str: String,
    selected: bool,
    fg: Color,
    pad_to: usize,
) -> Vec<Span<'static>> {
    let mut spans = build_list_row_spans(title, dur_str, selected, fg);
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let pad = pad_to.saturating_sub(used);
    if pad > 0 {
        let pad_span = if selected {
            Span::styled(
                " ".repeat(pad),
                Style::default().bg(palette::SURFACE_RESTING),
            )
        } else {
            Span::raw(" ".repeat(pad))
        };
        spans.push(pad_span);
    }
    spans
}

/// Horizontal edge a `SelectionMarker` block sits at: the left edge for a
/// single-column list, or a two-column list's left column; the right edge
/// for a two-column list's right column.
pub(super) enum MarkerEdge {
    Left,
    Right,
}

/// The shared `SelectionMarker` component (design.md decision 2): a thin
/// `ACCENT`-role block, directional in two-column mode. `active` selects
/// the accent glyph vs. a blank column so unselected rows keep standard
/// alignment. Returns the styled span every list embeds as its marker;
/// `draw_column_selection_markers` uses the same glyph/color definition to
/// paint the library list's marker at the true outer edge, outside the
/// row's own content area.
pub(super) fn selection_marker(active: bool, edge: MarkerEdge) -> Span<'static> {
    if !active {
        return Span::raw(" ");
    }
    let glyph = match edge {
        MarkerEdge::Left => "\u{258e}",
        MarkerEdge::Right => "\u{1fb87}",
    };
    Span::styled(glyph, Style::default().fg(palette::ACCENT))
}

/// Draws the library list's column selection marker after the list has
/// rendered, at the panel's outer edge: the left edge in single-column
/// mode or for a left-column selection, the right edge for a right-column
/// selection (symmetric). The background is extended to cover the gap
/// between the marker and the cell content.
pub(super) fn draw_column_selection_markers(
    f: &mut Frame,
    content_area: Rect,
    cursor: usize,
    item_rows: &[Vec<usize>],
    row_offset: usize,
) {
    let Some(cursor_row) = item_rows.iter().position(|row| row.contains(&cursor)) else {
        return;
    };
    let Some(row_idx) = cursor_row.checked_sub(row_offset) else {
        return;
    };
    let col_in_row = item_rows[cursor_row]
        .iter()
        .position(|&idx| idx == cursor)
        .unwrap_or(0);

    let row_y = content_area.y + row_idx as u16;

    if col_in_row == 0 {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_RESTING)),
            Rect {
                x: content_area.x.saturating_sub(2),
                y: row_y,
                width: 2,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Line::from(selection_marker(true, MarkerEdge::Left))),
            Rect {
                x: content_area.x.saturating_sub(2),
                y: row_y,
                width: 1,
                height: 1,
            },
        );
    } else {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_RESTING)),
            Rect {
                x: content_area.x + content_area.width,
                y: row_y,
                width: 2,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Line::from(selection_marker(true, MarkerEdge::Right))),
            Rect {
                x: content_area.x + content_area.width + 1,
                y: row_y,
                width: 1,
                height: 1,
            },
        );
    }
}

#[cfg(test)]
mod selection_marker_tests {
    use super::*;

    // Replaces `home_latest_row.rs`'s deleted `row_unselected_has_no_marker`
    // (design.md decision 2 centralized every list's marker onto this one
    // component, so the "unselected rows carry no marker glyph" guarantee
    // belongs here now, not in a per-screen row painter).
    #[test]
    fn inactive_marker_is_blank() {
        for edge in [MarkerEdge::Left, MarkerEdge::Right] {
            let span = selection_marker(false, edge);
            assert_eq!(span.content.as_ref(), " ");
            assert_eq!(span.style.fg, None);
        }
    }

    #[test]
    fn active_marker_uses_directional_glyph() {
        assert_eq!(
            selection_marker(true, MarkerEdge::Left).content.as_ref(),
            "\u{258e}"
        );
        assert_eq!(
            selection_marker(true, MarkerEdge::Right).content.as_ref(),
            "\u{1fb87}"
        );
    }
}
