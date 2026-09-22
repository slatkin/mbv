use super::{TreeBrowser, TreeMarkPolicy, TreeNode, TreeReconciliationError};
use crate::app::components::media_list::MediaSemanticState;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use rstest::rstest;
use std::hash::Hash;
use tuirealm::component::Component;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Target {
    Root,
    Branch,
    Leaf,
    Other,
}

fn node(target: Target, parent: Option<Target>) -> TreeNode<Target> {
    TreeNode::new(
        target,
        parent,
        "row",
        "row",
        MediaSemanticState::Ordinary,
        TreeMarkPolicy::Direct,
    )
}

fn component_bound<T: Component>() {}

fn paint(browser: &mut TreeBrowser<Target>) {
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| Component::view(browser, frame, Rect::new(1, 0, 18, 2)))
        .unwrap();
}

#[test]
fn generic_tree_browser_implements_tuirealm_component() {
    component_bound::<TreeBrowser<Target>>();
}

#[test]
fn three_level_forest_preserves_stable_selection_through_reorder_and_removal() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            node(Target::Root, None),
            node(Target::Branch, Some(Target::Root)),
            node(Target::Leaf, Some(Target::Branch)),
        ])
        .unwrap();
    assert_eq!(browser.selected_target(), Some(&Target::Root));
    assert_eq!(browser.roots(), vec![&Target::Root]);
    assert_eq!(
        browser.children_of(&Target::Root),
        Some(vec![&Target::Branch])
    );
    assert_eq!(
        browser.children_of(&Target::Branch),
        Some(vec![&Target::Leaf])
    );

    browser
        .reconcile([
            node(Target::Leaf, Some(Target::Branch)),
            node(Target::Root, None),
            node(Target::Branch, Some(Target::Root)),
        ])
        .unwrap();
    assert_eq!(browser.selected_target(), Some(&Target::Root));

    browser.reconcile([node(Target::Leaf, None)]).unwrap();
    assert_eq!(browser.selected_target(), Some(&Target::Leaf));
}

#[test]
fn identical_projection_preserves_revision_and_retained_geometry() {
    let projection = [node(Target::Root, None)];
    let mut browser = TreeBrowser::new();
    browser.reconcile(projection.clone()).unwrap();
    paint(&mut browser);

    let revision = browser.model_revision();
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 0)),
        Some(&Target::Root)
    );

    browser.reconcile(projection).unwrap();

    assert_eq!(browser.model_revision(), revision);
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 0)),
        Some(&Target::Root)
    );
}

#[test]
fn changed_projection_bumps_revision_and_invalidates_retained_geometry() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([node(Target::Root, None), node(Target::Other, None)])
        .unwrap();
    paint(&mut browser);
    let revision = browser.model_revision();

    browser
        .reconcile([node(Target::Other, None), node(Target::Root, None)])
        .unwrap();

    assert_eq!(browser.model_revision(), revision + 1);
    assert_eq!(browser.resolve_current_point(Position::new(2, 0)), None);

    paint(&mut browser);
    let revision = browser.model_revision();
    let changed = TreeNode::new(
        Target::Other,
        None,
        "changed",
        "row",
        MediaSemanticState::Ordinary,
        TreeMarkPolicy::Direct,
    );
    browser
        .reconcile([changed, node(Target::Root, None)])
        .unwrap();

    assert_eq!(browser.model_revision(), revision + 1);
    assert_eq!(browser.resolve_current_point(Position::new(2, 0)), None);
}

