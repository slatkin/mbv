//! The `List` component (design.md "Component catalogue"): row rendering,
//! the row/cell padding the lists share, and the selected-row background
//! extension they compose with, extracted from and shared by movies/TV's list
//! renderers (`list_letter_groups.rs`, `media_list.rs`, both consumers of
//! `item_cell_spans`/`draw_column_selection_bleed` below) and reused by the
//! audiobookshelf show grid. `ListRenderCtx`/`DisplayRow` are its row
//! model; `render_right_scrollbar` (`widgets.rs`) is its `Scrollbar`.
//! Screens still call these functions directly and record their own row hit
//! targets on their own geometry rather than getting one back from a single
//! entry point -- unifying that return shape, and folding in grouped
//! Music's structurally different row model, is design.md's phase
//! 8 ("Unified mouse hit targets"), not this extraction phase.

use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::Frame;

#[cfg(test)]
pub(in crate::app) const SELECTED_BLOCK_SIDE_PADDING: u16 = 2;

/// Returns `palette::TEXT_EMPHASIS` when `focused`, `palette::TEXT_SECONDARY` otherwise.
pub(in crate::app::render) fn focused_or_subtle(focused: bool) -> Color {
    if focused {
        palette::TEXT_EMPHASIS
    } else {
        palette::TEXT_SECONDARY
    }
}

pub(in crate::app::render) enum DisplayRow {
    Spacer,
    LetterHeader(String),
    /// One display row: the item indices occupying it, in column order. In
    /// one-column mode every such row carries exactly one index, so both
    /// modes share a single rendering path with no `cols == 1` branch.
    Item(Vec<usize>),
}

/// The shared fixed-row flow plan for a browser renderer.
/// Callers provide their already-built rows; this plan owns clamped scroll and
/// the row structure used by painting and selection bleed.
pub(in crate::app::render) struct FixedRowPlan<'a> {
    display_rows: &'a [DisplayRow],
    total_display_rows: usize,
    offset: usize,
}

impl<'a> FixedRowPlan<'a> {
    pub(in crate::app::render) fn new(
        display_rows: &'a [DisplayRow],
        selected_row: usize,
        _selected_item: usize,
        visible_rows: u16,
        stored_offset: usize,
    ) -> Self {
        let height = visible_rows.max(1) as usize;
        let max_offset = display_rows.len().saturating_sub(height);
        let mut offset = stored_offset.min(max_offset);
        if selected_row < offset {
            offset = selected_row;
        } else if selected_row >= offset + height {
            offset = selected_row + 1 - height;
        }
        Self {
            display_rows,
            total_display_rows: display_rows.len(),
            offset,
        }
    }

    pub(in crate::app::render) fn offset(&self) -> usize {
        self.offset
    }

    pub(in crate::app::render) fn total_display_rows(&self) -> usize {
        self.total_display_rows
    }

    pub(in crate::app::render) fn display_row(&self, row: usize) -> Option<usize> {
        (row < self.display_rows.len()).then_some(row)
    }

    pub(in crate::app::render) fn item_rows(&self) -> Vec<Vec<usize>> {
        self.display_rows
            .iter()
            .map(|row| match row {
                DisplayRow::Item(indices) => indices.clone(),
                DisplayRow::Spacer | DisplayRow::LetterHeader(_) => Vec::new(),
            })
            .collect()
    }
}

/// Shared inputs to the per-kind row-rendering bodies of `render_list`
/// (`render_letter_grouped_rows`, `media_list::render_plain_rows`): the
/// prelude values both kinds' bodies read, factored out so each callee takes
/// one struct instead of the same six-plus positional arguments.
pub(in crate::app::render) struct ListRenderCtx<'a> {
    /// The list's scrolling area.
    pub(in crate::app::render) content_area: Rect,
    pub(in crate::app::render) items: &'a [mbv_core::api::EmbyItem],
    pub(in crate::app::render) cursor: usize,
    pub(in crate::app::render) stored_scroll: usize,
    /// Column count for this frame's list pane width (1 or 2).
    pub(in crate::app::render) cols: usize,
    pub(in crate::app::render) focused: bool,
}

/// Owned browser-list inputs shared by narrow and wide renderers. The shell
/// builds this once from the active source; renderers no longer choose between
/// search results and the navigation level while painting rows.
#[derive(Clone)]
pub(in crate::app) struct LibraryListRenderCtx {
    pub(in crate::app) items: Vec<mbv_core::api::EmbyItem>,
    pub(in crate::app::render) cursor: usize,
    pub(in crate::app::render) scroll: usize,
    pub(in crate::app) total_count: usize,
    pub(in crate::app) library_total: Option<usize>,
    pub(in crate::app) letter_filter: Option<super::super::LetterFilter>,
    pub(in crate::app) loading: bool,
    pub(in crate::app) search_query: Option<String>,
    pub(in crate::app) search_loading: bool,
    /// Session-only Wide hero list-pane width override (`None` = default
    /// ratio). Carried here so the wide TV/Music render contexts that embed
    /// this struct forward it into the shared split; normalized against the
    /// active content-area width by the arrangement, never stored clamped.
    pub(in crate::app) list_pane_width: Option<u16>,
}

