use super::test_helpers::buffer_to_string;
use super::*;
use crate::app::components::list::tree_browser::{
    TreeAggregateMark, TreeBrowser, TreeEntry, TreeMarkPolicy, TreeNode, TreeOperation,
    TreePaintRow, TreePaintRowKind, TreeTitleRole,
};
use crate::app::components::media_list::MediaSemanticState;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use ratatui::Terminal;
use rstest::rstest;
use std::hash::Hash;
use std::time::Instant;
use tuirealm::component::Component;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Target {
    Alpha,
    Beta,
    Gamma,
    Delta,
    Epsilon,
    Zeta,
}

fn node(target: Target, title: &str) -> TreeNode<Target> {
    TreeNode::new(
        target,
        None,
        title,
        title,
        MediaSemanticState::Ordinary,
        TreeMarkPolicy::Direct,
    )
}

fn grouped_tree() -> TreeBrowser<Target> {
    let mut tree = TreeBrowser::new();
    tree.reconcile([
        TreeEntry::Heading("Alpha group".into()),
        TreeEntry::Node(node(Target::Alpha, "Alpha show")),
        TreeEntry::Spacer,
        TreeEntry::Heading("Beta group".into()),
        TreeEntry::Node(node(Target::Beta, "Beta show")),
        TreeEntry::Node(node(Target::Gamma, "Gamma show")),
    ])
    .unwrap();
    tree
}

