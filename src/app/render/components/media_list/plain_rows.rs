use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::palette;
use crate::app::render::components::list_rows::{
    focused_or_subtle, item_cell_spans, DisplayRow, FixedRowPlan, ListRenderCtx,
};
use crate::app::ui_util::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Canonical fixed-row render path for the canonical media-list controls.
/// Semantic theme ownership stays in `theme`/`palette` and `list_rows`.
///
/// Plain list kind of `render_list`: the catch-all covering the Home
/// "Continue Watching" tab, search result sets, small libraries, and
/// non-album music levels, all of which render identically without letter
/// grouping. Returns the scroll offset to persist.
/// Legacy painter (design.md D7): unit U2 deletes this file.
#[allow(dead_code)]
pub(in crate::app) fn render_plain_rows(f: &mut Frame, ctx: ListRenderCtx) -> usize {
    let ListRenderCtx {
        content_area,
        items,
        cursor,
        stored_scroll,
        cols,
        focused,
    } = ctx;
    let n = items.len();
    let visible = content_area.height as usize;
    let cell_w = library_cell_width(content_area, cols) as usize;

    // Build display rows row-major: item `i` occupies column `i % cols`
    // of row `i / cols`. In one-column mode every row carries exactly
    // one index, so both modes share this single path.
    let display_rows: Vec<DisplayRow> = (0..n)
        .collect::<Vec<_>>()
        .chunks(cols.max(1))
        .map(|item_row| DisplayRow::Item(item_row.to_vec()))
        .collect();
    // `display_cursor` is the index of the *row containing* the cursor
    // item, so the scroll clamp keeps the cursor row on screen.
    let display_cursor = display_rows
        .iter()
        .position(|r| matches!(r, DisplayRow::Item(idxs) if idxs.contains(&cursor)))
        .unwrap_or(0);
    let plan = FixedRowPlan::new(
        &display_rows,
        display_cursor,
        cursor,
        content_area.height,
        stored_scroll,
    );
    let offset = plan.offset();
    let total_display = plan.total_display_rows();
    let final_offset = offset;

    let show_scrollbar = focused && total_display > visible;

    let list_items: Vec<ListItem> = (offset..total_display)
        .take(visible)
        .map(|display_row| {
            let source_row = plan
                .display_row(display_row)
                .expect("display row is within the fixed-row flow");
            match &display_rows[source_row] {
                DisplayRow::Spacer | DisplayRow::LetterHeader(_) => ListItem::new(Line::default()),
                DisplayRow::Item(idxs) => {
                    // Each item renders into its own cell, truncated to the
                    // cell width; cells are padded to the cell boundary
                    // (+ inter-column gap) so the next cell starts at its
                    // own x offset. Trailing partial rows leave the empty
                    // cells as plain list background.
                    let mut spans: Vec<Span> = Vec::new();
                    for (cell_idx, &idx) in idxs.iter().enumerate() {
                        let item = &items[idx];
                        let selected = idx == cursor;

                        // Compute name and duration as separate strings so they can be styled
                        // independently: name in the normal fg, duration in OVERLAY (no parens).
                        let (item_name, year_str) = if item.is_folder {
                            let name = if item.item_type == "Folder" && item.total_count > 0 {
                                format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                            } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
                                format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                            } else {
                                item.display_name()
                            };
                            (name, String::new())
                        } else {
                            let year = if item.production_year > 0 {
                                format!(" {}", item.production_year)
                            } else {
                                String::new()
                            };
                            (item.display_name(), year)
                        };

                        // Every cell starts with a 1-column leading
                        // separator (the selected cell's highlight
                        // background, a plain space otherwise), so titles
                        // align across rows.
                        let avail = cell_w.saturating_sub(2);
                        let name_w = avail.saturating_sub(year_str.width());
                        let (title, year_str) = (trunc_str(&item_name, name_w), year_str);
                        let fg = focused_or_subtle(focused);

                        let pad_to = if cell_idx + 1 == idxs.len() {
                            cell_w
                        } else {
                            cell_w + LIBRARY_COLUMN_GAP as usize
                        };
                        spans.extend(item_cell_spans(title, year_str, selected, fg, pad_to));
                    }
                    ListItem::new(Line::from(spans))
                }
            }
        })
        .collect();

    // The full row structure (parallel to the display rows, empty entries
    // for headers), local to this paint call: column-aware cursor movement
    // and mouse hit-testing within it resolve cells from this below.
    let item_rows = plan.item_rows();

    let mut state = ListState::default();
    state.select(Some(display_cursor.saturating_sub(offset)));
    f.render_stateful_widget(
        List::new(list_items).highlight_style(Style::default()),
        content_area,
        &mut state,
    );

    if show_scrollbar {
        let max_off = total_display.saturating_sub(visible);
        crate::app::render::render_right_scrollbar(
            f,
            content_area,
            max_off,
            offset,
            palette::SCROLLBAR,
        );
    }

    {
        crate::app::render::components::list_rows::draw_column_selection_bleed(
            f,
            content_area,
            cursor,
            &item_rows,
            offset,
        );
    }

    final_offset
}
