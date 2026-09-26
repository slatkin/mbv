//! The non-Wide Library panel skeleton. In non-Wide geometry the panel *is*
//! the Wide browser pane without a Hero (design D1): the shared
//! browser-pane composition paints the Selector row and the list box, with the
//! same surface identities the Wide list pane uses, and no Hero pane is
//! painted beside it.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::render::arrangements::wide_hero::wide_hero_browser_pane_with_selector;

use super::content::LibraryPanelContent;
use super::wide::{
    paint_browser_pane, selector_row_visible, SkeletonHits, SkeletonPillWindows,
    WideSkeletonGeometry,
};

/// Paint the non-Wide Library skeleton: the panel is the whole browser pane,
/// with a Selector band only when the content supplies a selector or search.
pub(in crate::app) fn render_narrow_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
    hovered_selector: Option<usize>,
    hits: &mut SkeletonHits,
    windows: &mut SkeletonPillWindows,
) -> WideSkeletonGeometry {
    let pane = wide_hero_browser_pane_with_selector(area, area, selector_row_visible(content));
    // The non-Wide rail's focus is the panel's bit alone: `workspace_focused`
    // is breakpoint-unaware (a Workspace focused in Wide survives a shrink
    // with the overlay closed), so the non-Wide rail never subtracts it.
    let browser = paint_browser_pane(
        f,
        pane,
        content,
        browser_focused,
        browser_focused,
        hovered_selector,
        hits,
        windows,
    );
    WideSkeletonGeometry {
        browser: area,
        #[cfg(test)]
        selector_bar: browser.selector_bar,
        list_panel: browser.list_panel,
        list_area: browser.list_area,
        selected: browser.selected,
        ..Default::default()
    }
}
