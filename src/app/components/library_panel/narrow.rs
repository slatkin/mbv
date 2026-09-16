//! The non-Wide Library panel skeleton.  It keeps the same fixed-row
//! canonical media-list owner as the Wide skeleton; only the surrounding
//! selector/control rectangles and the row-flow geometry change.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::components::media_list::Presentation;
use crate::app::render::arrangements::wide_hero::pill_bar_areas;
use crate::app::render::{render_inline_search, render_placeholder};

use super::content::{LibraryPanelContent, ListSlot, PanelListPaintPolicy};
use super::slots::{
    paint_list_controls_row, paint_pill_bar_row, paint_pill_row_gap, paint_selector_row,
    SELECTOR_ROW_PREFIX,
};
use super::wide::{SkeletonHits, SkeletonPillWindows};

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct NarrowSkeletonGeometry {
    pub selector_bar: Rect,
    pub controls: Option<Rect>,
    pub list_area: Rect,
    pub selected: Option<Rect>,
}

/// Paint the non-Wide Library skeleton with ordinary fixed-height rows.
pub(in crate::app) fn render_narrow_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
    hovered_selector: Option<usize>,
    hits: &mut SkeletonHits,
    windows: &mut SkeletonPillWindows,
) -> NarrowSkeletonGeometry {
    let areas = pill_bar_areas(area);
    let searching = matches!(content.list, ListSlot::Search(_));
    match (&content.selector, searching) {
        (Some(selector), false) => paint_selector_row(
            f,
            areas.pills_area,
            areas.spacer_area,
            selector,
            hovered_selector,
            &mut hits.selector,
            &mut windows.selector,
            // The spacer is this panel showing through: in non-Wide geometry
            // that is the panel's own body, resolved with the same focus bit
            // the list below it uses.
            crate::app::palette::Surface::NarrowLibraryBody,
            browser_focused,
        ),
        (None, false) => {
            paint_pill_bar_row(
                f,
                areas.pills_area,
                &[],
                None,
                None,
                Some(SELECTOR_ROW_PREFIX),
                &mut hits.selector,
                &mut windows.selector,
            );
            paint_pill_row_gap(
                f,
                areas.spacer_area,
                crate::app::palette::Surface::NarrowLibraryBody,
                browser_focused,
            );
        }
        (_, true) => paint_pill_row_gap(
            f,
            areas.spacer_area,
            crate::app::palette::Surface::NarrowLibraryBody,
            browser_focused,
        ),
    }

    let (list_area, controls_area) = match &content.controls {
        Some(controls) if areas.content_area.height > 0 => {
            let row = Rect {
                height: 1,
                ..areas.content_area
            };
            paint_list_controls_row(f, row, controls, &mut hits.controls);
            (
                Rect {
                    y: row.bottom(),
                    height: areas.content_area.height.saturating_sub(1),
                    ..areas.content_area
                },
                Some(row),
            )
        }
        _ => (areas.content_area, None),
    };

    match &mut content.list {
        ListSlot::Search(search) => {
            let items = search.ordered_items();
            let query = search.query().to_string();
            let loading = search.loading();
            let cursor = search.cursor();
            let scroll_in = search.scroll();
            let new_scroll = render_inline_search(
                f,
                areas.pills_area,
                list_area,
                &query,
                loading,
                items,
                cursor,
                scroll_in,
                browser_focused,
                1,
                search.layout_mut(),
            );
            search.set_scroll(new_scroll);
        }
        ListSlot::Media(list) => {
            // The list owns the surface it sits on in this geometry: same
            // identity its zebra stripe resolves from, and the same focus bit,
            // so a focused narrow list is one flat surface and a resting one
            // keeps the default backdrop. No separate body fill here.
            list.set_presentation(Presentation::Wide, list_area.height.max(1) as usize);
            list.set_paint_policy(PanelListPaintPolicy::Narrow {
                focused: browser_focused,
            });
            list.set_geometry(list_area, list_area);
            list.view(f, list_area);
        }
        ListSlot::Empty { loading, text } => {
            let msg = if *loading {
                " Loading…"
            } else {
                text.as_str()
            };
            if !msg.is_empty() {
                render_placeholder(f, list_area, msg);
            }
        }
    }

    let selected = match &mut content.list {
        ListSlot::Media(list) => list.selected_row_rect(),
        _ => None,
    };
    NarrowSkeletonGeometry {
        selector_bar: areas.pills_area,
        controls: controls_area,
        list_area,
        selected,
    }
}

#[cfg(test)]
#[path = "narrow_tests.rs"]
mod narrow_tests;