fn draw<Target: Clone + Eq + Hash>(
    tree: &mut TreeBrowser<Target>,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(tree, frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

#[test]
fn tree_browser_paints_heading_and_spacer_rows_with_shared_group_semantics() {
    let mut tree = grouped_tree();
    tree.set_focused(false);
    tree.set_geometry(Rect::new(0, 0, 42, 6), Rect::new(2, 0, 40, 6));
    let terminal = draw(&mut tree, 42, 6);
    let buffer = terminal.backend().buffer();
    let output = buffer_to_string(&terminal);

    assert!(output.lines().next().unwrap().starts_with("  ALPHA GROUP"));
    assert!(output.lines().nth(2).unwrap().trim().is_empty());
    assert!(output.lines().nth(3).unwrap().starts_with("  BETA GROUP"));
    assert!(output.lines().nth(4).unwrap().contains("Beta show"));

    assert_eq!(buffer[(2, 0)].fg, palette::TEXT_METADATA);
    assert!(buffer[(2, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(10, 0)].bg, buffer[(10, 2)].bg);
    assert_eq!(buffer[(10, 2)].bg, buffer[(10, 3)].bg);
    assert_eq!(buffer[(10, 1)].bg, buffer[(10, 4)].bg);
    assert_ne!(buffer[(10, 2)].bg, buffer[(10, 4)].bg);
}

#[test]
fn tree_spacers_keep_the_surface_fill_in_both_group_parity_contexts() {
    let spacer = |root_index| TreePaintRow {
        kind: TreePaintRowKind::Spacer,
        title: String::new(),
        title_role: TreeTitleRole::Standard,
        trailing: None,
        depth: 0,
        root_index,
        group_root_index: 0,
        zebra_striped: false,
        selected: false,
        marked: false,
        aggregate_mark: TreeAggregateMark::None,
        semantic_state: MediaSemanticState::Ordinary,
    };
    let rows = [spacer(0), spacer(1)];
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).unwrap();
    let mut marquee_text = String::new();
    let mut marquee_started_at = Instant::now();
    terminal
        .draw(|frame| {
            render_tree_browser(
                frame,
                Rect::new(0, 0, 12, 2),
                Rect::new(0, 0, 12, 2),
                &rows,
                rows.len(),
                0,
                false,
                &mut marquee_text,
                &mut marquee_started_at,
            );
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let base = palette::surface_colors(palette::Surface::QueuePanel, false).fill;
    assert_eq!(buffer[(0, 0)].bg, base, "even group parity spacer");
    assert_eq!(buffer[(0, 1)].bg, base, "odd group parity spacer");
}

#[test]
fn grouped_tree_played_rows_use_muted_semantics_before_depth_roles() {
    let mut tree = TreeBrowser::new();
    tree.reconcile([
        TreeEntry::Heading("Shows".into()),
        TreeEntry::Node(TreeNode::new(
            Target::Alpha,
            None,
            "Played show",
            "Played show",
            MediaSemanticState::Played,
            TreeMarkPolicy::Direct,
        )),
        TreeEntry::Node(TreeNode::new(
            Target::Beta,
            Some(Target::Alpha),
            "Played season",
            "Played season",
            MediaSemanticState::Played,
            TreeMarkPolicy::Direct,
        )),
        TreeEntry::Node(TreeNode::new(
            Target::Gamma,
            None,
            "Unplayed show",
            "Unplayed show",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Direct,
        )),
        TreeEntry::Node(TreeNode::new(
            Target::Delta,
            Some(Target::Alpha),
            "Unplayed season",
            "Unplayed season",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Direct,
        )),
        TreeEntry::Node(TreeNode::new(
            Target::Epsilon,
            Some(Target::Beta),
            "Played episode",
            "Played episode",
            MediaSemanticState::Played,
            TreeMarkPolicy::Direct,
        )),
        TreeEntry::Node(TreeNode::new(
            Target::Zeta,
            Some(Target::Delta),
            "Unplayed episode",
            "Unplayed episode",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Direct,
        )),
    ])
    .unwrap();
    tree.apply(TreeOperation::ToggleExpansionTarget(Target::Alpha));
    tree.apply(TreeOperation::ToggleExpansionTarget(Target::Beta));
    tree.apply(TreeOperation::ToggleExpansionTarget(Target::Delta));
    tree.apply(TreeOperation::ToggleExpansionTarget(Target::Gamma));
    tree.set_geometry(Rect::new(0, 0, 42, 7), Rect::new(2, 0, 40, 7));
    tree.set_focused(false);

    let terminal = draw(&mut tree, 42, 7);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(2, 1)].fg, palette::TEXT_MUTED);
    assert_eq!(buffer[(4, 2)].fg, palette::TEXT_MUTED);
    assert_eq!(buffer[(6, 3)].fg, palette::TEXT_MUTED);
    assert_eq!(buffer[(4, 4)].fg, palette::TEXT_FOCUS_ACCENT);
    assert_eq!(buffer[(6, 5)].fg, palette::ACCENT);
    assert_eq!(buffer[(2, 6)].fg, palette::TEXT_EMPHASIS);

    tree.apply(TreeOperation::Select(Target::Epsilon));
    tree.set_focused(true);
    let terminal = draw(&mut tree, 42, 7);
    assert_eq!(
        terminal.backend().buffer()[(6, 3)].fg,
        palette::SELECTED_ROW_FG,
        "the selected-row foreground takes precedence over played semantics"
    );
}

#[test]
fn heading_free_tree_keeps_its_existing_row_flow_and_pixels() {
    let mut tree = TreeBrowser::new();
    tree.reconcile([
        node(Target::Alpha, "Alpha show"),
        node(Target::Beta, "Beta show"),
    ])
    .unwrap();
    let terminal = draw(&mut tree, 42, 2);
    let output = buffer_to_string(&terminal);

    assert!(output.lines().next().unwrap().contains("Alpha show"));
    assert!(output.lines().nth(1).unwrap().contains("Beta show"));
    assert_eq!(tree.visible_targets().len(), 2);
}

#[test]
fn scrolled_completed_paint_counts_structural_rows_in_the_viewport_offset() {
    let mut tree = grouped_tree();
    tree.set_geometry(Rect::new(0, 0, 42, 2), Rect::new(0, 0, 42, 2));
    tree.apply(TreeOperation::Select(Target::Beta));
    let _terminal = draw(&mut tree, 42, 2);

    assert_eq!(tree.resolve_current_point(Position::new(20, 0)), None);
    assert_eq!(
        tree.resolve_current_point(Position::new(20, 1)),
        Some(&Target::Beta)
    );
}

#[rstest]
#[case::wide_grouped(42, true)]
#[case::narrow_grouped(12, true)]
#[case::wide_ungrouped(42, false)]
#[case::narrow_ungrouped(12, false)]
fn completed_paint_resolves_rows_around_structures_at_each_width(
    #[case] width: u16,
    #[case] grouped: bool,
) {
    let mut tree = if grouped {
        grouped_tree()
    } else {
        let mut tree = TreeBrowser::new();
        tree.reconcile([
            node(Target::Alpha, "Alpha show"),
            node(Target::Beta, "Beta show"),
            node(Target::Gamma, "Gamma show"),
        ])
        .unwrap();
        tree
    };
    let height = if grouped { 6 } else { 3 };
    let _terminal = draw(&mut tree, width, height);

    if grouped {
        for (row, expected) in [(1, Target::Alpha), (4, Target::Beta), (5, Target::Gamma)] {
            let point = Position::new(width / 2, row);
            assert_eq!(tree.resolve_current_point(point), Some(&expected));
            tree.apply(TreeOperation::Select(expected.clone()));
            assert_eq!(tree.selected_target(), Some(&expected));
            let _terminal = draw(&mut tree, width, height);
        }
        for row in [0, 2, 3] {
            let point = Position::new(width / 2, row);
            assert_eq!(tree.resolve_current_point(point), None);
        }
    } else {
        for (row, expected) in [(0, Target::Alpha), (1, Target::Beta), (2, Target::Gamma)] {
            let point = Position::new(width / 2, row);
            assert_eq!(tree.resolve_current_point(point), Some(&expected));
            tree.apply(TreeOperation::Select(expected.clone()));
            assert_eq!(tree.selected_target(), Some(&expected));
            let _terminal = draw(&mut tree, width, height);
        }
    }
}
