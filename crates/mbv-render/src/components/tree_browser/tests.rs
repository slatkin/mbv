use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn paint_row(kind: TreePaintRowKind, title: &str, depth: usize) -> TreePaintRow {
    TreePaintRow {
        kind,
        title: String::from(title),
        secondary: None,
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

fn split_node(
    title: &str,
    secondary: &str,
    depth: usize,
    semantic_state: MediaSemanticState,
) -> TreePaintRow {
    let mut row = paint_row(TreePaintRowKind::Node, title, depth);
    row.secondary = Some(String::from(secondary));
    row.semantic_state = semantic_state;
    row
}

/// Paints one focused buffer with `rows` and returns it.
fn draw_rows(rows: &[TreePaintRow]) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    let mut marquee_text = String::new();
    let mut marquee_started_at = Instant::now();
    terminal
        .draw(|frame| {
            render_tree_browser(
                frame,
                Rect::new(0, 0, 40, 4),
                Rect::new(0, 0, 40, 4),
                rows,
                rows.len(),
                0,
                true,
                true,
                &mut marquee_text,
                &mut marquee_started_at,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

/// `tree_row_title` returns the indent width with the node title unchanged,
/// headings uppercased, and spacers empty (issue #896: pins the
/// allocation-hygiene rewrite against output drift).
#[test]
fn tree_row_title_outputs_match_legacy_rows_issue_896() {
    let node = paint_row(TreePaintRowKind::Node, "Dune", 2);
    let (prefix_width, title) = tree_row_title(&node);
    assert_eq!(prefix_width, 4);
    assert_eq!(&*title, "Dune");
    let heading = paint_row(TreePaintRowKind::Heading, "Shows", 1);
    let (prefix_width, title) = tree_row_title(&heading);
    assert_eq!(prefix_width, 0);
    assert_eq!(&*title, "SHOWS");
    let spacer = paint_row(TreePaintRowKind::Spacer, "", 0);
    let (prefix_width, title) = tree_row_title(&spacer);
    assert_eq!(prefix_width, 0);
    assert_eq!(&*title, "");
}

/// A split node paints the flat media list's split-row role policy instead
/// of the depth-colour fallback (home-recently-added-tree 2.2, D9): the
/// primary part in the playback-context role, the secondary in the
/// playback-title role. Depth two would otherwise paint aqua (`Accent`).
#[test]
fn a_split_node_paints_the_flat_split_row_roles() {
    let buffer = draw_rows(&[split_node(
        "Show",
        "Episode",
        2,
        MediaSemanticState::Ordinary,
    )]);
    assert_eq!(buffer[(4, 0)].symbol(), "S");

    for x in 4..8 {
        assert_eq!(
            buffer[(x, 0)].style().fg,
            Some(palette::Role::SplitRowContextFg.color()),
            "the primary part paints the playback-context role"
        );
    }
    assert_eq!(buffer[(8, 0)].symbol(), " ");
    for x in 9..16 {
        assert_eq!(
            buffer[(x, 0)].style().fg,
            Some(palette::Role::SplitRowTitleFg.color()),
            "the secondary part paints the playback-title role"
        );
    }
}

/// A played split node mutes only the secondary part; the primary keeps the
/// playback-context role (home-recently-added-tree 2.2, D9).
#[test]
fn a_played_split_node_mutes_only_its_secondary() {
    let buffer = draw_rows(&[split_node("Show", "Episode", 2, MediaSemanticState::Played)]);

    for x in 4..8 {
        assert_eq!(
            buffer[(x, 0)].style().fg,
            Some(palette::Role::SplitRowContextFg.color()),
            "the primary part keeps the playback-context role"
        );
    }
    for x in 9..16 {
        assert_eq!(
            buffer[(x, 0)].style().fg,
            Some(palette::Role::TextMuted.color()),
            "the secondary part paints muted"
        );
    }
}

/// A full-width split row — the focused selected row and a directly marked
/// row — carries the selection-bar foreground on every part (primary,
/// separator, secondary) exactly as the flat media list's selected-bar
/// recolor does, and both parts stay painted (marquee or not).
#[test]
fn a_full_width_split_row_carries_the_bar_on_both_parts() {
    let mut selected = split_node("Show", "Episode", 0, MediaSemanticState::Ordinary);
    selected.selected = true;
    let mut marked = split_node("Show", "Episode", 0, MediaSemanticState::Ordinary);
    marked.marked = true;
    let buffer = draw_rows(&[selected, marked]);
    let bar = palette::Role::SelectedRowFg.color();

    assert_eq!(buffer[(0, 0)].symbol(), "S");
    assert_eq!(buffer[(5, 0)].symbol(), "E");
    assert_eq!(
        buffer[(5, 0)].style().fg,
        Some(bar),
        "the selected split row paints its secondary in the bar foreground"
    );
    assert_eq!(
        buffer[(11, 1)].style().fg,
        Some(bar),
        "the marked split row paints its secondary in the bar foreground"
    );
}
