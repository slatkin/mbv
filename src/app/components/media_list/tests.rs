use super::{
    MediaKind, MediaList, MediaListCarrier, MediaListDisposition, MediaListOperation, MediaListRow,
    MediaSemanticState, ViewportAnchor, WideMediaList, WideMediaListPaintPolicy,
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

fn numbered_items(count: usize) -> Vec<MediaListRow<String>> {
    (0..count).map(|i| item(&i.to_string())).collect()
}

// Task 1.1: a step inside the window moves only the window.
#[test]
fn viewport_step_inside_the_window_moves_only_the_window() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"1".to_string());
    assert!(list.scroll_viewport(1, 3));
    assert_eq!(list.scroll(), 1);
    assert_eq!(list.selected_target(), Some(&"1".to_string()));
    assert!(list.scroll_viewport(-1, 3));
    assert_eq!(list.scroll(), 0);
    assert_eq!(list.selected_target(), Some(&"1".to_string()));
}

// Task 1.1: a step at the window's edge drags the selection, both directions.
#[test]
fn viewport_step_at_the_window_edge_drags_the_selection_both_ways() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    // Stepping down with the selection on the window's first visible row
    // drags it to the nearest selectable row inside the new window.
    list.select_target(&"0".to_string());
    assert!(list.scroll_viewport(1, 3));
    assert_eq!(list.scroll(), 1);
    assert_eq!(list.selected_target(), Some(&"1".to_string()));
    // Stepping up with the selection on the window's last visible row drags
    // it back to the nearest selectable row above the window's last row.
    list.set_scroll(3);
    list.select_target(&"5".to_string());
    assert!(list.scroll_viewport(-1, 3));
    assert_eq!(list.scroll(), 2);
    assert_eq!(list.selected_target(), Some(&"4".to_string()));
}

// Task 1.1: both content ends clamp.
#[test]
fn viewport_step_clamps_at_both_content_ends() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"0".to_string());
    assert!(!list.scroll_viewport(-1, 3));
    assert_eq!(list.scroll(), 0);
    assert_eq!(list.selected_target(), Some(&"0".to_string()));
    list.set_scroll(3);
    list.select_target(&"5".to_string());
    assert!(!list.scroll_viewport(1, 3));
    assert_eq!(list.scroll(), 3);
    assert_eq!(list.selected_target(), Some(&"5".to_string()));
}

// Task 1.1 / D3: the drag resolves the nearest *selectable* row against the
// ascending selectable index; a `Heading` inside the window is never selected.
#[test]
fn viewport_drag_skips_structural_rows_in_both_directions() {
    let mut list = MediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a"),
        item("b"),
        MediaListRow::Heading { text: "B".into() },
        item("c"),
        item("d"),
        item("e"),
    ]);
    // Display rows: 0=Heading, 1=a, 2=b, 3=Heading, 4=c, 5=d, 6=e.
    // Step down from window [2, 5) with the selection on its first visible
    // row: the new window [3, 6) starts at the `Heading`, so the drag lands
    // past it on "c".
    list.set_scroll(2);
    list.select_target(&"b".to_string());
    assert!(list.scroll_viewport(1, 3));
    assert_eq!(list.scroll(), 3);
    assert_eq!(list.selected_target(), Some(&"c".to_string()));
    // Step up from window [2, 5) with the selection on its last visible row:
    // the row above the new window's last row is the `Heading`, so the drag
    // lands before it on "b".
    list.set_scroll(2);
    list.select_target(&"c".to_string());
    assert!(list.scroll_viewport(-1, 3));
    assert_eq!(list.scroll(), 1);
    assert_eq!(list.selected_target(), Some(&"b".to_string()));
}

// Task 1.2: a page moves by the painted height and drags the selection.
#[test]
fn viewport_page_moves_by_the_painted_height_and_drags() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(10));
    list.select_target(&"0".to_string());
    assert!(list.scroll_viewport_page(1, 3));
    assert_eq!(list.scroll(), 3);
    assert_eq!(list.selected_target(), Some(&"3".to_string()));
    list.select_target(&"6".to_string());
    assert!(list.scroll_viewport_page(-1, 3));
    assert_eq!(list.scroll(), 0);
    assert_eq!(list.selected_target(), Some(&"2".to_string()));
}

