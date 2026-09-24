use super::{TreeBrowser, TreeMarkPolicy, TreeNode};
use crate::app::components::media_list::MediaSemanticState;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
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

mod painting;
mod selection;
mod structure;

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
