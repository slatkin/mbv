//! Cursor arithmetic over the shared row flow (moved with the seam from
//! `mbv-components`).

use rstest::rstest;

use super::helpers::TestListState;
use mbv_surfaces::list::{Cursored, Row, RowFlow};

fn flow() -> RowFlow<u8> {
    RowFlow::new(vec![
        Row::selectable(1),
        Row::structural(),
        Row::selectable(2),
        Row::structural(),
        Row::selectable(3),
    ])
}

#[rstest]
#[case(-1, Some(0), Some(1))]
#[case(1, Some(0), Some(2))]
#[case(99, Some(4), Some(3))]
fn movement_skips_structural_rows_and_clamps(
    #[case] delta: isize,
    #[case] starting_position: Option<usize>,
    #[case] expected_target: Option<u8>,
) {
    let rows = flow();
    let mut cursor = TestListState::default();
    cursor.first(&rows);
    if let Some(position) = starting_position {
        cursor.select_target(&rows, rows.row_at(position).and_then(Row::target).unwrap());
    }
    cursor.move_by(&rows, delta);
    assert_eq!(cursor.selected, expected_target);
    assert!(cursor.index(&rows).is_some_and(|position| {
        rows.row_at(position)
            .is_some_and(|row| !row.is_structural())
    }));
}

#[test]
fn first_and_last_skip_structural_rows_without_wrap() {
    let rows = flow();
    let mut cursor = TestListState::default();

    assert_eq!(cursor.first(&rows), Some(0));
    assert_eq!(cursor.move_by(&rows, -1), Some(0));
    assert_eq!(cursor.last(&rows), Some(4));
    assert_eq!(cursor.move_by(&rows, 1), Some(4));
}

#[test]
fn absent_target_falls_back_to_first_selectable_row() {
    let rows = flow();
    let mut cursor = TestListState {
        selected: Some(2),
        ..Default::default()
    };

    assert!(!cursor.select_target(&rows, &99));
    assert_eq!(cursor.selected_target(), Some(&1));
    assert_eq!(cursor.index(&rows), Some(0));
}

#[rstest]
#[case(-1, Some(4), Some(3))]
#[case(1, Some(0), Some(1))]
fn movement_repairs_selection_for_rows_absent_from_flow(
    #[case] delta: isize,
    #[case] expected_position: Option<usize>,
    #[case] expected_target: Option<u8>,
) {
    let rows = flow();
    let mut cursor = TestListState {
        selected: Some(99),
        ..Default::default()
    };

    assert_eq!(cursor.move_by(&rows, delta), expected_position);
    assert_eq!(cursor.selected, expected_target);
}

#[test]
fn selecting_an_ordinal_in_an_empty_flow_clears_stale_selection() {
    let mut cursor = TestListState {
        selected: Some(2),
        ..Default::default()
    };

    assert_eq!(
        cursor.select_selectable_ordinal(&RowFlow::new(vec![]), 0),
        None
    );
    assert_eq!(cursor.selected, None);
}
