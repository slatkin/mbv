use super::super::{
    TreeBrowser, TreeEntry, TreeMarkPolicy, TreeNode, TreeOperation, TreeReconciliationError,
};
use super::{node, paint, Target};
use crate::app::components::media_list::MediaSemanticState;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::component::Component;

fn component_bound<T: Component>() {}
#[test]
fn generic_tree_browser_implements_tuirealm_component() {
    component_bound::<TreeBrowser<Target>>();
}

#[test]
fn right_expands_nodes_then_activates_only_an_expanded_root() {
    let mut browser = TreeBrowser::new();
    let root = node(Target::Root, None).with_expandable(true);
    browser
        .reconcile([
            root,
            node(Target::Branch, Some(Target::Root)).with_expandable(true),
            node(Target::Leaf, Some(Target::Branch)),
        ])
        .unwrap();

    let collapsed_root = browser.apply(TreeOperation::Right);
    assert_eq!(collapsed_root.external_intent, None);
    assert!(browser.is_expanded(&Target::Root));

    let expanded_root = browser.apply(TreeOperation::Right);
    assert_eq!(
        expanded_root.external_intent,
        Some(super::super::TreeExternalIntent::Activate(Target::Root))
    );
    assert!(browser.is_expanded(&Target::Root));

    browser.apply(TreeOperation::Select(Target::Branch));
    browser.apply(TreeOperation::Right);
    assert!(browser.is_expanded(&Target::Branch));
    let expanded_branch = browser.apply(TreeOperation::Right);
    assert_eq!(expanded_branch.external_intent, None);
    assert!(browser.is_expanded(&Target::Branch));
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

    let revision = browser.model_revision;
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 0)),
        Some(&Target::Root)
    );

    browser.reconcile(projection).unwrap();

    assert_eq!(browser.model_revision, revision);
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
    let revision = browser.model_revision;

    browser
        .reconcile([node(Target::Other, None), node(Target::Root, None)])
        .unwrap();

    assert_eq!(browser.model_revision, revision + 1);
    assert_eq!(browser.resolve_current_point(Position::new(2, 0)), None);

    paint(&mut browser);
    let revision = browser.model_revision;
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

    assert_eq!(browser.model_revision, revision + 1);
    assert_eq!(browser.resolve_current_point(Position::new(2, 0)), None);
}

#[test]
fn changing_heading_content_invalidates_retained_geometry() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            TreeEntry::Heading("Old heading".into()),
            TreeEntry::Node(node(Target::Root, None)),
        ])
        .unwrap();
    paint(&mut browser);
    let revision = browser.model_revision;

    browser
        .reconcile([
            TreeEntry::Heading("New heading".into()),
            TreeEntry::Node(node(Target::Root, None)),
        ])
        .unwrap();

    assert_eq!(browser.model_revision, revision + 1);
    assert!(!browser.has_completed_paint());
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
    browser
        .reconcile([
            TreeEntry::Heading("Retained group".into()),
            TreeEntry::Node(node(Target::Root, None)),
        ])
        .unwrap();
    browser.expanded.insert(Target::Root);
    browser.marks.add(Target::Root);
    browser.viewport_offset = 7;
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(1, 0, 18, 2)))
        .unwrap();
    let revision = browser.model_revision;
    let selected = browser.selected_target().cloned();
    let roots = browser.roots().into_iter().cloned().collect::<Vec<_>>();
    let expanded = browser.expanded.clone();
    let marks = browser.marks.clone();
    let viewport_offset = browser.viewport_offset;
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 1)),
        Some(&Target::Root)
    );

    let error = browser.reconcile(projection).unwrap_err();
    assert_eq!(error, expected);
    assert_eq!(browser.model_revision, revision);
    assert_eq!(browser.selected_target().cloned(), selected);
    assert_eq!(browser.expanded, expanded);
    assert_eq!(browser.marks, marks);
    assert_eq!(browser.viewport_offset, viewport_offset);
    assert_eq!(
        browser.roots().into_iter().cloned().collect::<Vec<_>>(),
        roots
    );
    assert_eq!(
        browser.resolve_current_point(Position::new(2, 1)),
        Some(&Target::Root)
    );
}

#[test]
fn structural_rows_share_the_flow_but_are_never_selectable_or_actionable() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            TreeEntry::Heading("First group".into()),
            TreeEntry::Node(node(Target::Root, None)),
            TreeEntry::Spacer,
            TreeEntry::Heading("Second group".into()),
            TreeEntry::Node(node(Target::Branch, None)),
            TreeEntry::Node(node(Target::Other, None)),
        ])
        .unwrap();
    browser.set_geometry(Rect::new(0, 0, 18, 2), Rect::new(0, 0, 18, 2));

    assert_eq!(browser.visible_len(), 6);
    assert_eq!(
        browser.visible_targets(),
        vec![Target::Root, Target::Branch, Target::Other]
    );
    assert_eq!(browser.selected_target(), Some(&Target::Root));
    paint(&mut browser);
    assert_eq!(browser.resolve_current_point(Position::new(2, 0)), None);
    assert_eq!(
        browser
            .apply(TreeOperation::PointerToggleMark(Position::new(2, 0)))
            .disposition,
        super::super::TreeConsumed::Unhandled
    );

    browser.apply(TreeOperation::Move(1));
    assert_eq!(browser.selected_target(), Some(&Target::Branch));
    browser.apply(TreeOperation::Page(1));
    assert_eq!(browser.selected_target(), Some(&Target::Other));
    browser.apply(TreeOperation::First);
    assert_eq!(browser.selected_target(), Some(&Target::Root));
    let transition = browser.apply(TreeOperation::ToggleExpansionTarget(Target::Missing));
    assert_eq!(
        transition.disposition,
        super::super::TreeConsumed::Unhandled
    );
    assert_eq!(transition.external_intent, None);
    assert_eq!(browser.marked_targets(), &[]);
}

