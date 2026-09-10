use super::wide_row::wide_media_row;
use crate::app::components::media_list::{
    GridMediaList, GridPaintPolicy, InlineLayout, InlineMediaBrowser,
    InlineMediaBrowserPaintPolicy, RowGeometry, SelectedRowSurface, WideMediaList,
    WideMediaListPaintPolicy,
};
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Resolved paint output for [`render_wide_media_list`]: the flow geometry the
/// painter laid out and the selected row's absolute rect within the hit/scroll
/// geometry rect. The painter persists the resolved scroll offset into `list`
/// itself, so no caller can forget to. This is internal to the media-list
/// paint subsystem (design.md D6/D7): destinations receive rows only through
/// the retained `Component::view` facts.
pub(super) struct MediaListPaint<Target> {
    pub row_geometry: RowGeometry<Target>,
    pub selected_row_rect: Option<Rect>,
}

/// Paint entry point for the embedded plain `WideMediaList` (design.md D1):
/// a fixed-height, one-column list with no inline-detail replacement flow.
/// Reuses the shared list-row span and scrollbar primitives rather than the
/// `EmbyItem`-typed `render_plain_rows` in `plain_rows` (which stays the path
/// for the inline browsers until it is parameterised).
///
/// `paint_area` supplies the full-width visual span (so the selected-row
/// background and flush edge marker reach the panel border); its vertical span
/// is replaced with `content_area`'s row-flow span. This keeps framed parents'
/// full-width selection treatment while aligning painted rows with retained
/// geometry. `content_area` remains the hit/scroll geometry rect (inset on
/// both axes); the returned `selected_row_rect` and caller hit maps resolve
/// against it. The title's text indent is applied per row in `wide_media_row`,
/// not by insetting either rect.
///
/// The painter resolves the scroll offset and stores it back into `list` via
/// [`WideMediaList::set_scroll`] before returning, so the offset persists across
/// frames without the caller threading a `usize` back.
pub(super) fn render_wide_media_list<Target: Clone>(
    f: &mut Frame,
    paint_area: Rect,
    content_area: Rect,
    list: &mut WideMediaList<Target>,
    focused: bool,
    selected_bg: Color,
    throbber: Option<char>,
) -> MediaListPaint<Target> {
    #[cfg(test)]
    super::WIDE_MEDIA_LIST_PAINTS.with(|count| count.set(count.get() + 1));
    let geometry = list.row_geometry(content_area.height as usize);
    let rows = list.rows();
    let selected_row = geometry.selected_row();
    let offset = geometry.offset();
    let total_rows = geometry.len();

    let overflows = total_rows > content_area.height as usize;
    let scrollbar = focused && overflows;
    let inner_width = paint_area.width.saturating_sub(u16::from(scrollbar)) as usize;
    let list_items: Vec<ListItem> = (offset..total_rows)
        .take(content_area.height as usize)
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
                scrollbar,
                throbber,
            )
        })
        .collect();
    // Keep the established full-width row treatment, but take the vertical
    // flow from the content rectangle. A framed parent may claim a wider
    // panel than its padded row flow; it must not move the painted rows away
    // from the retained row geometry.
    let row_paint_area = row_paint_area(paint_area, content_area);
    f.render_widget(List::new(list_items), row_paint_area);

    if scrollbar {
        crate::app::render::render_right_scrollbar(
            f,
            row_paint_area,
            total_rows.saturating_sub(content_area.height as usize),
            offset,
            palette::SCROLLBAR,
        );
    }

    let selected_row_rect = geometry.selected_row_rect(content_area);
    list.set_scroll(offset);
    MediaListPaint {
        row_geometry: geometry,
        selected_row_rect,
    }
}

/// Resolved paint output for [`render_inline_media_browser`]: the exact flow
/// geometry used for painting and compatibility hit maps, plus the screen rect
/// of the admitted detail block (the caller paints the hero into it), or `None`
/// when the block did not fit and the ordinary selected row was painted.
pub(super) struct InlinePaintResult<Target> {
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
fn render_inline_media_browser_with_geometry<Target: Clone>(
    f: &mut Frame,
    paint_area: Rect,
    content_area: Rect,
    list: &InlineMediaBrowser<Target>,
    desired_detail_rows: usize,
    focused: bool,
    selected_bg: Color,
) -> InlinePaintResult<Target> {
    #[cfg(test)]
    super::INLINE_MEDIA_BROWSER_PAINTS.with(|count| count.set(count.get() + 1));
    let layout: InlineLayout<Target> =
        list.resolve_inline_layout(content_area.height as usize, desired_detail_rows);
    let geometry = layout.row_geometry;
    let rows = list.rows();
    let offset = geometry.offset();
    let total_rows = geometry.len();
    let selected_row = geometry.selected_row();

    let overflows = total_rows > content_area.height as usize;
    let inner_width = paint_area
        .width
        .saturating_sub(u16::from(focused && overflows)) as usize;
    let window = (offset..total_rows).take(content_area.height as usize);
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
                        focused && overflows,
                        None,
                    )
                })
                .unwrap_or_else(|| ListItem::new(Line::default()))
        })
        .collect();
    // Inline replacement uses the same claim-width/content-flow model as the
    // fixed-row painter: ordinary rows keep the parent's full-width visual
    // treatment while their vertical placement follows the retained flow.
    let row_paint_area = row_paint_area(paint_area, content_area);
    f.render_widget(List::new(list_items), row_paint_area);

    if focused && overflows {
        crate::app::render::render_right_scrollbar(
            f,
            row_paint_area,
            total_rows.saturating_sub(content_area.height as usize),
            offset,
            palette::SCROLLBAR,
        );
    }

    let hero_area = (layout.detail_rows > 0)
        .then(|| geometry.selected_row_rect(content_area))
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

