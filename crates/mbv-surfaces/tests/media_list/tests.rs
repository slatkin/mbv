//! The canonical media-list owner's public-API tests, moved with `MediaList`
//! from `mbv-components` (add-gui-front-end 4.3). Assertions are unchanged;
//! the pointer-input construction the adapter owned (`into_operation`) stayed
//! there, so the moved selection tests name the target-resolved operation.

use mbv_surfaces::list::{ViewportAnchor, Viewported};
use mbv_surfaces::media_list::{MediaList, MediaListDisposition, MediaListOperation};

use super::helpers::{counted_row, item, list, reset_target_clones, target_clones};

#[test]
fn refresh_preserves_target_and_resolves_a_missing_target_to_the_first_row() {
    let rows = vec![item("a"), item("b"), item("c"), item("d")];
    let mut list = MediaList::new();
    list.set_content(rows.clone());
    list.select_target(&"c".to_string());
    list.set_scroll(2);
    list.set_content(rows);
    assert_eq!(list.selected_target(), Some(&"c".to_string()));
    assert_eq!(list.scroll(), 2);
    list.set_content(vec![item("a"), item("b")]);
    assert_eq!(list.selected_target(), Some(&"a".to_string()));
    assert!(list.scroll() <= 1);
}

/// D2 (shared-list-components): a keyboard move after a free scroll
/// re-anchors the viewport to the selection — the key advances the selection
/// by one and the resolved offset pulls that row back into view.
#[test]
fn a_key_after_a_free_scroll_bring_the_selection_back_into_view() {
    let mut list = MediaList::new();
    list.set_content(vec![item("a"), item("b"), item("c"), item("d"), item("e")]);
    let flow = list.row_flow();
    Viewported::scroll_viewport(&mut list, &flow, 3, 100);
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(
        list.viewport_offset(),
        2,
        "the scroll clamped past 'a', so it is out of view"
    );

    let transition = list.delegate_operation(MediaListOperation::Move(1));

    assert_eq!(transition.selected_target, Some("b".to_string()));
    assert_eq!(list.viewport_anchor(), ViewportAnchor::FollowSelection);
    assert_eq!(list.resolve_viewport(3).offset, 1);
}

/// D2 (shared-list-components, invariant 23): a pointer right-click
/// (`Context`) after a free scroll keeps the user's offset — unlike the
/// keyboard `ContextCurrent` it sets no selection, so the viewport stays
/// free and the selection stays where it was.
#[test]
fn a_pointer_context_click_after_a_free_scroll_keeps_the_free_viewport() {
    let mut list = MediaList::new();
    list.set_content(vec![item("a"), item("b"), item("c"), item("d"), item("e")]);
    let flow = list.row_flow();
    Viewported::scroll_viewport(&mut list, &flow, 3, 100);
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(list.viewport_offset(), 2);

    let transition = list.delegate_operation(MediaListOperation::Context("c".to_string()));

    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(list.viewport_offset(), 2);
    assert_eq!(list.selected_target(), Some(&"a".to_string()));
    assert!(transition.external_intent.is_some());
}

/// D2 (shared-list-components): refreshed content that still holds the
/// selected target keeps a freely scrolled viewport exactly where it is.
#[test]
fn refreshed_content_keeps_a_freely_scrolled_viewport() {
    let rows = vec![item("a"), item("b"), item("c"), item("d"), item("e")];
    let mut list = MediaList::new();
    list.set_content(rows.clone());
    let flow = list.row_flow();
    Viewported::scroll_viewport(&mut list, &flow, 3, 2);

    list.set_content(rows);

    assert_eq!(list.selected_target(), Some(&"a".to_string()));
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(list.scroll(), 2);
}

/// Regression (#926): the wheel's `Scroll` arm must never build the row
/// flow — the offset needs only the row count. Over 1,000 rows a notch
/// clones a bounded handful of targets (the transition's before/after
/// selection reads), where a flow build would clone all of them.
#[test]
fn a_wheel_scroll_never_builds_the_row_flow() {
    let mut list = MediaList::new();
    list.set_content((0..1000).map(counted_row).collect());
    reset_target_clones();

    let transition = list.delegate_operation(MediaListOperation::Scroll(1));

    assert_eq!(transition.disposition, MediaListDisposition::Consumed);
    assert_eq!(list.viewport_offset(), 1);
    let clones = target_clones();
    assert!(
        clones <= 4,
        "the scroll built the row flow: {clones} target clones"
    );
}

