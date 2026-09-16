use super::row::media_list_row;
use crate::app::components::media_list::{
    MediaListRow, RowGeometry, SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
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
/// bar reaches the panel border); its vertical span
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
            MediaListRow::Item { primary, .. } => Some(primary.clone()),
            _ => None,
        });
    let mut marquee = marquee_primary.map(|primary| list.marquee_state(&primary));
    let rows = list.rows();
    let offset = geometry.offset();
    let total_rows = geometry.len();

    // A grouped list fills its content rows with the secondary colour under its
    // surface-coloured labels, and alternates within each group: the group's
    // first member carries the secondary fill and every second member after it
    // takes the surface fill instead. An ungrouped list alternates throughout,
    // opening on the primary fill.
    let grouped = rows
        .iter()
        .any(|row| matches!(row, MediaListRow::Heading { .. }));

    let overflows = total_rows > content_area.height as usize;
    let scrollbar = focused && overflows;
    let inner_width = paint_area.width.saturating_sub(u16::from(scrollbar)) as usize;
    let mut striped = offset % 2 == 1;
    let list_items: Vec<ListItem> = (offset..total_rows)
        .take(content_area.height as usize)
        .map(|source_row| {
            let row_target = rows[source_row].selectable_target();
            let multi_selected = row_target.is_some_and(|target| list.is_selected_target(target));
            let alternate_bg = match rows[source_row] {
                // A group's header is its surface-coloured label, and the
                // separator above it sits outside the fill too: both keep the
                // fill the list box already painted.
                MediaListRow::Heading { .. } | MediaListRow::Spacer => None,
                _ if grouped => zebra_bg.filter(|_| grouped_member_striped(rows, source_row)),
                _ => {
                    let alternate_bg = zebra_bg.filter(|_| striped);
                    striped = !striped;
                    alternate_bg
                }
            };
            media_list_row(
                &rows[source_row],
                Some(source_row) == selected_row || multi_selected,
                focused || multi_selected,
                selected_bg,
                alternate_bg,
                gutter_accent,
                inner_width,
                scrollbar,
                marquee
                    .as_mut()
                    .filter(|_| Some(source_row) == selected_row)
                    .map(|(text, started_at)| (text, started_at)),
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
        // The scrollbar column keeps the row's own background: the selected
        // row's item style already fills it, and every other row keeps the
        // panel fill. Painting the column separately would smear the selected
        // row's bar down the whole list.
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

/// Whether one content row of a grouped list takes the secondary zebra fill
/// rather than the surface fill: a group's members alternate from the secondary
/// fill at their first member, so every second member reverts to the surface
/// fill. `row` is a content row's source index; the grouping rows are never
/// asked.
fn grouped_member_striped<Target>(rows: &[MediaListRow<Target>], row: usize) -> bool {
    // The group's members start at the row below its header (the list's own
    // first row when it has none above the row).
    let start = (0..=row)
        .rev()
        .find(|&index| matches!(rows[index], MediaListRow::Heading { .. }))
        .map_or(0, |header| header + 1);
    (row - start).is_multiple_of(2)
}

fn selected_row_surface_color(_surface: SelectedRowSurface, _focused: bool) -> Color {
    // Audition: every selected row paints the opaque bar (and the scrollbar
    // column behind it), so the surface table's punch-through resolution is
    // deliberately bypassed.
    palette::SELECTED_ROW_BG
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
        // Retained for the Wide callers; the bar is painted regardless, so this
        // no longer changes appearance.
        true,
    );
    list.finish_view(
        claim_rect,
        content_rect,
        paint.row_geometry,
        paint.selected_row_rect,
    );
}
