use super::row::media_list_row;
use crate::app::components::media_list::{
    RowGeometry, SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::*;
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
/// against it. The title's text indent is applied per row in `media_list_row`,
/// not by insetting either rect.
///
/// The painter resolves the scroll offset and stores it back into `list` via
/// [`WideMediaList::set_scroll`] before returning, so the offset persists across
/// frames without the caller threading a `usize` back.
#[cfg(test)]
pub(super) fn render_wide_media_list<Target: Clone + PartialEq>(
    f: &mut Frame,
    paint_area: Rect,
    content_area: Rect,
    list: &mut WideMediaList<Target>,
    focused: bool,
    selected_bg: Color,
) -> MediaListPaint<Target> {
    render_wide_media_list_with_zebra(
        f,
        paint_area,
        content_area,
        list,
        focused,
        selected_bg,
        None,
        false,
    )
}

pub(super) fn render_wide_media_list_with_zebra<Target: Clone + PartialEq>(
    f: &mut Frame,
    paint_area: Rect,
    content_area: Rect,
    list: &mut WideMediaList<Target>,
    focused: bool,
    selected_bg: Color,
    zebra_bg: Option<Color>,
    gutter_accent: bool,
) -> MediaListPaint<Target> {
    let geometry = list.row_geometry(content_area.height as usize);
    let selected_row = geometry.selected_row();
    let marquee_primary = focused
        .then_some(selected_row)
        .flatten()
        .and_then(|row| list.rows().get(row))
        .and_then(|row| match row {
            crate::app::components::media_list::MediaListRow::Item { primary, .. } => {
                Some(primary.clone())
            }
            _ => None,
        });
    let mut marquee = marquee_primary.map(|primary| list.marquee_state(&primary));
    let rows = list.rows();
    let offset = geometry.offset();
    let total_rows = geometry.len();

    let overflows = total_rows > content_area.height as usize;
    let scrollbar = focused && overflows;
    let inner_width = paint_area.width.saturating_sub(u16::from(scrollbar)) as usize;
    let list_items: Vec<ListItem> = (offset..total_rows)
        .take(content_area.height as usize)
        .enumerate()
        .map(|(visible_row, row)| {
            let source_row = geometry
                .source_row(row)
                .expect("wide geometry contains a source row");
            let row_target = rows[source_row].selectable_target();
            let multi_selected = row_target.is_some_and(|target| list.is_selected_target(target));
            // The stripe runs continuously down the visible window: group
            // headings and spacers take their place in the alternation like
            // any other row, so a group does not restart the sequence. The
            // sequence opens on the primary fill.
            let alternate_bg = zebra_bg.filter(|_| visible_row % 2 == 1);
            let item = media_list_row(
                &rows[source_row],
                Some(row) == selected_row || multi_selected,
                focused || multi_selected,
                selected_bg,
                alternate_bg,
                gutter_accent,
                inner_width,
                scrollbar,
                marquee
                    .as_mut()
                    .filter(|_| Some(row) == selected_row)
                    .map(|(text, started_at)| (text, started_at)),
            );
            item
        })
        .collect();
    // Keep the established full-width row treatment, but take the vertical
    // flow from the content rectangle. A framed parent may claim a wider
    // panel than its padded row flow; it must not move the painted rows away
    // from the retained row geometry.
    let row_paint_area = row_paint_area(paint_area, content_area);
    f.render_widget(List::new(list_items), row_paint_area);

    if scrollbar {
        let scrollbar_x = if row_paint_area.right() < f.area().right() {
            row_paint_area.right()
        } else {
            row_paint_area.x + row_paint_area.width.saturating_sub(1)
        };
        f.render_widget(
            Block::default().style(Style::default().bg(selected_bg)),
            Rect {
                x: scrollbar_x,
                width: 1,
                ..row_paint_area
            },
        );
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

fn row_paint_area(paint_area: Rect, content_area: Rect) -> Rect {
    Rect {
        y: content_area.y,
        height: content_area.height,
        ..paint_area
    }
}

fn selected_row_surface_color(surface: SelectedRowSurface, focused: bool) -> Color {
    let surface = match surface {
        SelectedRowSurface::ListBackdrop => palette::Surface::SelectedRow,
        SelectedRowSurface::OwningQueueColumn => palette::Surface::SelectedRowOnQueueColumn,
        SelectedRowSurface::OwningLibraryPane => palette::Surface::SelectedRowOnLibraryPane,
    };
    palette::surface_colors(surface, focused).fill
}

/// Component-view adapter for the retained-result seam.
pub(in crate::app) fn render_wide_media_list_component<Target: Clone + PartialEq>(
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
    let paint = render_wide_media_list_with_zebra(
        f,
        claim_rect,
        content_rect,
        list,
        policy.focused(),
        selected_row_surface_color(policy.selected_surface(), policy.focused()),
        policy.zebra_bg(),
        // Wide selection is always the gutter accent; `selected_bg` now only
        // resolves the scrollbar backing for focused overflowing lists.
        true,
    );
    list.finish_view(
        claim_rect,
        content_rect,
        paint.row_geometry,
        paint.selected_row_rect,
    );
}
