use super::{
    MediaList, MediaListCarrier, MediaListDisposition, MediaListOperation, MediaListSurfaceInput,
    WideMediaList,
};
use crate::list::{ViewportAnchor, Viewported};
use mbv_render::components::media_list::{
    MediaKind, MediaListRow, MediaListTitleReveal, MediaSemanticState, WideMediaListPaintPolicy,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
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
fn refresh_preserves_target_and_resolves_a_missing_target_to_the_first_row() {
    let rows = vec![item("a"), item("b"), item("c"), item("d")];
    let mut list = WideMediaList::new();
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

#[test]
fn title_reveal_defaults_to_always_and_survives_a_content_refresh() {
    let mut carrier = MediaListCarrier::new();
    assert_eq!(
        carrier.wide().title_reveal(),
        MediaListTitleReveal::Always,
        "a list that declares nothing reveals every row's title"
    );
    carrier.set_title_reveal(MediaListTitleReveal::OnSelection);
    carrier.set_content(vec![item("one"), item("two")]);
    assert_eq!(
        carrier.wide().title_reveal(),
        MediaListTitleReveal::OnSelection,
        "an ordinary refresh keeps the list's declared policy"
    );
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

/// 2.3 (shared-list-components): a wheel gesture maps to `Scroll` and
/// scrolls a canonical list's viewport by the gesture delta while every
/// selection fact — cursor, multi-selection, live range — stays untouched.
#[test]
fn a_wheel_scroll_moves_the_viewport_and_leaves_the_selection_untouched() {
    let mut list = MediaList::new();
    list.set_content((1..=8).map(|target| item(&target.to_string())).collect());
    list.select_target(&"3".to_string());
    list.enter_visual_mode();
    list.toggle_selection(&"5".to_string());
    list.enter_visual_mode();
    assert_eq!(list.multi_selection(), &["3".to_string(), "5".to_string()]);
    assert!(list.live_range);

    let input = MediaListSurfaceInput::Wheel {
        at: Position { x: 0, y: 2 },
        delta: 3,
    };
    let operation = input
        .into_operation(None::<String>)
        .expect("resolved media-list pointer target");
    assert_eq!(operation, MediaListOperation::Scroll(3));

    let transition = list.delegate_operation(operation);

    assert_eq!(transition.disposition, MediaListDisposition::Consumed);
    assert_eq!(transition.selected_target, None);
    assert_eq!(transition.selection_summary, None);
    assert_eq!(transition.external_intent, None);
    assert_eq!(list.scroll(), 3);
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(list.selected_target().cloned(), Some("3".to_string()));
    assert_eq!(list.multi_selection(), &["3".to_string(), "5".to_string()]);
    assert!(list.live_range);
}

/// #745: a content-preserving reset keeps the projected rows but returns the
/// list to its normal initial presentation — first selectable row, top of the
/// viewport, no marks, and no retained painted frame.
#[test]
fn reset_presentation_keeps_rows_and_restores_initial_selection_and_viewport() {
    let mut carrier = MediaListCarrier::new();
    carrier.set_content(vec![item("a"), item("b"), item("c")]);
    carrier.select_target(&"c".to_string());
    carrier.set_scroll(2);
    carrier.toggle_selection(&"b".to_string());
    let area = Rect::new(2, 1, 12, 3);
    paint(carrier.wide_mut(), area);
    assert!(carrier.claims_current_point(Position { x: 2, y: 1 }));

    carrier.reset_presentation();

    assert_eq!(carrier.rows().len(), 3, "rows are retained");
    assert_eq!(carrier.selected_target(), Some(&"a".to_string()));
    assert_eq!(carrier.scroll(), 0);
    assert_eq!(carrier.multi_selection().len(), 0);
    assert!(
        !carrier.claims_current_point(Position { x: 2, y: 1 }),
        "the pre-reset painted geometry is invalidated"
    );
}

/// A target whose clones the test counts: the old wheel path cloned every
/// selectable target through `row_flow()` (#926), so a large list floods the
/// counter while a length-only scroll stays constant.
#[derive(Debug, PartialEq, Eq)]
struct Counted(u32);

thread_local! {
    static TARGET_CLONES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Clone for Counted {
    fn clone(&self) -> Self {
        TARGET_CLONES.with(|count| count.set(count.get() + 1));
        Self(self.0)
    }
}

fn counted_row(target: u32) -> MediaListRow<Counted> {
    MediaListRow::Item {
        target: Counted(target),
        primary: String::new(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// Regression (#926): the wheel's `Scroll` arm must never build the row
/// flow — the offset needs only the row count. Over 1,000 rows a notch
/// clones a bounded handful of targets (the transition's before/after
/// selection reads), where a flow build would clone all of them.
#[test]
fn a_wheel_scroll_never_builds_the_row_flow() {
    let mut list = MediaList::new();
    list.set_content((0..1000).map(counted_row).collect());
    TARGET_CLONES.with(|count| count.set(0));

    let transition = list.delegate_operation(MediaListOperation::Scroll(1));

    assert_eq!(transition.disposition, MediaListDisposition::Consumed);
    assert_eq!(list.viewport_offset(), 1);
    TARGET_CLONES.with(|count| {
        let clones = count.get();
        assert!(
            clones <= 4,
            "the scroll built the row flow: {clones} target clones"
        );
    });
}

/// Regression (#928): a wheel at the end of a painted list leaves the last
/// row at the bottom of the viewport, not above it. The clamp uses the wide
/// list's painted content height (5), so the offset stops at 20 - 5 = 15
/// instead of the one-row clamp's 19.
#[test]
fn a_wheel_scroll_stops_with_the_last_row_at_the_viewport_bottom() {
    let mut list = WideMediaList::new();
    list.set_content((0..20).map(|i| item(&i.to_string())).collect());
    paint(&mut list, Rect::new(0, 0, 10, 5));

    for _ in 0..16 {
        list.delegate_operation(MediaListOperation::Scroll(1));
    }

    assert_eq!(list.viewport_offset(), 15);
}
