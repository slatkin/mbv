use super::row::media_list_row;
use crate::app::render::components::media_list::{
    MediaListRow, RowGeometry, SelectedRowSurface, WideMediaListPaintPolicy,
};
use mbv_theme as palette;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

/// Resolved paint output for [`render_wide_media_list`]: the flow geometry the
/// painter laid out and the selected row's absolute rect within the hit/scroll
/// geometry rect. The painter persists the resolved scroll offset into `list`
/// itself, so no caller can forget to. This is internal to the media-list
/// paint subsystem (design.md D6/D7): destinations receive rows only through
/// the retained `Component::view` facts.
pub(crate) struct MediaListPaint<Target> {
    pub(crate) row_geometry: RowGeometry<Target>,
    pub(crate) selected_row_rect: Option<Rect>,
}

/// Paint entry point for the embedded plain `WideMediaList` (design.md D1):
/// a fixed-height, one-column list with no inline-detail replacement flow.
/// Reuses the shared palette/theme roles rather than any `EmbyItem`-typed
/// legacy row painter.
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
/// The component resolves row geometry and persists the returned scroll offset.
pub(super) fn render_wide_media_list_with_zebra<Target: Clone + Eq>(
    f: &mut Frame,
    paint_area: Rect,
    content_area: Rect,
    input: WideMediaListPaintInput<'_, Target>,
    focused: bool,
    selected_bg: Color,
    zebra_bg: Option<Color>,
) -> MediaListPaint<Target> {
    let geometry = input.row_geometry;
    let selected_row = geometry.selected_row();
    // The marquee clock keys on the full title text the row marquees (a split
    // row's context text and item title), so two rows sharing a context name
    // never share a clock position. `row_marquee_key` is the one formula the
    // painter receives its key from too.
    let title_reveal = input.title_reveal;
    let mut marquee = input.marquee;
    let rows = input.rows;
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
            let multi_selected = row_target.is_some_and(|target| {
                input
                    .multi_selection
                    .iter()
                    .any(|selected| selected == target)
            });
            let alternate_bg = match rows[source_row] {
                // A group's header is its surface-coloured label, and the
                // separator above it sits outside the fill too: both keep the
                // fill the list box already painted.
                MediaListRow::Heading { .. } | MediaListRow::Spacer => None,
                MediaListRow::Item { .. } if grouped => {
                    zebra_bg.filter(|_| grouped_member_striped(rows, source_row))
                }
                MediaListRow::Item { .. } => {
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
                inner_width,
                scrollbar,
                title_reveal,
                marquee
                    .as_mut()
                    .filter(|_| Some(source_row) == selected_row)
                    .map(|(key, text, started_at)| (*key, &mut **text, &mut **started_at)),
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
    // Every selected row paints the same opaque Iris bar, regardless of the
    // owning tab or whether the selected row is a focused cursor or a
    // multi-selected row.
    palette::SELECTED_ROW_BG
}

/// Borrowed data needed to paint a wide media list.
pub(crate) struct WideMediaListPaintInput<'a, Target> {
    pub(crate) rows: &'a [MediaListRow<Target>],
    pub(crate) row_geometry: RowGeometry<Target>,
    pub(crate) multi_selection: &'a [Target],
    pub(crate) title_reveal: super::MediaListTitleReveal,
    pub(crate) marquee: Option<(&'a str, &'a mut String, &'a mut std::time::Instant)>,
    pub(crate) policy: WideMediaListPaintPolicy,
    pub(crate) claim_rect: Rect,
    pub(crate) content_rect: Rect,
}

/// Component-view adapter for the retained-result seam.
pub(in crate::app) fn render_wide_media_list_component<Target: Clone + Eq>(
    f: &mut Frame,
    input: WideMediaListPaintInput<'_, Target>,
) -> MediaListPaint<Target> {
    let policy = input.policy;
    render_wide_media_list_with_zebra(
        f,
        input.claim_rect,
        input.content_rect,
        input,
        policy.focused(),
        selected_row_surface_color(policy.selected_surface(), policy.focused()),
        policy.zebra_bg(),
    )
}
