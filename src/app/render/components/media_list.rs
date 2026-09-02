use super::hero::{inline_display_row, InlineDisplayRow};
use super::list_rows::{
    focused_or_subtle, item_cell_spans, selected_cell_rect, selection_marker, DisplayRow,
    InlineReplacementPlan, ListRenderCtx, MarkerEdge,
};
use crate::app::components::media_list::{
    InlineLayout, InlineMediaBrowser, MediaListRow, MediaSemanticState, WideMediaList,
};
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::palette;
use crate::app::ui_util::*;
use ratatui::layout::Rect;
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
pub(in crate::app) fn render_plain_rows(
    f: &mut Frame,
    ctx: ListRenderCtx,
    layout: &mut LayoutMain,
) -> usize {
    let ListRenderCtx {
        content_area,
        items,
        cursor,
        stored_scroll,
        cols,
        focused,
        hero_rows,
    } = ctx;
    let n = items.len();
    // Publish the identity display order just like letter-grouped lists publish
    // their sorted order; the fresh frame layout makes this authoritative for
    // cursor and hit testing even when the list has no grouping.
    layout.left_sorted_indices = (0..n).collect();
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
    let plan = InlineReplacementPlan::new(
        &display_rows,
        display_cursor,
        cursor,
        hero_rows,
        content_area.height,
        stored_scroll,
    );
    let offset = plan.offset();
    let detail_rows = plan.detail_rows();
    let total_display = plan.total_display_rows();
    let final_offset = offset;

    let show_scrollbar = focused && total_display > visible;

    let list_items: Vec<ListItem> = (offset..total_display)
        .take(visible)
        .map(|display_row| {
            match plan
                .display_row(display_row)
                .expect("display row is within the replacement flow")
            {
                InlineDisplayRow::Replacement => ListItem::new(Line::default()),
                InlineDisplayRow::Source(source_row) => match &display_rows[source_row] {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) => {
                        ListItem::new(Line::default())
                    }
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
                            let (item_name, dur_str) = if item.is_folder {
                                let name = if item.item_type == "Folder" && item.total_count > 0 {
                                    format!(
                                        "{} \u{b7} {} items",
                                        item.display_name(),
                                        item.total_count
                                    )
                                } else if item.unplayed_item_count > 0 && item.item_type != "Series"
                                {
                                    format!(
                                        "{} [{}]",
                                        item.display_name(),
                                        item.unplayed_item_count
                                    )
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
                            let name_w = avail.saturating_sub(dur_str.width());
                            let (title, dur_str) = if selected && detail_rows > 0 {
                                (String::new(), String::new())
                            } else {
                                (trunc_str(&item_name, name_w), dur_str)
                            };
                            let fg = focused_or_subtle(focused);

                            let pad_to = if cell_idx + 1 == idxs.len() {
                                cell_w
                            } else {
                                cell_w + LIBRARY_COLUMN_GAP as usize
                            };
                            spans.extend(item_cell_spans(title, dur_str, selected, fg, pad_to));
                        }
                        ListItem::new(Line::from(spans))
                    }
                },
            }
        })
        .collect();

    let row_targets = plan.row_targets();
    layout.left_row_map = (offset..total_display)
        .take(visible)
        .enumerate()
        .map(|(visible_row, _)| row_targets[offset + visible_row])
        .collect();
    // Publish the full row structure (parallel to the display rows,
    // empty entries for headers) so column-aware cursor movement and
    // mouse hit-testing can resolve cells between frames.
    layout.left_item_rows = plan.item_rows();

    if let Some(hero_area) = plan.hero_area(content_area) {
        layout.hero_area = hero_area;
        layout.inline_hero_area = layout.hero_area;
    }

    let mut state = ListState::default();
    state.select(Some(display_cursor.saturating_sub(offset)));
    layout.selected_item_rect = selected_cell_rect(
        content_area,
        cursor,
        &layout.left_item_rows,
        offset,
        cols,
        cell_w as u16,
        LIBRARY_COLUMN_GAP,
    );
    if detail_rows > 0 {
        layout.selected_item_rect = Some(layout.hero_area);
    }
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

    if plan.should_draw_selection_markers() {
        super::list_rows::draw_column_selection_markers(
            f,
            content_area,
            cursor,
            &layout.left_item_rows,
            offset,
        );
    }

    final_offset
}