// Task 1.2: a page longer than the remaining content clamps.
#[test]
fn viewport_page_clamps_to_the_remaining_content() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(10));
    list.set_scroll(6);
    list.select_target(&"6".to_string());
    assert!(list.scroll_viewport_page(1, 3));
    assert_eq!(list.scroll(), 7);
    assert_eq!(list.selected_target(), Some(&"7".to_string()));
    // Already at the last display row: a further page changes nothing.
    assert!(!list.scroll_viewport_page(1, 3));
    assert_eq!(list.scroll(), 7);
    assert_eq!(list.selected_target(), Some(&"7".to_string()));
}

// Task 1.3: transition dispositions — a window-only move is consumed with no
// selection move; a drag reports the moved selection; a boundary no-op stays
// unhandled.
#[test]
fn viewport_step_transition_dispositions() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"1".to_string());
    let window_only =
        list.delegate_viewport_operation(MediaListOperation::ScrollViewport(1), Some(3));
    assert_eq!(window_only.disposition, MediaListDisposition::Consumed);
    assert_eq!(window_only.selected_target, None);
    assert_eq!(window_only.selection_summary, None);
    assert_eq!(window_only.external_intent, None);
    assert_eq!(list.scroll(), 1);

    list.set_scroll(0);
    list.select_target(&"0".to_string());
    let dragged = list.delegate_viewport_operation(MediaListOperation::ScrollViewport(1), Some(3));
    assert_eq!(dragged.disposition, MediaListDisposition::Consumed);
    assert_eq!(dragged.selected_target, Some("1".to_string()));

    list.set_scroll(0);
    list.select_target(&"0".to_string());
    let boundary =
        list.delegate_viewport_operation(MediaListOperation::ScrollViewport(-1), Some(3));
    assert_eq!(boundary.disposition, MediaListDisposition::Unhandled);
    assert_eq!(boundary.selected_target, None);
    assert_eq!(list.scroll(), 0);
}

// Task 1.2: the page op through the height-aware delegate — a page drags the
// selection the same way and is consumed.
#[test]
fn viewport_page_transition_moves_by_the_height_and_reports_the_drag() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(10));
    list.select_target(&"0".to_string());
    let paged =
        list.delegate_viewport_operation(MediaListOperation::ScrollViewportPage(1), Some(3));
    assert_eq!(paged.disposition, MediaListDisposition::Consumed);
    assert_eq!(paged.selected_target, Some("3".to_string()));
    assert_eq!(paged.selection_summary, None);
    assert_eq!(list.scroll(), 3);
    // Paging back up drags the selection the same way, and a further page at
    // the top boundary stays unhandled.
    let paged_up =
        list.delegate_viewport_operation(MediaListOperation::ScrollViewportPage(-1), Some(3));
    assert_eq!(paged_up.disposition, MediaListDisposition::Consumed);
    assert_eq!(paged_up.selected_target, Some("2".to_string()));
    assert_eq!(list.scroll(), 0);
    let boundary =
        list.delegate_viewport_operation(MediaListOperation::ScrollViewportPage(-1), Some(3));
    assert_eq!(boundary.disposition, MediaListDisposition::Unhandled);
    assert_eq!(boundary.selected_target, None);
    assert_eq!(list.scroll(), 0);
}

// Task 1.3: a step never extends the live range — the multi-selection is
// untouched even when the drag fires.
#[test]
fn viewport_step_during_visual_mode_leaves_the_multi_selection_unchanged() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"2".to_string());
    list.enter_visual_mode();
    list.delegate_operation(MediaListOperation::Move(2));
    assert_eq!(list.multi_selection(), &["2", "3", "4"]);
    // The selection ("4", display row 4) sits outside the new window [1, 4),
    // so the step drags the cursor to "3" without touching the range.
    let stepped = list.delegate_viewport_operation(MediaListOperation::ScrollViewport(1), Some(3));
    assert_eq!(stepped.disposition, MediaListDisposition::Consumed);
    assert_eq!(stepped.selected_target, Some("3".to_string()));
    assert_eq!(stepped.selection_summary, None);
    assert_eq!(list.multi_selection(), &["2", "3", "4"]);
}

// Task 1.3 / D2: the height-free delegate cannot resolve the painted window,
// so a viewport step routed through it is an unhandled no-op.
#[test]
fn height_free_delegate_leaves_a_viewport_step_unhandled() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"0".to_string());
    let transition = list.delegate_operation(MediaListOperation::ScrollViewport(1));
    assert_eq!(transition.disposition, MediaListDisposition::Unhandled);
    assert_eq!(transition.selected_target, None);
    assert_eq!(list.scroll(), 0);
    assert_eq!(list.selected_target(), Some(&"0".to_string()));
}
