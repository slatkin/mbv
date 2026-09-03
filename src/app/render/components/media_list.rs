use super::hero::InlineDisplayRow;
use super::list_rows::{
    focused_or_subtle, item_cell_spans, selected_cell_rect, selection_marker, DisplayRow,
    InlineReplacementPlan, ListRenderCtx, MarkerEdge,
};
use crate::app::components::media_list::{
    InlineLayout, InlineMediaBrowser, MediaListRow, MediaSemanticState, RowGeometry, WideMediaList,
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

/// Resolved paint output for [`render_wide_media_list`]: the flow geometry the
/// painter laid out (callers rebuild their pre-#638 hit maps from it), the
/// selected row's absolute rect within the hit/scroll geometry rect, and the
/// pre-#638 mouse-compat maps the painter used to publish through a `&mut
/// LayoutMain` out-param. The painter persists the resolved scroll offset into
/// `list` itself, so no caller can forget to.
pub(in crate::app) struct MediaListPaint<Target> {
    pub row_geometry: RowGeometry<Target>,
    pub selected_row_rect: Option<Rect>,
    pub left_item_rows: Vec<Vec<usize>>,
    pub left_row_map: Vec<Option<usize>>,
}

/// Paint entry point for the embedded plain `WideMediaList` (design.md D1):
/// a fixed-height, one-column list with no inline-detail replacement flow.
/// Reuses the shared list-row span and scrollbar primitives rather than the
/// `EmbyItem`-typed `render_plain_rows` above (which stays the path for the
/// inline browsers until it is parameterised).
///
/// `paint_area` is the row-flow paint rect: its `x`/`width` span the full panel
/// (so the selected-row background and the flush edge marker reach the panel
/// border), while callers that own a framed rail pass a `paint_area` already
/// inset vertically for their reserved border rows. `content_area` is the
/// hit/scroll geometry rect (inset on both axes); the returned
/// `selected_row_rect` and the caller's hit maps are resolved against it. The
/// title's text indent is applied per row in `wide_media_row`, not by
/// insetting either rect.
///
/// The painter resolves the scroll offset and stores it back into `list` via
/// [`WideMediaList::set_scroll`] before returning, so the offset persists across
/// frames without the caller threading a `usize` back.
pub(in crate::app) fn render_wide_media_list<Target: Clone>(
    f: &mut Frame,
    paint_area: Rect,
    content_area: Rect,
    list: &mut WideMediaList<Target>,
    focused: bool,
    selected_bg: Color,
) -> MediaListPaint<Target> {
    let geometry = list.row_geometry(content_area.height as usize);
    let rows = list.rows();
    let selected_row = geometry.selected_row();
    let offset = geometry.offset();
    let total_rows = geometry.len();
    let left_item_rows: Vec<Vec<usize>> = (0..total_rows)
        .filter_map(|row| {
            geometry.source_row(row).and_then(|source_row| {
                matches!(rows[source_row], MediaListRow::Item { .. }).then_some(vec![source_row])
            })
        })
        .collect();
    // Pre-#638 mouse compatibility map (kept wired, not rebuilt): read the
    // painter's own `RowGeometry` and map each painted display row to the
    // control's selectable index for that item, with letter headings and
    // spacers left `None`. Walking `RowGeometry::targets` keeps this in step
    // with the painted flow; the previous projection of source-row indices
    // mis-targeted by the count of preceding non-item rows every row that
    // followed a letter heading or spacer.
    let selectable_by_flow_row: Vec<Option<usize>> = {
        let mut next_selectable = 0usize;
        geometry
            .targets()
            .map(|target| {
                target.map(|_| {
                    let index = next_selectable;
                    next_selectable += 1;
                    index
                })
            })
            .collect()
    };
    let left_row_map: Vec<Option<usize>> = selectable_by_flow_row
        .into_iter()
        .skip(offset)
        .take(paint_area.height as usize)
        .collect();

    let overflows = total_rows > paint_area.height as usize;
    let scrollbar = focused && overflows;
    let inner_width = paint_area.width.saturating_sub(u16::from(scrollbar)) as usize;
    let list_items: Vec<ListItem> = (offset..total_rows)
        .take(paint_area.height as usize)
        .map(|row| {
            let source_row = geometry
                .source_row(row)
                .expect("wide geometry contains a source row");
            wide_media_row(
                &rows[source_row],
                Some(row) == selected_row,
                focused,
                selected_bg,
                inner_width,
            )
        })
        .collect();
    // Row backgrounds own the full paint-rect width (legacy `selection_bg_full`
    // parity); `List` fills each row's style across the whole row area.
    f.render_widget(List::new(list_items), paint_area);

    if scrollbar {
        crate::app::render::render_right_scrollbar(
            f,
            paint_area,
            total_rows.saturating_sub(paint_area.height as usize),
            offset,
            palette::SCROLLBAR,
        );
    }

    let selected_row_rect = geometry.selected_row_rect(content_area);
    list.set_scroll(offset);
    MediaListPaint {
        row_geometry: geometry,
        selected_row_rect,
        left_item_rows,
        left_row_map,
    }
}

/// Resolved paint output for [`render_inline_media_browser`]: the exact flow
/// geometry used for painting and compatibility hit maps, plus the screen rect
/// of the admitted detail block (the caller paints the hero into it), or `None`
/// when the block did not fit and the ordinary selected row was painted.
pub(in crate::app) struct InlinePaintResult<Target> {
    pub row_geometry: crate::app::components::media_list::RowGeometry<Target>,
    pub hero_area: Option<Rect>,
}

/// Paint entry point for the embedded plain `InlineMediaBrowser` (design.md
/// D1): the one-column `render_wide_media_list` flow plus selected-row
/// replacement. The component owns the fit admission, fallback, and geometry
/// (`InlineMediaBrowser::resolve_inline_layout`); this function paints the
/// ordinary rows around the reserved detail block, reusing the shared
/// `wide_media_row` primitive and `hero::inline_display_row` mapping.
///
pub(in crate::app) fn render_inline_media_browser<Target: Clone>(
    f: &mut Frame,
    area: Rect,
    list: &InlineMediaBrowser<Target>,
    desired_detail_rows: usize,
    focused: bool,
    selected_bg: Color,
) -> InlinePaintResult<Target> {
    let layout: InlineLayout<Target> =
        list.resolve_inline_layout(area.height as usize, desired_detail_rows);
    let geometry = layout.row_geometry;
    let rows = list.rows();
    let offset = geometry.offset();
    let total_rows = geometry.len();
    let selected_row = geometry.selected_row();

    let overflows = total_rows > area.height as usize;
    let inner_width = area.width.saturating_sub(u16::from(focused && overflows)) as usize;
    let window = (offset..total_rows).take(area.height as usize);
    let list_items: Vec<ListItem> = window
        .map(|display_row| {
            geometry
                .source_row(display_row)
                .map(|source_row| {
                    wide_media_row(
                        &rows[source_row],
                        Some(display_row) == selected_row && layout.detail_rows == 0,
                        focused,
                        selected_bg,
                        inner_width,
                    )
                })
                .unwrap_or_else(|| ListItem::new(Line::default()))
        })
        .collect();
    f.render_widget(List::new(list_items), area);

    if focused && overflows {
        crate::app::render::render_right_scrollbar(
            f,
            area,
            total_rows.saturating_sub(area.height as usize),
            offset,
            palette::SCROLLBAR,
        );
    }

    let hero_area = (layout.detail_rows > 0)
        .then(|| geometry.selected_row_rect(area))
        .flatten()
        .map(|selected| Rect {
            height: layout.detail_rows as u16,
            ..selected
        });
    InlinePaintResult {
        row_geometry: geometry,
        hero_area,
    }
}

/// One painted row of a `WideMediaList`. Semantic state drives the row
/// colour and, for active rows, an appended progress percentage; `primary`
/// is truncated with an ellipsis to fit; `duration` is a distinct
/// right-aligned green element ending at the panel text-flow content edge
/// (`inner_width` already excludes the scrollbar column).
///
/// `selected_bg` is not a free per-caller choice: the focused selected row
/// "punches through" to the surface *containing* the panel that holds the
/// list, so it must be that parent container's background. Every library
/// rail plus Home and Feeds sits inside a resting-surface parent (even while
/// the list panel itself is focus-green), so they pass
/// `palette::list_selected_row_bg()` (`SURFACE_RESTING`). Queue's parent is
/// itself focus-green, so it passes `SURFACE_FOCUSED`.
///
/// Row geometry: the flush edge marker sits at the paint rect's `x` (the
/// panel border) and the title text is indented `LEFT_INSET` (2) columns in
/// — `[marker][1 space][title…]` — so the title lands at column 2 of the
/// panel; the selected row's background fills the whole row via `List`'s
/// row-style fill.
fn wide_media_row<Target>(
    row: &MediaListRow<Target>,
    selected: bool,
    focused: bool,
    selected_bg: Color,
    inner_width: usize,
) -> ListItem<'static> {
    match row {
        MediaListRow::Spacer => ListItem::new(Line::default()),
        MediaListRow::Heading { text } => ListItem::new(Line::from(vec![
            selection_marker(false, MarkerEdge::Left),
            Span::raw(" "),
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
            // Canonical row geometry:
            // `[marker][1 space][title…]  [FOAM trailing]  [green duration]`
            // with the flush marker at the panel edge, the title at column 2,
            // and a quiet gap before the right-aligned duration.
            const LEFT_INSET: usize = 2;
            const QUIET_GAP: usize = 2;
            const RIGHT_INSET: usize = 2;

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

            let content_w = inner_width.saturating_sub(RIGHT_INSET);
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
            // Pad the selected row's spans out to the full row width (up to
            // the scrollbar column) so the highlighted background bar spans
            // the whole panel regardless of whether a duration string is
            // present — never just the width of the row text.
            if selected {
                let used: usize = spans.iter().map(|span| span.content.width()).sum();
                spans.push(Span::raw(" ".repeat(inner_width.saturating_sub(used))));
            }
            ListItem::new(Line::from(spans)).style(if selected {
                Style::default().bg(selected_bg)
            } else {
                Style::default()
            })
        }
    }
}

#[cfg(test)]
mod wide_row_regression_tests {
    use super::*;
    use crate::app::components::media_list::{MediaListRow, MediaSemanticState, WideMediaList};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn item(target: &str, primary: &str, duration: Option<String>) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: primary.into(),
            trailing: None,
            duration,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    fn paint(
        list: &mut WideMediaList<String>,
        rect: Rect,
        selected_bg: Color,
    ) -> super::MediaListPaint<String> {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let mut captured = None;
        terminal
            .draw(|f| {
                captured = Some(render_wide_media_list(
                    f,
                    rect,
                    rect,
                    list,
                    true,
                    selected_bg,
                ));
            })
            .unwrap();
        captured.unwrap()
    }

    /// migrate-home-feeds 4.6: the selected row's highlight bar must span the
    /// whole panel width (never just the row text, with or without a duration
    /// string), the edge marker must sit flush at the panel's `x`, and the
    /// title must land at column 2. These broke together when the painter was
    /// handed an already-inset content rect.
    #[test]
    fn selected_row_spans_full_width_with_flush_marker_and_three_col_indent() {
        const PX: u16 = 10;
        const PW: u16 = 40;
        let selected_bg = palette::SURFACE_RESTING;

        for duration in [None, Some("1:05".to_string())] {
            let mut list: WideMediaList<String> = WideMediaList::new();
            list.set_content(vec![
                item("sel", "Selected Entry", duration.clone()),
                item("other", "Other Entry", None),
            ]);

            let mut terminal = Terminal::new(TestBackend::new(60, 6)).unwrap();
            terminal
                .draw(|f| {
                    render_wide_media_list(
                        f,
                        Rect::new(PX, 0, PW, 4),
                        Rect::new(PX, 0, PW, 4),
                        &mut list,
                        true,
                        selected_bg,
                    );
                })
                .unwrap();
            let buf = terminal.backend().buffer();

            assert_eq!(
                buf[(PX, 0)].symbol(),
                "▎",
                "edge marker must be flush at the panel x (duration={duration:?})"
            );
            // Skip the flush marker glyph itself; the title is the next
            // non-blank cell.
            let first_text = (PX + 1..PX + PW)
                .find(|&x| buf[(x, 0)].symbol().trim() != "")
                .map(|x| x - PX);
            assert_eq!(
                first_text,
                Some(2),
                "title text indent must be 2 columns (duration={duration:?})"
            );
            for x in PX..PX + PW {
                assert_eq!(
                    buf[(x, 0)].bg,
                    selected_bg,
                    "selected-row bar must fill column {x} (duration={duration:?})"
                );
            }
            assert_ne!(
                buf[(PX, 1)].bg,
                selected_bg,
                "only the selected row is filled (duration={duration:?})"
            );
        }
    }

    /// Step 4 latent bug: the painter must persist the resolved scroll offset
    /// back into `list` so it survives across frames. Home discarded the old
    /// `usize` return, so its rail always re-scrolled to the top.
    #[test]
    fn painter_persists_resolved_scroll_offset_across_frames() {
        let rect = Rect::new(0, 0, 40, 4);
        let selected_bg = palette::SURFACE_RESTING;
        let mut list: WideMediaList<String> = WideMediaList::new();
        list.set_content(
            (0..12)
                .map(|i| item(&format!("t{i}"), &format!("Entry {i}"), None))
                .collect(),
        );
        list.select_last();

        let first = paint(&mut list, rect, selected_bg);
        let resolved = first.row_geometry.offset();
        assert!(resolved > 0, "a bottom selection must scroll the viewport");
        assert_eq!(
            list.scroll(),
            resolved,
            "painter stores the offset it resolved"
        );

        // Re-render with no further input: the stored offset is reused, not reset.
        let second = paint(&mut list, rect, selected_bg);
        assert_eq!(second.row_geometry.offset(), resolved);
        assert_eq!(list.scroll(), resolved);
    }
}
