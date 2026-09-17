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

// Task 1.1 / D3: a window whose rows are all structural (`Heading`/`Spacer`)
// shows no selectable row, so the selection stays where it is even though the
// nearest selectable candidate lies outside the moved window.
#[test]
fn viewport_step_leaves_the_selection_when_the_window_shows_no_selectable_row() {
    let mut list = MediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a"),
        MediaListRow::Heading { text: "B".into() },
        MediaListRow::Spacer,
        item("b"),
    ]);
    // Display rows: 0=Heading, 1=a, 2=Heading, 3=Spacer, 4=b.
    // Step down from window [1, 3) with the selection on its first visible
    // row: the new window [2, 4) holds only the `Heading` and the `Spacer`,
    // and the nearest selectable row past the top ("b", row 4) is outside it,
    // so the selection stays on "a" while the window alone moves. (The window
    // is stored after the selection so the D4 cursor rule does not back it up
    // onto the `Heading` — this is a step, not a cursor move.)
    list.select_target(&"a".to_string());
    list.set_scroll(1);
    assert!(list.scroll_viewport(1, 2));
    assert_eq!(list.scroll(), 2);
    assert_eq!(list.selected_target(), Some(&"a".to_string()));
    // Mirrored upward: step up from window [2, 4) with the selection on its
    // last visible row; the new window [1, 3) holds the `Heading` and the
    // `Spacer`, and the nearest selectable row before the bottom ("a", row 1)
    // is inside it, so use a list whose nearest candidate falls below instead.
    let mut list = MediaList::new();
    list.set_content(vec![
        item("a"),
        MediaListRow::Heading { text: "B".into() },
        MediaListRow::Spacer,
        MediaListRow::Heading { text: "C".into() },
        MediaListRow::Spacer,
        item("b"),
    ]);
    // Display rows: 0=a, 1=Heading, 2=Spacer, 3=Heading, 4=Spacer, 5=b.
    list.select_target(&"b".to_string());
    list.set_scroll(2);
    assert!(list.scroll_viewport(-1, 2));
    assert_eq!(list.scroll(), 1);
    // The new window [1, 3) holds only structural rows and the nearest
    // selectable row before its bottom ("a", row 0) is outside it, so the
    // selection stays on "b".
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

// Task 2.1 / D4: a cursor move that raises the window onto the first
// selectable row backs up one row further so its labelling `Heading` stays
// inside the window.
#[test]
fn cursor_move_to_the_first_selectable_row_keeps_its_heading_in_the_window() {
    let mut list = MediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a1"),
        item("a2"),
        MediaListRow::Heading { text: "B".into() },
        item("b1"),
        item("b2"),
    ]);
    // Display rows: 0=Heading A, 1=a1, 2=a2, 3=Heading B, 4=b1, 5=b2.
    list.select_target(&"a2".to_string());
    list.set_scroll(2);
    let transition = list.delegate_operation(MediaListOperation::Move(-1));
    assert_eq!(transition.selected_target, Some("a1".to_string()));
    // The minimum move would pin a1 on the window's first row with its
    // `Heading` directly above; the window moves one row further instead.
    assert_eq!(list.scroll(), 0);
    assert_eq!(list.resolve_viewport(3).offset, 0);
    assert_eq!(list.selected_target(), Some(&"a1".to_string()));
}

// Task 2.1 / D4: a stored window whose top sits above the selection is
// preserved — a cursor move that keeps the selection inside it does not
// raise the window onto the cursor.
#[test]
fn cursor_move_preserves_a_stored_window_above_the_selection() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"2".to_string());
    list.set_scroll(1);
    assert_eq!(
        list.delegate_operation(MediaListOperation::Move(1))
            .selected_target,
        Some("3".to_string())
    );
    assert_eq!(list.scroll(), 1);
    assert_eq!(
        list.delegate_operation(MediaListOperation::Move(-1))
            .selected_target,
        Some("2".to_string())
    );
    assert_eq!(list.scroll(), 1);
}

