use crate::app::components::media_list::{MediaListRow, MediaSemanticState, WideMediaList};
use crate::app::layout::{LayoutMain, LibraryRowTarget};
use crate::app::palette;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
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
) -> usize {
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
        return 0;
    }

    let mut media: WideMediaList<String> = WideMediaList::new();
    media.set_content(wide_album_rows(&list.items, album_info, order));
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
        &mut media,
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
                    *slot = Some(LibraryRowTarget::Album(index));
                }
            }
        }
    }

    geometry.offset()
}

/// Projects the grouped album order onto the canonical row vocabulary: one
/// `Heading` per artist group, a `Spacer` between groups, and one `Item` per
/// album keyed by its stable id.
fn wide_album_rows(
    albums: &[mbv_core::api::EmbyItem],
    album_info: &[(String, String, String)],
    order: &[usize],
) -> Vec<MediaListRow<String>> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let artist = album_info[order[start]].0.clone();
        let mut end = start + 1;
        while end < order.len() && album_info[order[end]].0 == artist {
            end += 1;
        }
        if start > 0 {
            rows.push(MediaListRow::Spacer);
        }
        rows.push(MediaListRow::Heading { text: artist });
        for &idx in &order[start..end] {
            let (_, year, name) = &album_info[idx];
            rows.push(MediaListRow::Item {
                target: albums[idx].id.clone(),
                primary: name.clone(),
                trailing: (!year.is_empty()).then(|| year.clone()),
                duration: None,
                semantic_state: MediaSemanticState::Ordinary,
            });
        }
        start = end;
    }
    rows
}
