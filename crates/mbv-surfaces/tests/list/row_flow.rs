//! Row-flow position arithmetic (moved with the seam from `mbv-components`).

use mbv_surfaces::list::{Row, RowFlow};

#[test]
fn structural_rows_occupy_positions_without_being_addressable() {
    let flow = RowFlow::new(vec![
        Row::selectable(10),
        Row::structural(),
        Row::selectable(20),
    ]);

    assert_eq!(flow.len(), 3);
    assert!(matches!(flow.row_at(1), Some(Row::Structural)));
    assert_eq!(flow.position_of(&10), Some(0));
    assert_eq!(flow.position_of(&20), Some(2));
    assert_eq!(flow.position_of(&30), None);
    assert_eq!(flow.row_at(1).and_then(Row::target), None);
}

#[test]
fn selectable_count_and_position_ignore_structural_rows() {
    let flow = RowFlow::new(vec![
        Row::structural(),
        Row::selectable('a'),
        Row::structural(),
        Row::selectable('b'),
    ]);

    assert_eq!(flow.selectable_len(), 2);
    assert_eq!(flow.position_of_selectable(0), Some(1));
    assert_eq!(flow.position_of_selectable(1), Some(3));
    assert_eq!(flow.position_of_selectable(2), None);
}
