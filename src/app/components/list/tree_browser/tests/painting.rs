use super::super::{TreeBrowser, TreeEntry, TreeMarkPolicy, TreeOperation};
use super::{named_node, node, Target};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tuirealm::component::Component;
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
    browser.apply(super::super::TreeOperation::ToggleExpansionTarget(
        Target::Root,
    ));
    browser.apply(super::super::TreeOperation::Select(Target::Branch));
    browser.apply(super::super::TreeOperation::ToggleExpansionTarget(
        Target::Branch,
    ));
    browser.apply(super::super::TreeOperation::Select(Target::Leaf));

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
fn grouped_tree_stripes_items_only_and_reset_at_each_heading() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            TreeEntry::Heading("First group".into()),
            TreeEntry::Node(node(Target::Root, None)),
            TreeEntry::Node(node(Target::Branch, None)),
            TreeEntry::Spacer,
            TreeEntry::Heading("Second group".into()),
            TreeEntry::Node(node(Target::Other, None)),
            TreeEntry::Node(node(Target::Missing, None)),
        ])
        .unwrap();
    browser.focused = false;

    let mut terminal = Terminal::new(TestBackend::new(20, 8)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(0, 0, 20, 8)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let zebra =
        crate::app::palette::surface_colors(crate::app::palette::Surface::QueueColumn, false).fill;
    let base =
        crate::app::palette::surface_colors(crate::app::palette::Surface::QueuePanel, false).fill;

    for (row, expected) in [
        (0, base), // heading
        (1, zebra),
        (2, base),
        (3, base),  // spacers use the base fill and do not affect item parity
        (4, base),  // heading
        (5, zebra), // parity resets at the heading
        (6, base),
    ] {
        assert_eq!(buffer[(2, row)].bg, expected, "row {row}");
    }

    browser.viewport_offset = 5;
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(0, 0, 20, 2)))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(2, 0)].bg, zebra);
}

#[test]
fn grouped_tree_zebra_parity_survives_scrolling_past_its_heading() {
    let mut browser = TreeBrowser::new();
    browser
        .reconcile([
            TreeEntry::Heading("First group".into()),
            TreeEntry::Node(node(Target::Root, None)),
            TreeEntry::Spacer,
            TreeEntry::Heading("Second group".into()),
            TreeEntry::Node(node(Target::Branch, None)),
        ])
        .unwrap();
    browser.apply(TreeOperation::Select(Target::Branch));
    browser.focused = false;

    let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(0, 0, 20, 5)))
        .unwrap();
    let zebra =
        crate::app::palette::surface_colors(crate::app::palette::Surface::QueueColumn, false).fill;
    assert_eq!(terminal.backend().buffer()[(0, 4)].bg, zebra);

    browser.viewport_offset = 4;
    terminal
        .draw(|frame| Component::view(&mut browser, frame, Rect::new(0, 0, 20, 1)))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].bg, zebra);
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
    browser.apply(super::super::TreeOperation::Select(Target::Other));

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
