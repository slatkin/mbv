use super::test_helpers::lifecycle_item;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tuirealm::component::Component;

#[test]
fn grid_preserves_two_column_cells_and_resolves_stable_targets() {
    use super::{GridMediaList, GridPaintPolicy};
    let mut grid = GridMediaList::new();
    grid.set_columns(2, 8, 2);
    grid.set_content(vec![
        lifecycle_item("a"),
        lifecycle_item("b"),
        lifecycle_item("c"),
    ]);
    grid.set_geometry(Rect::new(0, 0, 20, 2), Rect::new(1, 1, 18, 2));
    let mut terminal = Terminal::new(TestBackend::new(24, 5)).unwrap();
    terminal
        .draw(|frame| {
            grid.set_paint_policy(GridPaintPolicy::new(true));
            Component::view(&mut grid, frame, Rect::new(0, 0, 20, 2));
        })
        .unwrap();
    let cells = grid.current_cells().expect("completed grid frame");
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[0].rect.y, cells[1].rect.y);
    assert_eq!(cells[1].rect.x, cells[0].rect.right() + 2);
    assert_eq!(
        grid.resolve_current_point(Position::new(cells[1].rect.x, cells[1].rect.y)),
        Some(&"b".to_string())
    );
    assert_eq!(
        grid.resolve_current_point(Position::new(cells[2].rect.x, cells[2].rect.y)),
        Some(&"c".to_string())
    );
}

#[test]
fn grid_movement_keeps_selected_target_in_the_painted_window() {
    use super::{GridMediaList, GridPaintPolicy};
    let mut grid = GridMediaList::new();
    grid.set_columns(2, 8, 2);
    grid.set_content((0..30).map(|n| lifecycle_item(&n.to_string())).collect());
    let area = Rect::new(0, 0, 20, 5);
    let mut terminal = Terminal::new(TestBackend::new(24, 7)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut grid, frame, area))
        .unwrap();
    let outcome = grid.delegate(super::RowLocalInput::Move(25), None);
    assert!(matches!(
        outcome,
        super::RowLocalOutcome::SelectedTargetChanged(_)
    ));
    grid.set_paint_policy(GridPaintPolicy::new(true));
    terminal
        .draw(|frame| Component::view(&mut grid, frame, area))
        .unwrap();
    let selected = grid.selected_target().cloned().expect("selection");
    assert!(grid.current_flow_offset().unwrap() > 0);
    assert!(grid
        .current_cells()
        .unwrap()
        .iter()
        .any(|cell| cell.target.as_ref() == Some(&selected)));
}

/// Task 4.1: the Grid presentation executes the established bucketed
/// two-column catalog policy — a `Heading`/`Spacer` occupies a full painted
/// line, a bucket's items pack into fresh item rows (so a ragged trailing row
/// stays inside its bucket), scroll/viewport facts count display lines, and
/// `move_item_rows` preserves the column across a header with the ragged
/// trailing-row clamp.
#[test]
fn grid_structural_rows_take_full_lines_and_ragged_traversal_clamps() {
    use super::{GridMediaList, GridPaintPolicy};
    let mut grid = GridMediaList::new();
    grid.set_columns(2, 8, 2);
    // A bucket: heading + 3 items (rows [a,b],[c] ragged); then a spacer +
    // heading; B bucket: [d,e].
    grid.set_content(vec![
        super::MediaListRow::Heading { text: "A".into() },
        lifecycle_item("a"),
        lifecycle_item("b"),
        lifecycle_item("c"),
        super::MediaListRow::Spacer,
        super::MediaListRow::Heading { text: "B".into() },
        lifecycle_item("d"),
        lifecycle_item("e"),
    ]);
    assert_eq!(
        grid.display_lines().len(),
        6,
        "heading/spacer take a line each; items pack two per line"
    );
    let area = Rect::new(0, 0, 20, 6);
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal
        .draw(|frame| {
            grid.set_paint_policy(GridPaintPolicy::new(true));
            Component::view(&mut grid, frame, area);
        })
        .unwrap();
    let cells = grid.current_cells().expect("completed grid frame");
    // Heading lines paint no cells; the B bucket starts on its own line after
    // the spacer + heading, never sharing a row with A's ragged item.
    assert_eq!(cells.len(), 5);
    let c = cells
        .iter()
        .find(|cell| cell.target.as_ref() == Some(&"c".to_string()))
        .expect("ragged item c painted");
    let d = cells
        .iter()
        .find(|cell| cell.target.as_ref() == Some(&"d".to_string()))
        .expect("next-bucket item d painted");
    assert_eq!(
        d.rect.y,
        c.rect.y + 3,
        "spacer + heading lines separate buckets"
    );
    assert!(
        grid.claims_current_point(Position::new(c.rect.x, c.rect.y + 1)),
        "a heading line is inside the claim but resolves no target"
    );
    assert_eq!(
        grid.resolve_current_point(Position::new(c.rect.x, c.rect.y + 1)),
        None,
        "a heading line resolves no cell target"
    );

    // Down from the ragged item c clamps into its own row (tree order: c is
    // its row's last item), then the next item row is the following bucket.
    assert!(grid.select_target(&"c".to_string()));
    grid.move_item_rows(1);
    assert_eq!(grid.selected_target(), Some(&"d".to_string()));
    grid.move_item_rows(-1);
    assert_eq!(grid.selected_target(), Some(&"c".to_string()));
}