#[rstest]
#[case::duplicate(
    vec![node(Target::Root, None), node(Target::Root, None)],
    TreeReconciliationError::DuplicateTarget { target: Target::Root }
)]
#[case::missing_parent(
    vec![node(Target::Leaf, Some(Target::Other))],
    TreeReconciliationError::MissingParent { target: Target::Leaf, parent: Target::Other }
)]
#[case::self_parent(
    vec![node(Target::Root, Some(Target::Root))],
    TreeReconciliationError::SelfParent { target: Target::Root }
)]
#[case::cycle(
    vec![node(Target::Root, Some(Target::Branch)), node(Target::Branch, Some(Target::Root))],
    TreeReconciliationError::Cycle { target: Target::Root }
)]
fn invalid_projection_returns_typed_error_without_mutating_owner(
    #[case] projection: Vec<TreeNode<Target>>,
    #[case] expected: TreeReconciliationError<Target>,
) {
    let mut browser = TreeBrowser::new();
    browser.reconcile([node(Target::Root, None)]).unwrap();
    browser.expanded.insert(Target::Root);
    browser.marks.add(Target::Root);
    browser.viewport_offset = 7;
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(1, 0, 18, 2)))
        .unwrap();
    let revision = browser.model_revision();
    let selected = browser.selected_target().cloned();
    let roots = browser.roots().into_iter().cloned().collect::<Vec<_>>();
    let expanded = browser.expanded.clone();
    let marks = browser.marks.clone();
    let viewport_offset = browser.viewport_offset;
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 0)),
        Some(&Target::Root)
    );

    let error = browser.reconcile(projection).unwrap_err();
    assert_eq!(error, expected);
    assert_eq!(browser.model_revision(), revision);
    assert_eq!(browser.selected_target().cloned(), selected);
    assert_eq!(browser.expanded, expanded);
    assert_eq!(browser.marks, marks);
    assert_eq!(browser.viewport_offset, viewport_offset);
    assert_eq!(
        browser.roots().into_iter().cloned().collect::<Vec<_>>(),
        roots
    );
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 0)),
        Some(&Target::Root)
    );
}

fn named_node(
    target: Target,
    parent: Option<Target>,
    title: &str,
    search_text: &str,
    policy: TreeMarkPolicy,
) -> TreeNode<Target> {
    TreeNode::new(
        target,
        parent,
        title,
        search_text,
        MediaSemanticState::Ordinary,
        policy,
    )
}

#[test]
fn apply_owns_clamped_tree_navigation_and_parent_child_traversal() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            node(Target::Root, None),
            node(Target::Branch, Some(Target::Root)),
            node(Target::Leaf, Some(Target::Branch)),
            node(Target::Other, None),
        ])
        .unwrap();
    browser.set_geometry(Rect::new(0, 0, 10, 2), Rect::new(0, 0, 10, 2));

    assert_eq!(
        browser
            .apply(super::TreeOperation::Move(-1))
            .selected_target,
        Some(Target::Root)
    );
    browser.apply(super::TreeOperation::ToggleExpansion);
    assert_eq!(
        browser.apply(super::TreeOperation::Child).selected_target,
        Some(Target::Branch)
    );
    browser.apply(super::TreeOperation::ToggleExpansion);
    assert_eq!(
        browser.apply(super::TreeOperation::Child).selected_target,
        Some(Target::Leaf)
    );
    assert_eq!(
        browser.apply(super::TreeOperation::Parent).selected_target,
        Some(Target::Branch)
    );
    browser.apply(super::TreeOperation::Last);
    assert_eq!(browser.selected_target(), Some(&Target::Other));
    assert!(browser.viewport_offset() <= 2);
}

#[test]
fn anchor_selection_counts_all_levels_in_the_settled_flow() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            node(Target::Root, None),
            node(Target::Branch, Some(Target::Root)),
            node(Target::Leaf, Some(Target::Branch)),
            node(Target::Other, None),
        ])
        .unwrap();

    browser.apply(super::TreeOperation::AnchorSelection {
        target: Target::Leaf,
        flow_offset: 2,
    });

    assert_eq!(browser.selected_target(), Some(&Target::Leaf));
    assert_eq!(browser.viewport_offset(), 2);
    assert_eq!(
        browser.visible_targets(),
        vec![Target::Root, Target::Branch, Target::Leaf, Target::Other]
    );
}