/// Paint entry point for the embedded plain `WideMediaList` (design.md D1):
/// a fixed-height, one-column list with no inline-detail replacement flow.
/// Reuses the shared list-row span and scrollbar primitives rather than the
/// `EmbyItem`-typed `render_plain_rows` above (which stays the path for the
/// inline browsers until it is parameterised). Returns the resolved scroll
/// offset for the caller to store via `WideMediaList::set_scroll`.
pub(in crate::app) fn render_wide_media_list<Target>(
    f: &mut Frame,
    area: Rect,
    list: &WideMediaList<Target>,
    focused: bool,
    selected_bg: Color,
    layout: &mut LayoutMain,
) -> usize {
    let viewport = list.resolve_viewport(area.height as usize);
    let rows = list.rows();
    let selected_row = list.selected_display_row();
    layout.left_item_rows = rows
        .iter()
        .enumerate()
        .filter_map(|(row, item)| match item {
            MediaListRow::Item { .. } => Some(vec![row]),
            _ => None,
        })
        .collect();
    layout.left_row_map = (viewport.offset..viewport.total_rows)
        .take(viewport.height)
        .map(|row| matches!(rows[row], MediaListRow::Item { .. }).then_some(row))
        .collect();

    let inner_width = area
        .width
        .saturating_sub(u16::from(focused && viewport.overflows())) as usize;
    let list_items: Vec<ListItem> = (viewport.offset..viewport.total_rows)
        .take(viewport.height)
        .map(|row| {
            wide_media_row(
                &rows[row],
                Some(row) == selected_row,
                focused,
                selected_bg,
                inner_width,
                focused && viewport.overflows(),
            )
        })
        .collect();
    f.render_widget(List::new(list_items), area);

    if focused && viewport.overflows() {
        crate::app::render::render_right_scrollbar(
            f,
            area,
            viewport.max_offset(),
            viewport.offset,
            palette::SCROLLBAR,
        );
    }

    viewport.offset
}

/// Resolved paint output for [`render_inline_media_browser`]: the offset the
/// caller stores via `InlineMediaBrowser::set_scroll`, and the screen rect of
/// the admitted detail block (the caller paints the hero into it), or `None`
/// when the block did not fit and the ordinary selected row was painted.
pub(in crate::app) struct InlinePaintResult {
    pub offset: usize,
    pub hero_area: Option<Rect>,
}

/// Paint entry point for the embedded plain `InlineMediaBrowser` (design.md
/// D1): the one-column `render_wide_media_list` flow plus selected-row
/// replacement. The component owns the fit admission, fallback, and geometry
/// (`InlineMediaBrowser::resolve_inline_layout`); this function paints the
/// ordinary rows around the reserved detail block, reusing the shared
/// `wide_media_row` primitive and `hero::inline_display_row` mapping.
///
pub(in crate::app) fn render_inline_media_browser<Target>(
    f: &mut Frame,
    area: Rect,
    list: &InlineMediaBrowser<Target>,
    desired_detail_rows: usize,
    focused: bool,
    selected_bg: Color,
) -> InlinePaintResult {
    let layout: InlineLayout =
        list.resolve_inline_layout(area.height as usize, desired_detail_rows);
    let rows = list.rows();
    let source_rows = rows.len();
    let selected_row = list.selected_display_row();

    let inner_width = area.width.saturating_sub(u16::from(
        focused && layout.total_display_rows > layout.height,
    )) as usize;
    let window = (layout.offset..layout.total_display_rows).take(layout.height);
    let list_items: Vec<ListItem> = if layout.detail_rows == 0 {
        window
            .map(|row| {
                wide_media_row(
                    &rows[row],
                    Some(row) == selected_row,
                    focused,
                    selected_bg,
                    inner_width,
                    focused && layout.total_display_rows > layout.height,
                )
            })
            .collect()
    } else {
        let sel = selected_row.expect("an admitted detail block requires a selection");
        window
            .map(|display_row| {
                match inline_display_row(source_rows, sel, layout.detail_rows as u16, display_row) {
                    Some(InlineDisplayRow::Source(source_row)) => wide_media_row(
                        &rows[source_row],
                        false,
                        focused,
                        selected_bg,
                        inner_width,
                        focused && layout.total_display_rows > layout.height,
                    ),
                    Some(InlineDisplayRow::Replacement) | None => ListItem::new(Line::default()),
                }
            })
            .collect()
    };
    f.render_widget(List::new(list_items), area);

    if focused && layout.total_display_rows > layout.height {
        crate::app::render::render_right_scrollbar(
            f,
            area,
            layout.total_display_rows.saturating_sub(layout.height),
            layout.offset,
            palette::SCROLLBAR,
        );
    }

    let hero_area = layout.detail_screen_row.map(|screen_row| Rect {
        y: area.y + screen_row as u16,
        height: layout.detail_rows as u16,
        ..area
    });
    InlinePaintResult {
        offset: layout.offset,
        hero_area,
    }
}

