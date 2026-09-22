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
    browser.marks.push(Target::Root);
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