#[test]
fn structural_refresh_preserves_stable_selection_and_clamps_viewport() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            TreeEntry::Heading("Group".into()),
            TreeEntry::Node(node(Target::Root, None)),
            TreeEntry::Spacer,
            TreeEntry::Node(node(Target::Branch, None)),
            TreeEntry::Node(node(Target::Other, None)),
        ])
        .unwrap();
    browser.set_geometry(Rect::new(0, 0, 18, 2), Rect::new(0, 0, 18, 2));
    browser.apply(TreeOperation::Last);
    assert_eq!(browser.selected_target(), Some(&Target::Other));
    assert!(browser.viewport_offset > 0);

    paint(&mut browser);
    browser
        .reconcile([
            TreeEntry::Node(node(Target::Branch, None)),
            TreeEntry::Node(node(Target::Root, None)),
            TreeEntry::Node(node(Target::Other, None)),
        ])
        .unwrap();

    assert_eq!(browser.selected_target(), Some(&Target::Other));
    assert!(browser.viewport_offset <= browser.visible_len().saturating_sub(2));
    assert!(!browser.has_completed_paint());

    browser
        .reconcile([
            TreeEntry::Heading("New group".into()),
            TreeEntry::Node(node(Target::Branch, None)),
            TreeEntry::Node(node(Target::Root, None)),
            TreeEntry::Node(node(Target::Other, None)),
        ])
        .unwrap();
    assert_eq!(browser.selected_target(), Some(&Target::Other));
    assert!(browser.viewport_offset <= browser.visible_len().saturating_sub(2));
}

#[test]
fn declared_expandability_retains_pending_expansion_and_reveals_children() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([node(Target::Root, None).with_expandable(true)])
        .unwrap();
    browser.viewport_offset = 1;

    let transition = browser.apply(TreeOperation::ToggleExpansionTarget(Target::Root));
    assert_eq!(transition.disposition, super::super::TreeConsumed::Consumed);
    assert!(browser.is_expanded(&Target::Root));
    assert_eq!(browser.visible_len(), 1);
    assert_eq!(browser.visible_targets(), vec![Target::Root]);

    browser
        .reconcile([
            node(Target::Root, None).with_expandable(true),
            node(Target::Branch, Some(Target::Root)),
        ])
        .unwrap();

    assert!(browser.is_expanded(&Target::Root));
    assert_eq!(
        browser.visible_targets(),
        vec![Target::Root, Target::Branch]
    );
    assert_eq!(browser.selected_target(), Some(&Target::Root));
    assert_eq!(browser.viewport_offset, 1);
}

#[rstest]
#[case::declaration_retained(true, false, true)]
#[case::children_arrived(false, true, true)]
#[case::empty_completion(false, false, false)]
fn reconciliation_keeps_pending_expansion_only_while_expandable_or_populated(
    #[case] declared_expandable: bool,
    #[case] has_child: bool,
    #[case] expected_expanded: bool,
) {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([node(Target::Root, None).with_expandable(true)])
        .unwrap();
    browser.apply(TreeOperation::ToggleExpansionTarget(Target::Root));
    assert!(browser.is_expanded(&Target::Root));

    let mut projection = vec![node(Target::Root, None).with_expandable(declared_expandable)];
    if has_child {
        projection.push(node(Target::Branch, Some(Target::Root)));
    }
    browser.reconcile(projection).unwrap();

    assert_eq!(browser.is_expanded(&Target::Root), expected_expanded);
    assert_eq!(
        browser.visible_targets(),
        if expected_expanded && has_child {
            vec![Target::Root, Target::Branch]
        } else {
            vec![Target::Root]
        }
    );
}

#[test]
fn undeclared_childless_music_node_remains_unhandled() {
    let mut browser = TreeBrowser::new();
    browser.reconcile([node(Target::Root, None)]).unwrap();

    assert_eq!(
        browser
            .apply(TreeOperation::ToggleExpansionTarget(Target::Root))
            .disposition,
        super::super::TreeConsumed::Unhandled
    );
    assert!(!browser.is_expanded(&Target::Root));
}

#[test]
fn heading_free_tree_keeps_music_row_flow_navigation_and_marks() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            node(Target::Root, None),
            node(Target::Branch, None),
            node(Target::Other, None),
        ])
        .unwrap();

    assert_eq!(browser.visible_len(), 3);
    assert_eq!(
        browser.visible_targets(),
        vec![Target::Root, Target::Branch, Target::Other]
    );
    browser.set_geometry(Rect::new(0, 0, 18, 3), Rect::new(0, 0, 18, 3));
    browser.apply(TreeOperation::Move(1));
    paint(&mut browser);
    browser.apply(TreeOperation::PointerToggleMark(Position::new(2, 1)));
    assert_eq!(browser.selected_target(), Some(&Target::Branch));
    assert_eq!(browser.marked_targets(), &[Target::Branch]);
    browser.apply(TreeOperation::Last);
    assert_eq!(browser.selected_target(), Some(&Target::Other));
    assert_eq!(browser.visible_len(), 3);
}