// Task 2.1 / D4: an in-window cursor move that lands on the window's first
// row with a `Heading` above it does not raise the window — the heading
// backup fires only when the move actually shifted the window.
#[test]
fn cursor_move_inside_the_window_does_not_back_up_over_a_heading() {
    let mut list = MediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a1"),
        item("a2"),
        item("a3"),
    ]);
    // Display rows: 0=Heading A, 1=a1, 2=a2, 3=a3.
    list.select_target(&"a2".to_string());
    list.set_scroll(1);
    let transition = list.delegate_operation(MediaListOperation::Move(-1));
    assert_eq!(transition.selected_target, Some("a1".to_string()));
    // The selection already sat on the window's first row; no window move is
    // needed, so the window stays at the stored top instead of backing up
    // over the `Heading`.
    assert_eq!(list.scroll(), 1);
    assert_eq!(list.resolve_viewport(3).offset, 1);
    assert_eq!(list.selected_target(), Some(&"a1".to_string()));
}

// Task 2.1 / D4: the step path stays clamped solely by the content ends —
// a step may leave the selection on the window's first row with its
// labelling `Heading` scrolled off, because a step is reversible.
#[test]
fn viewport_step_may_still_scroll_the_label_off() {
    let mut list = MediaList::new();
    list.set_content(vec![
        MediaListRow::Heading { text: "A".into() },
        item("a"),
        item("b"),
        item("c"),
        item("d"),
        item("e"),
    ]);
    list.select_target(&"a".to_string());
    assert!(list.scroll_viewport(1, 3));
    // The step moves the window onto the selection's row without backing up
    // over the `Heading`, and the selection does not move.
    assert_eq!(list.scroll(), 1);
    assert_eq!(list.selected_target(), Some(&"a".to_string()));
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

// Task 3.1 / D5: a reordered row flow keeps the window on the same target —
// the previous flow's window-top target is re-found in the new flow and the
// window is restored to its row, with no display-row index carried over.
#[test]
fn set_content_reordered_flow_keeps_the_window_on_the_same_target() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(8));
    list.select_target(&"2".to_string());
    list.set_scroll(5);
    let reordered: Vec<_> = (0..8).rev().map(|i| item(&i.to_string())).collect();
    list.set_content(reordered);
    // "5" was the previous window top; in the reversed flow it sits at
    // display row 2, so the window anchors there, not at the old index 5.
    assert_eq!(list.scroll(), 2);
    assert_eq!(list.selected_target(), Some(&"2".to_string()));
}

// Task 3.1 / D5: a regrouped flow restores the label — the previous flow
// showed a `Heading` directly above the window-top target and the new flow
// still places one above it, so the window anchors to that `Heading`.
#[test]
fn set_content_regrouped_flow_restores_the_heading_above_the_target() {
    let mut list = MediaList::new();
    list.set_content(vec![
        item("a1"),
        item("a2"),
        MediaListRow::Heading {
            text: "M–O".into()
        },
        item("m1"),
        item("m2"),
        item("m3"),
    ]);
    // Display rows: 0=a1, 1=a2, 2=Heading, 3=m1, 4=m2, 5=m3. The window top
    // is "m1" with its labelling `Heading` directly above, and the selection
    // sits inside the window.
    list.select_target(&"m2".to_string());
    list.set_scroll(3);
    list.set_content(vec![
        item("a1"),
        item("a2"),
        item("a3"),
        MediaListRow::Heading {
            text: "M–O".into()
        },
        item("m1"),
        item("m2"),
        item("m3"),
    ]);
    // Display rows: 0..2=a1..a3, 3=Heading, 4=m1, 5=m2, 6=m3. "m1" moved to
    // row 4 with a `Heading` directly above, so the window anchors to that
    // `Heading` and the label and the target paint together.
    assert_eq!(list.scroll(), 3);
    assert_eq!(list.selected_target(), Some(&"m2".to_string()));
}