/// One painted row of a `WideMediaList`. Semantic state drives the row
/// colour and, for active rows, an appended progress percentage; `primary`
/// is truncated with an ellipsis to fit; `duration` is a distinct
/// right-aligned green element ending two columns before the content edge
/// (`inner_width` already excludes the scrollbar column). `selected_bg` is
/// the focused-panel's parent surface — the colour the selected row takes so
/// it reads against the panel body (Queue: `SURFACE_FOCUSED`; hero-on-left
/// rails: `SURFACE_RESTING`, matching the legacy painters they replace).
fn wide_media_row<Target>(
    row: &MediaListRow<Target>,
    selected: bool,
    focused: bool,
    selected_bg: Color,
    inner_width: usize,
    has_scrollbar: bool,
) -> ListItem<'static> {
    match row {
        MediaListRow::Spacer => ListItem::new(Line::default()),
        MediaListRow::Heading { text } => ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                text.clone(),
                Style::default()
                    .fg(palette::TEXT_FOCUS_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        MediaListRow::Item {
            primary,
            trailing,
            duration,
            semantic_state,
            ..
        } => {
            // Canonical row geometry (legacy `render_queue_content`,
            // de4a079c): `[marker][space][title…]  [FOAM trailing]  [green
            // duration]` with a 2-col inset on the left (marker + space) and
            // the right, and a quiet gap before the right-aligned duration.
            const LEFT_INSET: usize = 2;
            const QUIET_GAP: usize = 2;
            let right_inset = if focused && has_scrollbar { 1 } else { 2 };

            let (fg, progress) = match semantic_state {
                MediaSemanticState::Ordinary => (palette::TEXT_EMPHASIS, None),
                MediaSemanticState::Played => (palette::TEXT_MUTED, None),
                MediaSemanticState::Active { progress } => (
                    palette::TEXT_FOCUS_ACCENT,
                    (*progress).map(|value| format!("{}%", value.percent())),
                ),
                MediaSemanticState::Disabled => (palette::TEXT_MUTED, None),
            };
            let trailing = match (
                trailing.as_deref().filter(|text| !text.is_empty()),
                progress,
            ) {
                (Some(text), Some(pct)) => format!("{text} {pct}"),
                (Some(text), None) => text.to_owned(),
                (None, Some(pct)) => pct,
                (None, None) => String::new(),
            };
            let duration = duration.as_deref().filter(|dur| !dur.is_empty());

            let content_w = inner_width.saturating_sub(right_inset);
            let trailing_w = if trailing.is_empty() {
                0
            } else {
                1 + trailing.width()
            };
            let dur_reserve = duration.map_or(0, |dur| QUIET_GAP + dur.width());
            let title = trunc_str(
                primary,
                content_w.saturating_sub(LEFT_INSET + trailing_w + dur_reserve),
            );

            let selected = selected && focused;
            let mut spans = vec![selection_marker(selected, MarkerEdge::Left), Span::raw(" ")];
            spans.push(Span::styled(
                title,
                Style::default().fg(
                    if selected && !matches!(semantic_state, MediaSemanticState::Active { .. }) {
                        palette::TEXT_EMPHASIS
                    } else {
                        fg
                    },
                ),
            ));
            if !trailing.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    trailing,
                    Style::default().fg(palette::TEXT_METADATA),
                ));
            }
            if let Some(dur) = duration {
                let used: usize = spans.iter().map(|span| span.content.width()).sum();
                let pad = content_w.saturating_sub(used + dur.width());
                spans.push(Span::raw(" ".repeat(pad)));
                spans.push(Span::styled(
                    dur.to_owned(),
                    Style::default().fg(palette::STATUS_AVAILABLE),
                ));
            }
            ListItem::new(Line::from(spans)).style(if selected {
                Style::default().bg(selected_bg)
            } else {
                Style::default()
            })
        }
    }
}
