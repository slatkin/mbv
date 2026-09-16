use super::{
    MediaKind, MediaList, MediaListCarrier, MediaListRow, MediaSemanticState, ViewportAnchor,
    WideMediaList, WideMediaListPaintPolicy,
};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tuirealm::component::Component;

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

fn paint(list: &mut WideMediaList<String>, area: Rect) {
    list.set_geometry(area, area);
    list.set_paint_policy(WideMediaListPaintPolicy::new(false));
    let mut terminal =
        Terminal::new(TestBackend::new(area.right().max(1), area.bottom().max(1))).unwrap();
    terminal.draw(|f| list.view(f, area)).unwrap();
}

#[test]
fn wide_list_maps_structural_rows_and_clamps_viewport() {
    let mut list = WideMediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a"),
        MediaListRow::Spacer,
        item("b"),
        item("c"),
        item("d"),
    ]);
    list.select_last();
    assert_eq!(list.resolve_viewport(3).offset, 3);
    assert_eq!(list.selected_row_offset(3), Some(2));
    paint(&mut list, Rect::new(2, 5, 10, 3));
    assert_eq!(
        list.resolve_current_point(Position { x: 4, y: 5 }),
        Some(&"b".into())
    );
    assert_eq!(
        list.resolve_current_point(Position { x: 4, y: 7 }),
        Some(&"d".into())
    );
    assert_eq!(list.resolve_current_point(Position { x: 4, y: 8 }), None);
    assert_eq!(list.resolve_current_point(Position { x: 1, y: 5 }), None);
}

#[test]
fn refresh_preserves_target_and_locally_clamps_missing_target() {
    let rows = vec![item("a"), item("b"), item("c"), item("d")];
    let mut list = WideMediaList::new();
    list.set_content(rows.clone());
    list.select_target(&"c".to_string());
    list.set_scroll(2);
    list.set_content(rows);
    assert_eq!(list.selected_target(), Some(&"c".to_string()));
    assert_eq!(list.scroll(), 2);
    list.set_content(vec![item("a"), item("b")]);
    assert_eq!(list.selected_target(), Some(&"b".to_string()));
    assert!(list.scroll() <= 1);
}

#[test]
fn fixed_row_owner_survives_wide_narrow_wide_geometry_changes() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content((0..8).map(|i| item(&i.to_string())).collect());
    carrier.select_target(&"5".to_string());
    carrier.set_scroll(5);
    let target = carrier.selected_target().cloned();

    // Both breakpoint arms configure the same fixed-row owner. The smaller
    // viewport clamps on view; no presentation-specific state is transferred.
    carrier.sync_viewport(3);
    paint(carrier.wide_mut(), Rect::new(0, 0, 20, 3));
    let narrow_offset = carrier.wide().current_flow_offset().unwrap();
    assert_eq!(carrier.selected_target(), target.as_ref());
    assert!(narrow_offset <= 5);

    carrier.sync_viewport(7);
    paint(carrier.wide_mut(), Rect::new(0, 0, 20, 7));
    assert_eq!(carrier.selected_target(), target.as_ref());
    assert!(carrier.wide().current_flow_offset().unwrap() <= narrow_offset);
}

#[test]
fn explicit_anchor_clamps_without_changing_fixed_row_owner() {
    let mut list = WideMediaList::new();
    list.set_content((0..6).map(|i| item(&i.to_string())).collect());
    list.select_target(&"3".to_string());
    let anchor = ViewportAnchor {
        selected_target: "3".to_string(),
        selected_row_offset: 5,
    };
    list.apply_viewport_anchor(&anchor, 2);
    assert_eq!(list.selected_target(), Some(&"3".to_string()));
    assert_eq!(list.resolve_viewport(2).offset, 2);
}

#[test]
fn fixed_row_claims_only_completed_current_frame() {
    let mut list = WideMediaList::new();
    list.set_content(vec![item("one"), item("two")]);
    let area = Rect::new(2, 1, 12, 2);
    list.set_geometry(area, area);
    assert!(!list.claims_current_point(Position { x: 2, y: 1 }));
    paint(&mut list, area);
    assert_eq!(
        list.resolve_current_point(Position { x: 2, y: 1 }),
        Some(&"one".into())
    );
    list.set_geometry(Rect::new(0, 0, 0, 2), area);
    assert!(!list.claims_current_point(Position { x: 2, y: 1 }));
}

#[test]
fn carrier_preserves_multi_selection_across_geometry_changes() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(vec![item("one"), item("two"), item("three")]);
    carrier.toggle_selection(&"one".to_string());
    carrier.toggle_selection(&"three".to_string());
    carrier.sync_viewport(2);
    assert_eq!(carrier.multi_selection(), &["one", "three"]);
}

#[test]
fn media_list_selection_summary_stays_provider_neutral() {
    let mut list = MediaList::new();
    list.set_content(vec![item("one"), item("two")]);
    list.enter_visual_mode();
    let transition = list.delegate_operation(super::MediaListOperation::Context("two".into()));
    assert!(transition.external_intent.is_some());
}
