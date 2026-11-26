//! The cast card's stripe contract (`gui-hero-design-polish`): the zebra
//! counts visual rows, not members, so both cells of a row share one fill and
//! one column alternates plainly.

use rstest::rstest;

use super::{CAST_DARK_ROW, CAST_LIGHT_ROW, row_fill};

#[rstest]
#[case::first_row_first_cell(0, 2, CAST_LIGHT_ROW)]
#[case::first_row_second_cell(1, 2, CAST_LIGHT_ROW)]
#[case::second_row_first_cell(2, 2, CAST_DARK_ROW)]
#[case::second_row_second_cell(3, 2, CAST_DARK_ROW)]
#[case::third_row(4, 2, CAST_LIGHT_ROW)]
#[case::one_column_alternates(1, 1, CAST_DARK_ROW)]
fn a_member_takes_the_fill_of_its_visual_row(
    #[case] index: usize,
    #[case] columns: usize,
    #[case] expected: &str,
) {
    assert_eq!(row_fill(index, columns), expected);
}
