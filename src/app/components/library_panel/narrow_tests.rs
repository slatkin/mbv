use super::super::wide::text_in;
use super::*;
use crate::app::components::inline_search::{InlineSearch, SearchPool};
use crate::app::components::library_panel::{LibraryPanelContent, ListSlot, SelectorRow};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState,
};
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn item(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

#[test]
fn narrow_skeleton_keeps_fixed_rows_and_panel_slots() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
    carrier.select_target(&"beta".to_string());
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            active: Some(0),
        }),
        list: ListSlot::Media(&mut carrier),
        hero: None,
    };
    let area = Rect::new(0, 0, 60, 30);
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    let mut hits = super::super::wide::SkeletonHits::default();
    let mut windows = super::super::wide::SkeletonPillWindows::default();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = Some(render_narrow_skeleton(
                f,
                area,
                &mut content,
                false,
                None,
                &mut hits,
                &mut windows,
            ));
        })
        .unwrap();
    let geometry = geometry.expect("narrow skeleton painted");
    let buf = terminal.backend().buffer();
    assert!(buf[(geometry.selector_bar.x, geometry.selector_bar.y)]
        .symbol()
        .contains(' '));
    // The list box follows the Selector band directly; its row flow keeps
    // the shared browser-pane inset and has no secondary controls row.
    assert_eq!(
        geometry.list_area.y,
        geometry.list_panel.y + crate::app::render::PANE_PAD_Y
    );
    assert!(geometry.selected.is_some());
    assert_eq!(carrier.selected_target(), Some(&"beta".to_string()));
}

#[test]
fn fixed_row_owner_clamps_when_narrow_viewport_shrinks_and_restores() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content((0..10).map(|i| item(&i.to_string())).collect());
    carrier.select_target(&"9".to_string());
    carrier.set_scroll(9);
    let mut content = LibraryPanelContent {
        selector: None,
        list: ListSlot::Media(&mut carrier),
        hero: None,
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    let mut hits = super::super::wide::SkeletonHits::default();
    let mut windows = super::super::wide::SkeletonPillWindows::default();
    terminal
        .draw(|f| {
            render_narrow_skeleton(
                f,
                Rect::new(0, 0, 60, 20),
                &mut content,
                false,
                None,
                &mut hits,
                &mut windows,
            );
        })
        .unwrap();
    assert_eq!(carrier.selected_target(), Some(&"9".to_string()));
    assert!(carrier.wide().current_flow_offset().unwrap() <= 9);
}

/// A search session at the non-Wide breakpoint paints the same one-bar +
/// one-list composition as Wide (task 6.3): the search box takes the
/// Selector row, the scored results flow through the one canonical fixed-row
/// list box, and the carrier retains exactly that rect as its hit geometry.
#[test]
fn narrow_search_paints_one_search_bar_and_one_result_list() {
    let mut search = InlineSearch::new();
    search.open();
    search.set_pool(SearchPool::Items(crate::app::tests::make_items(3)));
    search.restore_query("ite".into());
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            active: Some(0),
        }),
        list: ListSlot::Search(&mut search),
        hero: None,
    };
    let area = Rect::new(0, 0, 60, 30);
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    let mut hits = super::super::wide::SkeletonHits::default();
    let mut windows = super::super::wide::SkeletonPillWindows::default();
    let mut geometry = None;
    terminal
        .draw(|f| {
            geometry = Some(render_narrow_skeleton(
                f,
                area,
                &mut content,
                false,
                None,
                &mut hits,
                &mut windows,
            ));
        })
        .unwrap();
    let geometry = geometry.expect("narrow skeleton painted");
    let buf = terminal.backend().buffer();
    // One search bar in the Selector row's place, with no selector pill.
    assert!(text_in(buf, geometry.selector_bar, "SEARCH:"));
    assert!(text_in(buf, geometry.selector_bar, "ite"));
    assert!(
        !text_in(buf, geometry.selector_bar, "\u{25e2}"),
        "no selector pill under the search box"
    );
    // One result list in the list box, through the canonical fixed-row
    // presentation; the carrier retained that rect as its hit geometry.
    assert!(text_in(buf, geometry.list_area, "Item 0"));
    assert_eq!(
        search.results().current_content_rect(),
        Some(geometry.list_area)
    );
}

/// The non-Wide list box is the Wide browser pane's list box (design D3): the
/// `LibraryPanel` fill pair on the box, with no stripe pair of its own —
/// the browser list paints no zebra, so every unselected row rests on the
/// box fill in both focus states.
#[test]
fn narrow_list_box_uses_the_wide_browser_pane_fill_without_stripes() {
    for focused in [false, true] {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
        let mut content = LibraryPanelContent {
            selector: None,
            list: ListSlot::Media(&mut carrier),
            hero: None,
        };
        let area = Rect::new(0, 0, 60, 20);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut hits = super::super::wide::SkeletonHits::default();
        let mut windows = super::super::wide::SkeletonPillWindows::default();
        let mut geometry = None;
        terminal
            .draw(|f| {
                geometry = Some(render_narrow_skeleton(
                    f,
                    area,
                    &mut content,
                    focused,
                    None,
                    &mut hits,
                    &mut windows,
                ));
            })
            .unwrap();
        let geometry = geometry.expect("narrow skeleton painted");
        let buf = terminal.backend().buffer();
        // The list box carries the `LibraryPanel` fill for this focus state.
        assert_eq!(
            buf[(geometry.list_panel.x, geometry.list_panel.y)].bg,
            palette::surface_colors(palette::Surface::LibraryPanel, focused).fill
        );
        // No zebra on the browser list: the unselected rows rest on the
        // `LibraryPanel` box fill, never the `MainContentBox` stripe pair.
        for dy in 1..=2 {
            assert_eq!(
                buf[(geometry.list_area.x, geometry.list_area.y + dy)].bg,
                palette::surface_colors(palette::Surface::LibraryPanel, focused).fill
            );
        }
    }
}