#[test]
fn filter_matching_forces_visibility_without_persisting_expansion_and_restores_anchor() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(
                Target::Root,
                None,
                "Alpha",
                "alpha",
                TreeMarkPolicy::Aggregate,
            ),
            named_node(
                Target::Branch,
                Some(Target::Root),
                "Album",
                "album",
                TreeMarkPolicy::Excluded,
            ),
            named_node(
                Target::Leaf,
                Some(Target::Branch),
                "Track",
                "track",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();
    browser.apply(super::TreeOperation::ToggleExpansion);
    browser.apply(super::TreeOperation::Child);
    let anchor = browser.selected_target().cloned();
    browser.apply(super::TreeOperation::EditFilter("track".into()));
    assert_eq!(browser.filter_query(), "track");
    assert!(browser.is_expanded(&Target::Root));
    assert!(!browser.is_expanded(&Target::Branch));
    assert!(browser
        .visible_node_ids()
        .contains(&browser.target_to_node[&Target::Leaf]));
    browser.apply(super::TreeOperation::ClearFilter);
    assert_eq!(browser.selected_target(), anchor.as_ref());
    assert!(browser.is_expanded(&Target::Root));
}

#[test]
fn marking_filter_hidden_target_reconciles_selection_to_visible_row() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(
                Target::Root,
                None,
                "Visible",
                "visible",
                TreeMarkPolicy::Direct,
            ),
            named_node(
                Target::Other,
                None,
                "Hidden",
                "hidden",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();

    browser.apply(super::TreeOperation::EditFilter("visible".into()));
    let transition = browser.apply(super::TreeOperation::ToggleMarkTarget(Target::Other));

    assert_eq!(browser.marked_targets(), &[Target::Other]);
    assert_eq!(transition.selected_target, Some(Target::Root));
    assert_eq!(browser.selected_target(), Some(&Target::Root));
}

#[test]
fn marks_aggregate_and_context_use_visible_display_order() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(
                Target::Root,
                None,
                "Root",
                "root",
                TreeMarkPolicy::Aggregate,
            ),
            named_node(
                Target::Branch,
                Some(Target::Root),
                "Excluded",
                "excluded",
                TreeMarkPolicy::Excluded,
            ),
            named_node(
                Target::Leaf,
                Some(Target::Root),
                "Leaf",
                "leaf",
                TreeMarkPolicy::Direct,
            ),
            named_node(
                Target::Other,
                None,
                "Other",
                "other",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();
    browser.apply(super::TreeOperation::ToggleExpansion);
    browser.apply(super::TreeOperation::Select(Target::Leaf));
    let transition = browser.apply(super::TreeOperation::ToggleMark);
    assert_eq!(transition.mark_summary.unwrap().marked_count, 1);
    browser.apply(super::TreeOperation::Select(Target::Other));
    browser.apply(super::TreeOperation::ToggleMark);
    let context = browser.apply(super::TreeOperation::Context);
    assert_eq!(
        context.external_intent,
        Some(super::TreeExternalIntent::ContextSelection(vec![
            Target::Leaf,
            Target::Other
        ]))
    );
    browser.apply(super::TreeOperation::Select(Target::Root));
    browser.apply(super::TreeOperation::ToggleMark);
    assert_eq!(browser.marked_targets(), &[Target::Other]);
    assert_eq!(
        browser
            .apply(super::TreeOperation::ToggleMarkTarget(Target::Branch))
            .disposition,
        super::TreeConsumed::Unhandled
    );
}

#[test]
fn one_operation_reports_selection_and_mark_changes_together() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(Target::Root, None, "Root", "root", TreeMarkPolicy::Direct),
            named_node(
                Target::Other,
                None,
                "Other",
                "other",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();

    let transition = browser.apply(super::TreeOperation::ToggleMarkTarget(Target::Other));
    assert_eq!(transition.disposition, super::TreeConsumed::Consumed);
    assert_eq!(transition.selected_target, Some(Target::Other));
    assert!(transition.selected_target_change.is_some());
    assert_eq!(transition.mark_summary.unwrap().marked_count, 1);
    assert!(transition.mark_summary_change.is_some());
}