impl LibraryListRenderCtx {
    pub(in crate::app) fn from_items(
        items: Vec<mbv_core::api::EmbyItem>,
        cursor: usize,
        scroll: usize,
    ) -> Self {
        let total_count = items.len();
        Self {
            items,
            cursor,
            scroll,
            total_count,
            library_total: None,
            letter_filter: None,
            loading: false,
            search_query: None,
            search_loading: false,
            list_pane_width: None,
        }
    }

    pub(in crate::app) fn with_search(mut self, query: String, loading: bool) -> Self {
        self.search_query = Some(query);
        self.search_loading = loading;
        self
    }

    pub(in crate::app) fn item_count(&self) -> usize {
        self.items.len()
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn selected_item(&self) -> Option<&mbv_core::api::EmbyItem> {
        self.items.get(self.cursor)
    }

    pub(in crate::app::render) fn rows(
        &self,
        content_area: Rect,
        cols: usize,
        focused: bool,
    ) -> ListRenderCtx<'_> {
        ListRenderCtx {
            content_area,
            items: &self.items,
            cursor: self.cursor,
            stored_scroll: self.scroll,
            cols,
            focused,
        }
    }

    pub(in crate::app) fn is_search_active(&self) -> bool {
        self.search_query.is_some()
    }

    pub(in crate::app) fn true_total(&self) -> usize {
        self.library_total.unwrap_or(self.total_count)
    }

    pub(in crate::app) fn has_letter_filter(&self) -> bool {
        self.letter_filter.is_some()
    }
}

/// Builds the title (+ optional release year) spans for one list row, shared
/// by both the letter-grouped and plain-list rendering branches (identical
/// styling logic, only how `title`/`year_str`/`avail` are computed differs
/// between the two call sites). The year keeps the green metadata role the
/// canonical media-list rows use for it. Every cell starts with a 1-column leading
/// space; the selected cell punches through to the library backdrop (the
/// surface table's `SelectedRow` row) in both one- and two-column mode. The
/// selected row's background is extended to the list's outer edge, outside
/// the row's own content area, by `draw_column_selection_bleed`.
pub(in crate::app::render) fn build_list_row_spans(
    title: String,
    year_str: String,
    selected: bool,
    fg: Color,
) -> Vec<Span<'static>> {
    let bg = palette::SELECTED_ROW_BG;
    let mut spans: Vec<Span> = if selected {
        let title_style = Style::default().fg(fg).bg(bg);
        vec![
            Span::styled(" ", Style::default().bg(bg)),
            Span::styled(title, title_style),
        ]
    } else {
        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
    };
    if !year_str.is_empty() {
        let year_style = if selected {
            Style::default().fg(palette::STATUS_AVAILABLE).bg(bg)
        } else {
            Style::default().fg(palette::STATUS_AVAILABLE)
        };
        spans.push(Span::styled(year_str, year_style));
    }
    spans
}

/// Builds the padded spans for one item rendered into a `cell_width`-wide
/// cell: the existing marker/title/metadata/truncation logic operating
/// against the narrower cell width. Returns the cell's spans plus trailing
/// padding so the next cell starts at its own x offset; `pad_to` is the
/// total width to fill (cell width, plus the inter-column gap for every
/// cell except the last in its row).
pub(in crate::app::render) fn item_cell_spans(
    title: String,
    year_str: String,
    selected: bool,
    fg: Color,
    pad_to: usize,
) -> Vec<Span<'static>> {
    let mut spans = build_list_row_spans(title, year_str, selected, fg);
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let pad = pad_to.saturating_sub(used);
    if pad > 0 {
        let pad_span = if selected {
            Span::styled(
                " ".repeat(pad),
                Style::default().bg(palette::SELECTED_ROW_BG),
            )
        } else {
            Span::raw(" ".repeat(pad))
        };
        spans.push(pad_span);
    }
    spans
}

/// Draws the library list's selected-row background extension after the
/// list has rendered, at the panel's outer edge: the left edge in single-column
/// mode or for a left-column selection, the right edge for a right-column
/// selection (symmetric). The background is extended 2 columns into the panel
/// margin so selection reaches the list's outer edge.
pub(in crate::app::render) fn draw_column_selection_bleed(
    f: &mut Frame,
    content_area: Rect,
    cursor: usize,
    item_rows: &[Vec<usize>],
    row_offset: usize,
) {
    draw_column_selection_bleed_with_background(
        f,
        content_area,
        cursor,
        item_rows,
        row_offset,
        palette::SELECTED_ROW_BG,
    );
}

/// Draws the selected row's background extension with the selected row's
/// surface. Most catalog lists use the library backdrop; Wide hero Feeds
/// rows use their focus-resolved surface instead. The background bleeds
/// 2 columns into the panel margin.
pub(in crate::app::render) fn draw_column_selection_bleed_with_background(
    f: &mut Frame,
    content_area: Rect,
    cursor: usize,
    item_rows: &[Vec<usize>],
    row_offset: usize,
    background: Color,
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
    let rect = if col_in_row == 0 {
        Rect {
            x: content_area.x.saturating_sub(2),
            y: row_y,
            width: 2,
            height: 1,
        }
    } else {
        Rect {
            x: content_area.x + content_area.width,
            y: row_y,
            width: 2,
            height: 1,
        }
    };
    f.render_widget(
        Block::default().style(Style::default().bg(background)),
        rect,
    );
}
