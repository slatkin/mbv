use super::{TreeBrowser, TreeMarkPolicy, TreeNode};
use mbv_render::components::media_list::MediaSemanticState;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use std::hash::Hash;
use tuirealm::component::Component;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Target {
    Root,
    Branch,
    Leaf,
    Other,
    Missing,
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

fn paint(browser: &mut TreeBrowser<Target>) {
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| Component::view(browser, frame, Rect::new(1, 0, 18, 2)))
        .unwrap();
}

mod selection;
mod structure;

/// A split node's secondary part joins the search text, so the tree filter
/// matches either part of the row (home-recently-added-tree 2.1).
#[test]
fn a_secondary_title_part_joins_the_search_text() {
    let node = node(Target::Leaf, None).with_secondary("Episode");
    assert_eq!(node.search_text, "row Episode");
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
