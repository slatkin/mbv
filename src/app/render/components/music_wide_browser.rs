use crate::app::components::media_list::{
    SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::layout::LayoutMain;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::component::Component;

/// Paints the wide Music right rail through the canonical `WideMediaList`
/// control, exactly as the wide TV series rail and wide Movies list do
/// (`render_wide_tv_with_ctx`). The grouped artist-header / album / spacer
/// sequence is projected onto `MediaListRow::Heading` / `Item` / `Spacer`;
/// the control enforces that headings and spacers are never selectable.
///
/// `MusicWorkspaceComponent` feeds the persistent control when content is
/// projected. This function only configures and paints its current state.
pub(in crate::app) fn render_wide_right_album_browser_with_ctx(
    f: &mut Frame,
    browser_area: Rect,
    panel_area: Rect,
    list: &LibraryListRenderCtx,
    right_focused: bool,
    layout: &mut LayoutMain,
    media: &mut WideMediaList<String>,
) {
    layout.wide_music_browser_area = browser_area;
    if list.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            browser_area,
            if list.loading {
                " Loading\u{2026}"
            } else {
                " (empty)"
            },
        );
        return;
    }

    // The canonical rail owns the full panel row (selection markers and
    // selected backgrounds reach the panel border); `browser_area` remains
    // the padded hit/scroll geometry.
    let paint_area = Rect {
        x: panel_area.x,
        width: panel_area.width,
        ..browser_area
    };
    media.set_geometry(paint_area, browser_area);
    media.set_paint_policy(WideMediaListPaintPolicy::new(
        right_focused,
        SelectedRowSurface::ListBackdrop,
        None,
    ));
    Component::view(media, f, browser_area);

    layout.selected_item_rect = media.current_selected_row_rect();
}
