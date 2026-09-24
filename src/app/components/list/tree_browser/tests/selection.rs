use super::super::{TreeBrowser, TreeMarkPolicy};
use super::{named_node, node, paint, Target};
use ratatui::layout::{Position, Rect};
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
            .apply(super::super::TreeOperation::Move(-1))
            .selected_target,
        Some(Target::Root)
    );
    browser.apply(super::super::TreeOperation::ToggleExpansion);
    assert_eq!(
        browser
            .apply(super::super::TreeOperation::Child)
            .selected_target,
        Some(Target::Branch)
    );
    browser.apply(super::super::TreeOperation::ToggleExpansion);
    assert_eq!(
        browser
            .apply(super::super::TreeOperation::Child)
            .selected_target,
        Some(Target::Leaf)
    );
    assert_eq!(
        browser
            .apply(super::super::TreeOperation::Parent)
            .selected_target,
        Some(Target::Branch)
    );
    browser.apply(super::super::TreeOperation::Last);
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

    browser.apply(super::super::TreeOperation::AnchorSelection {
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
    browser.apply(super::super::TreeOperation::ToggleExpansion);
    browser.apply(super::super::TreeOperation::Child);
    let anchor = browser.selected_target().cloned();
    browser.apply(super::super::TreeOperation::EditFilter("track".into()));
    assert_eq!(browser.filter_query(), "track");
    assert!(browser.is_expanded(&Target::Root));
    assert!(!browser.is_expanded(&Target::Branch));
    assert!(browser
        .visible_node_ids()
        .contains(&browser.target_to_node[&Target::Leaf]));
    browser.apply(super::super::TreeOperation::ClearFilter);
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

    browser.apply(super::super::TreeOperation::EditFilter("visible".into()));
    let transition = browser.apply(super::super::TreeOperation::ToggleMarkTarget(Target::Other));

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
    browser.apply(super::super::TreeOperation::ToggleExpansion);
    browser.apply(super::super::TreeOperation::Select(Target::Leaf));
    let transition = browser.apply(super::super::TreeOperation::ToggleMark);
    assert_eq!(transition.mark_summary.unwrap().marked_count, 1);
    browser.apply(super::super::TreeOperation::Select(Target::Other));
    browser.apply(super::super::TreeOperation::ToggleMark);
    let context = browser.apply(super::super::TreeOperation::Context);
    assert_eq!(
        context.external_intent,
        Some(super::super::TreeExternalIntent::ContextSelection(vec![
            Target::Leaf,
            Target::Other
        ]))
    );
    browser.apply(super::super::TreeOperation::Select(Target::Root));
    browser.apply(super::super::TreeOperation::ToggleMark);
    assert_eq!(browser.marked_targets(), &[Target::Other]);
    assert_eq!(
        browser
            .apply(super::super::TreeOperation::ToggleMarkTarget(
                Target::Branch
            ))
            .disposition,
        super::super::TreeConsumed::Unhandled
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

    let transition = browser.apply(super::super::TreeOperation::ToggleMarkTarget(Target::Other));
    assert_eq!(transition.disposition, super::super::TreeConsumed::Consumed);
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
    let transition = browser.apply(super::super::TreeOperation::PointerToggleMark(
        Position::new(2, 1),
    ));
    assert_eq!(transition.disposition, super::super::TreeConsumed::Consumed);
    assert_eq!(
        transition.selected_target_change,
        Some(super::super::TreeSelectionChange {
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
    let excluded = browser.apply(super::super::TreeOperation::PointerToggleMark(
        Position::new(2, 0),
    ));
    assert_eq!(excluded.disposition, super::super::TreeConsumed::Consumed);
    assert_eq!(
        excluded.selected_target_change,
        Some(super::super::TreeSelectionChange {
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