// Task 3.1 / D5: when the new flow no longer places a `Heading` directly
// above the re-found target, the recorded heading offset is not consumed —
// the window anchors to the target's own row, not one row higher.
#[test]
fn set_content_anchors_to_the_target_row_when_the_new_flow_has_no_leading_heading() {
    let mut list = MediaList::new();
    list.set_content(vec![
        item("a1"),
        MediaListRow::Heading {
            text: "M–O".into()
        },
        item("m1"),
        item("m2"),
    ]);
    // Display rows: 0=a1, 1=Heading, 2=m1, 3=m2. Window top "m1", label above.
    list.select_target(&"m2".to_string());
    list.set_scroll(2);
    // The regrouped flow puts an ordinary row (not a `Heading`) directly
    // above "m1", so anchoring one row higher would point at "x0".
    list.set_content(vec![item("x0"), item("m1"), item("m2"), item("a1")]);
    // Display rows: 0=x0, 1=m1, 2=m2, 3=a1. The window rests on "m1" itself.
    assert_eq!(list.scroll(), 1);
    assert_eq!(list.selected_target(), Some(&"m2".to_string()));
}

// Task 3.1 / D5: a window-top target that is gone from the new flow falls
// back to keeping the selection inside the window and clamping.
#[test]
fn set_content_missing_target_falls_back_to_keeping_the_selection_inside_the_window() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(8));
    list.select_target(&"2".to_string());
    list.set_scroll(5);
    // Drop "5", the previous window-top target, from the replacement.
    let without_five: Vec<_> = (0..8)
        .filter(|i| *i != 5)
        .map(|i| item(&i.to_string()))
        .collect();
    list.set_content(without_five);
    // The selection ("2", display row 2) stays inside the window: the
    // fallback window raises to it from the clamped stale offset instead of
    // pointing at unrelated rows.
    assert_eq!(list.scroll(), 2);
    assert_eq!(list.selected_target(), Some(&"2".to_string()));
}

// Task 3.1 / D5: an append far from the window leaves it unchanged.
#[test]
fn set_content_append_far_from_the_window_leaves_it_unchanged() {
    let mut list = MediaList::new();
    list.set_content(numbered_items(6));
    list.select_target(&"2".to_string());
    list.set_scroll(2);
    list.set_content(numbered_items(30));
    assert_eq!(list.scroll(), 2);
    assert_eq!(list.selected_target(), Some(&"2".to_string()));
}

// Task 4.2 / D2: the carrier resolves a viewport step's height from the
// retained painted content rectangle — the same source hit resolution uses;
// with no retained frame the step is an unhandled no-op.
#[test]
fn carrier_step_resolves_its_height_from_the_retained_frame() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(numbered_items(10));
    carrier.select_target(&"1".to_string());

    // No retained frame yet: the step cannot resolve a height and changes
    // nothing.
    let transition = carrier.delegate_operation(MediaListOperation::ScrollViewport(1));
    assert_eq!(transition.disposition, MediaListDisposition::Unhandled);
    assert_eq!(carrier.scroll(), 0);
    assert_eq!(transition.selected_target, None);

    // Paint a 3-row frame: the step's height is the retained content rect's
    // height, so the window steps one row without dragging the selection.
    paint(carrier.wide_mut(), Rect::new(0, 0, 10, 3));
    let transition = carrier.delegate_operation(MediaListOperation::ScrollViewport(1));
    assert_eq!(transition.disposition, MediaListDisposition::Consumed);
    assert_eq!(carrier.scroll(), 1);
    assert_eq!(
        transition.selected_target, None,
        "a window-only step reports no selection move"
    );
}

// Task 4.2 / D9: `sync_viewport` is the panel's geometry clamp — it lowers an
// overshooting stored window in place but never stores a resolved offset and
// never raises the window onto the selection.
#[test]
fn sync_viewport_clamps_in_place_without_raising_the_window() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(numbered_items(10));
    carrier.select_target(&"8".to_string());

    // A stale window below the selection survives the height sync untouched:
    // the display clamp at paint shows the selection, the owner's window
    // stays authoritative.
    carrier.sync_viewport(3);
    assert_eq!(carrier.scroll(), 0);

    // An overshooting stored window is lowered to the content's end.
    carrier.set_scroll(9);
    carrier.sync_viewport(3);
    assert_eq!(carrier.scroll(), 7);

    // The clamp never raises: a stored window inside the range stays put.
    carrier.set_scroll(2);
    carrier.sync_viewport(3);
    assert_eq!(carrier.scroll(), 2);
}