fn row_paint_area(paint_area: Rect, content_area: Rect) -> Rect {
    Rect {
        y: content_area.y,
        height: content_area.height,
        ..paint_area
    }
}

/// The colour a selected row punches through to.
///
/// A selected row paints only while it holds the cursor, which implies its own
/// column holds panel focus, so the punch-through surface's focused fill is
/// always the colour. The cursor never chooses it (design D1/D3): moving the
/// cursor gates *whether* a row paints, not what colour it is.
fn selected_row_surface_color(surface: SelectedRowSurface) -> Color {
    let surface = match surface {
        SelectedRowSurface::ListBackdrop => palette::Surface::SelectedRow,
        SelectedRowSurface::OwningQueueColumn => palette::Surface::SelectedRowOnQueueColumn,
        SelectedRowSurface::OwningLibraryPane => palette::Surface::SelectedRowOnLibraryPane,
    };
    // `true` is this surface's own column focus, proven by the row being
    // selected: the `focused` gating bit that reaches the painter is a cursor
    // bit and must not choose the colour.
    palette::surface_colors_for_column_focus(surface, true).fill
}

/// Component-view adapter for the retained-result seam. The compatibility
/// painter above remains available to destinations that still own its legacy
/// geometry contract.
pub(in crate::app) fn render_wide_media_list_component<Target: Clone>(
    f: &mut Frame,
    area: Rect,
    list: &mut WideMediaList<Target>,
    policy: WideMediaListPaintPolicy,
) {
    list.begin_view();
    let (claim_rect, content_rect) = list.view_geometry(area);
    if area.is_empty() || claim_rect.is_empty() || content_rect.is_empty() || list.is_empty() {
        return;
    }
    let paint = render_wide_media_list(
        f,
        claim_rect,
        content_rect,
        list,
        policy.focused(),
        selected_row_surface_color(policy.selected_surface()),
        policy.throbber(),
    );
    list.finish_view(
        claim_rect,
        content_rect,
        paint.row_geometry,
        paint.selected_row_rect,
    );
}

/// Component-view adapter for the Grid retained-cell seam.
pub(in crate::app) fn render_grid_media_list_component<Target: Clone + PartialEq>(
    f: &mut Frame,
    area: Rect,
    grid: &mut GridMediaList<Target>,
    policy: GridPaintPolicy,
) {
    #[cfg(test)]
    super::GRID_MEDIA_LIST_PAINTS.with(|count| count.set(count.get() + 1));
    grid.begin_view();
    let (claim, content) = grid.geometry(area);
    if area.is_empty() || claim.is_empty() || content.is_empty() || grid.rows().is_empty() {
        return;
    }
    let cells = grid.cells(content);
    let rows = grid.rows();
    let selected = grid.selected_target();
    for (cell, source_row) in &cells {
        let Some(row) = rows.get(*source_row) else {
            continue;
        };
        let selected_row = cell
            .target
            .as_ref()
            .zip(selected)
            .is_some_and(|(a, b)| a == b);
        let item = wide_media_row(
            row,
            selected_row,
            policy.focused(),
            palette::surface_colors_for_column_focus(palette::Surface::SelectedRow, false).fill,
            cell.rect.width as usize,
            false,
            None,
        );
        f.render_widget(List::new(vec![item]), cell.rect);
    }
    let viewport = grid.resolve_viewport(content.height as usize);
    if policy.focused() && viewport.overflows() {
        crate::app::render::render_right_scrollbar(
            f,
            content,
            viewport.total_rows.saturating_sub(viewport.height),
            grid.scroll(),
            palette::SCROLLBAR,
        );
    }
    grid.finish_view(claim, content, cells);
}

/// Component-view adapter for the Inline retained-result seam.
pub(in crate::app) fn render_inline_media_browser_component<Target: Clone>(
    f: &mut Frame,
    area: Rect,
    list: &mut InlineMediaBrowser<Target>,
    policy: InlineMediaBrowserPaintPolicy,
) {
    list.begin_view();
    let (claim_rect, content_rect) = list.view_geometry(area);
    if area.is_empty() || claim_rect.is_empty() || content_rect.is_empty() || list.is_empty() {
        return;
    }
    let paint = render_inline_media_browser_with_geometry(
        f,
        claim_rect,
        content_rect,
        list,
        policy.desired_detail_rows(),
        policy.focused(),
        selected_row_surface_color(policy.selected_surface()),
    );
    let selected_row_rect = paint.row_geometry.selected_row_rect(content_rect);
    list.finish_view(
        claim_rect,
        content_rect,
        paint.row_geometry,
        selected_row_rect,
        paint.hero_area,
    );
}