#[test]
fn toggle_adds_and_removes() {
    let mut list = list();
    list.delegate_operation(MediaListOperation::Select(2));
    list.toggle_selection(&2);
    assert_eq!(list.multi_selection(), &[2]);
    list.toggle_selection(&2);
    assert_eq!(list.multi_selection(), [] as [u8; 0]);
}

#[test]
fn first_toggle_includes_prior_cursor() {
    let mut list = list();
    list.select_target(&3);
    list.toggle_selection(&6);
    assert_eq!(list.multi_selection(), &[3, 6]);
}

#[test]
fn range_recomputes_from_anchor() {
    let mut list = list();
    list.select_target(&2);
    list.toggle_selection(&5);
    list.extend_selection_to(&7);
    assert_eq!(list.multi_selection(), &[2, 3, 4, 5, 6, 7]);
    list.extend_selection_to(&4);
    assert_eq!(list.multi_selection(), &[2, 3, 4, 5]);
}

#[test]
fn visual_mode_starts_at_cursor_and_movement_extends_from_anchor() {
    let mut list = list();
    list.select_target(&3);
    list.enter_visual_mode();
    assert_eq!(list.multi_selection(), &[3]);
    list.delegate_operation(MediaListOperation::Move(2));
    assert_eq!(list.multi_selection(), &[3, 4, 5]);
    list.delegate_operation(MediaListOperation::Move(-1));
    assert_eq!(list.multi_selection(), &[3, 4]);
}

#[test]
fn keyboard_disjoint_selection_freezes_and_reanchors() {
    let mut list = list();
    list.select_target(&3);
    list.enter_visual_mode();
    list.delegate_operation(MediaListOperation::Move(1));
    list.toggle_selection(&4);
    list.delegate_operation(MediaListOperation::Move(2));
    assert_eq!(list.multi_selection(), &[3]);
    list.enter_visual_mode();
    list.delegate_operation(MediaListOperation::Move(0));
    assert_eq!(list.multi_selection(), &[3, 6]);
}

#[test]
fn shift_range_unions_frozen_selection() {
    let mut list = list();
    list.select_target(&2);
    list.toggle_selection(&5);
    list.extend_selection_to(&4);
    assert_eq!(list.multi_selection(), &[2, 3, 4, 5]);
}

#[test]
fn select_all_marks_every_selectable_row_and_keeps_the_cursor() {
    let mut list = list();
    list.select_target(&5);
    list.select_all();
    assert_eq!(list.multi_selection(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(list.is_visual_mode());
    assert_eq!(list.selected_target(), Some(&5));
}

#[test]
fn select_all_then_toggle_drops_one_row() {
    let mut list = list();
    list.select_all();
    list.toggle_selection(&4);
    assert_eq!(list.multi_selection(), &[1, 2, 3, 5, 6, 7, 8]);
}

#[test]
fn esc_after_freeze_clears_everything() {
    let mut list = list();
    list.select_target(&3);
    list.toggle_selection(&4);
    list.clear_selection();
    assert_eq!(list.multi_selection(), [] as [u8; 0]);
    assert!(!list.is_visual_mode());
}

/// The neutral visual-mode rule moved with the owner (add-gui-front-end
/// 4.3): the same key set the destination-side carrier matched before, now
/// over the neutral `KeyEvent`, with the mark count as its outcome.
#[test]
fn ctrl_a_selects_all_through_the_neutral_visual_key() {
    use mbv_ui_msg::{Key, KeyModifiers};

    let mut list = list();
    let key = mbv_ui_msg::KeyEvent::new(Key::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(list.handle_visual_key(&key).map(|o| o.selected), Some(8));
    assert_eq!(list.multi_selection(), &[1, 2, 3, 4, 5, 6, 7, 8]);
}