#[test]
fn pointer_operations_use_the_latest_completed_frame() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(
                Target::Branch,
                None,
                "Branch",
                "branch",
                TreeMarkPolicy::Excluded,
            ),
            named_node(
                Target::Other,
                None,
                "Other",
                "other",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();
    // The row flow has two painted rows and the cursor starts on row 0.
    assert_eq!(browser.selected_target(), Some(&Target::Branch));
    paint(&mut browser);

    // A pointer toggle resolves the painted row and moves the cursor to it
    // before marking, so the cursor is left on the row that was clicked.
    let transition = browser.apply(super::TreeOperation::PointerToggleMark(Position::new(2, 1)));
    assert_eq!(transition.disposition, super::TreeConsumed::Consumed);
    assert_eq!(
        transition.selected_target_change,
        Some(super::TreeSelectionChange {
            previous: Some(Target::Branch),
            current: Some(Target::Other),
        })
    );
    assert_eq!(browser.selected_target(), Some(&Target::Other));
    assert_eq!(browser.marked_targets(), &[Target::Other]);
    assert_eq!(transition.external_intent, None);

    // An `Excluded` row (a cached track) is not markable: the click still takes
    // the cursor and reports no mark change and no request.
    paint(&mut browser);
    let excluded = browser.apply(super::TreeOperation::PointerToggleMark(Position::new(2, 0)));
    assert_eq!(excluded.disposition, super::TreeConsumed::Consumed);
    assert_eq!(
        excluded.selected_target_change,
        Some(super::TreeSelectionChange {
            previous: Some(Target::Other),
            current: Some(Target::Branch),
        })
    );
    assert_eq!(browser.selected_target(), Some(&Target::Branch));
    assert_eq!(browser.marked_targets(), &[Target::Other]);
    assert!(excluded.mark_summary_change.is_none());
    assert_eq!(excluded.external_intent, None);

    assert!(browser.resolve_current_point(Position::new(2, 1)).is_none());
}

#[test]
fn shared_view_paints_depth_metadata_bars_and_scrollbar_without_state_glyphs() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(
                Target::Root,
                None,
                "Root",
                "root",
                TreeMarkPolicy::Aggregate,
            ),
            named_node(
                Target::Branch,
                Some(Target::Root),
                "Branch",
                "branch",
                TreeMarkPolicy::Excluded,
            ),
            named_node(
                Target::Leaf,
                Some(Target::Branch),
                "Leaf",
                "leaf",
                TreeMarkPolicy::Direct,
            )
            .with_trailing("2026"),
            named_node(
                Target::Other,
                None,
                "Other",
                "other",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();
    browser.set_geometry(Rect::new(1, 0, 18, 2), Rect::new(3, 0, 14, 2));
    browser.apply(super::TreeOperation::ToggleExpansion);
    browser.apply(super::TreeOperation::Child);
    browser.apply(super::TreeOperation::ToggleExpansion);
    browser.apply(super::TreeOperation::Child);
    browser.apply(super::TreeOperation::ToggleMark);

    let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(1, 0, 18, 2)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let selected = browser.selected_row_rect().expect("selected row retained");
    assert_eq!(
        buffer[(selected.x, selected.y)].bg,
        crate::app::palette::SELECTED_ROW_BG
    );
    assert!(buffer.content().iter().any(|cell| cell.symbol() == "2"));
    assert!(buffer
        .content()
        .iter()
        .all(|cell| { cell.symbol() != "▸" && cell.symbol() != "▾" }));
    assert!(
        buffer[(19, 0)].fg == crate::app::palette::SCROLLBAR
            || buffer[(19, 1)].fg == crate::app::palette::SCROLLBAR
    );
    assert!(browser
        .resolve_current_point(Position::new(2, selected.y))
        .is_some());
    assert!(browser.resolve_current_point(Position::new(2, 2)).is_none());
}

#[test]
fn ordinary_row_keeps_trailing_metadata_inside_inset_content_area() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            named_node(
                Target::Root,
                None,
                "Ordinary",
                "ordinary",
                TreeMarkPolicy::Direct,
            )
            .with_trailing("2026"),
            named_node(
                Target::Other,
                None,
                "Other",
                "other",
                TreeMarkPolicy::Direct,
            ),
        ])
        .unwrap();
    browser.set_geometry(Rect::new(1, 0, 18, 2), Rect::new(5, 0, 14, 2));
    browser.apply(super::TreeOperation::Select(Target::Other));

    let mut terminal = Terminal::new(TestBackend::new(24, 4)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(1, 0, 18, 2)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let ordinary_row = (5..19)
        .map(|x| buffer[(x, 0)].symbol())
        .collect::<Vec<_>>()
        .concat();

    assert!(
        ordinary_row.contains("2026"),
        "ordinary row: {ordinary_row:?}"
    );
}
