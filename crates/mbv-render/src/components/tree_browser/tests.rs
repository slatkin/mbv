use super::*;

fn paint_row(kind: TreePaintRowKind, title: &str, depth: usize) -> TreePaintRow {
    TreePaintRow {
        kind,
        title: String::from(title),
        title_role: TreeTitleRole::Standard,
        trailing: None,
        depth,
        root_index: 0,
        group_root_index: 0,
        zebra_striped: false,
        selected: false,
        marked: false,
        aggregate_mark: TreeAggregateMark::None,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// `tree_row_title` returns the indent width with the title borrowed for
/// nodes, uppercased and owned for headings, and empty for spacers (issue
/// #896: pins the allocation-hygiene rewrite against output drift).
#[test]
fn tree_row_title_outputs_match_legacy_rows_issue_896() {
    let node = paint_row(TreePaintRowKind::Node, "Dune", 2);
    let (prefix_width, title) = tree_row_title(&node);
    assert_eq!(prefix_width, 4);
    assert!(matches!(title, Cow::Borrowed(_)));
    assert_eq!(&*title, "Dune");
    let heading = paint_row(TreePaintRowKind::Heading, "Shows", 1);
    let (prefix_width, title) = tree_row_title(&heading);
    assert_eq!(prefix_width, 0);
    assert!(matches!(title, Cow::Owned(_)));
    assert_eq!(&*title, "SHOWS");
    let spacer = paint_row(TreePaintRowKind::Spacer, "", 0);
    let (prefix_width, title) = tree_row_title(&spacer);
    assert_eq!(prefix_width, 0);
    assert!(matches!(title, Cow::Borrowed(_)));
    assert_eq!(&*title, "");
}
