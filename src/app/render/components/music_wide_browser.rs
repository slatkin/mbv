use crate::app::components::media_list::WideMediaList;
use crate::app::layout::LayoutMain;
use crate::app::palette;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::components::music_wide::grouped_album_rows;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Paints the wide Music right rail through the canonical `WideMediaList`
/// control, exactly as the wide TV series rail and wide Movies list do
/// (`render_wide_tv_with_ctx`). The grouped artist-header / album / spacer
/// sequence is projected onto `MediaListRow::Heading` / `Item` / `Spacer`;
/// the control enforces that headings and spacers are never selectable.
///
/// `MusicWorkspaceComponent` still owns `album_cursor` / `album_scroll`; this
/// function seeds a fresh control from that local state each frame (stable
/// album id as the target, so an ordinary refresh preserves selection) and
/// returns the resolved scroll offset for the component to persist.
pub(in crate::app) fn render_wide_right_album_browser_with_ctx(
    f: &mut Frame,
    browser_area: Rect,
    panel_area: Rect,
    album_info: &[(String, String, String)],
    order: &[usize],
    list: &LibraryListRenderCtx,
    right_focused: bool,
    layout: &mut LayoutMain,
    media: &mut WideMediaList<String>,
) -> usize {
    layout.wide_music_browser_area = browser_area;
    if list.items.is_empty() {
        media.set_content(Vec::new());
        crate::app::render::render_placeholder(
            f,
            browser_area,
            if list.loading {
                " Loading\u{2026}"
            } else {
                " (empty)"
            },
        );
        return 0;
    }

    media.set_content(grouped_album_rows(&list.items, album_info, order));
    if let Some(selected) = list.items.get(list.cursor) {
        media.select_target(&selected.id);
    }
    media.set_scroll(list.scroll);

    // The canonical rail owns the full panel row (selection markers and
    // selected backgrounds reach the panel border); `browser_area` remains
    // the padded hit/scroll geometry.
    let paint_area = Rect {
        x: panel_area.x,
        width: panel_area.width,
        ..browser_area
    };
    let paint = super::media_list::render_wide_media_list(
        f,
        paint_area,
        browser_area,
        media,
        right_focused,
        palette::list_selected_row_bg(),
    );

    layout.selected_item_rect = paint.selected_row_rect;
    layout.left_sorted_indices = order.to_vec();
    layout.left_row_targets = vec![None; browser_area.height as usize];
    let geometry = &paint.row_geometry;
    for (screen_row, target) in geometry
        .targets()
        .skip(geometry.offset())
        .take(browser_area.height as usize)
        .enumerate()
    {
        if let Some(id) = target {
            if let Some(index) = list.items.iter().position(|item| &item.id == id) {
                if let Some(slot) = layout.left_row_targets.get_mut(screen_row) {
                    *slot = Some(index);
                }
            }
        }
    }

    geometry.offset()
}
