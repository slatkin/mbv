use super::*;
use crate::app::components::library_panel::{
    LibraryPanelContent, ListControls, ListSlot, SelectorRow,
};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation,
};
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
    let mut carrier = MediaListCarrier::new(Presentation::Wide);
    carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
    carrier.select_target(&"beta".to_string());
    let mut content = LibraryPanelContent {
        selector: Some(SelectorRow {
            pills: vec!["All".into()],
            active: Some(0),
        }),
        controls: Some(ListControls {
            label: "3 items".into(),
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
    let controls = geometry.controls.expect("controls row reserved");
    // The row flow is inset inside the list panel: one spacer row of the panel
    // above it (`PANE_PAD_Y`), which the inset's own surface paints.
    assert_eq!(
        geometry.list_area.y,
        controls.bottom() + crate::app::render::PANE_PAD_Y
    );
    assert!(geometry.selected.is_some());
    assert_eq!(carrier.selected_target(), Some(&"beta".to_string()));
}

#[test]
fn fixed_row_owner_clamps_when_narrow_viewport_shrinks_and_restores() {
    let mut carrier = MediaListCarrier::new(Presentation::Wide);
    carrier.set_content((0..10).map(|i| item(&i.to_string())).collect());
    carrier.select_target(&"9".to_string());
    carrier.set_scroll(9);
    let mut content = LibraryPanelContent {
        selector: None,
        controls: None,
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
