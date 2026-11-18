//! The shared whole-rows capacity rule, the port of the former report's
//! `wholeRows` arithmetic, under the redesign's 34 px row and 2 px gap
//! (gui-queue-panel-redesign design D9).

use rstest::rstest;

/// Contract (`gui-queue-panel-redesign` task 6.1, unit 6 review finding P2):
/// the capacity counts every whole row a list box can show. The gap between
/// rows is not a row's worth of space, so a box exactly two rows and their gap
/// tall fits both.
#[rstest]
#[case(0.0, 0)]
#[case(33.0, 0)]
#[case(34.0, 1)]
#[case(69.0, 1)]
#[case(70.0, 2)]
#[case(71.0, 2)]
fn the_capacity_counts_every_whole_row_that_fits(#[case] height: f32, #[case] rows: usize) {
    assert_eq!(super::super::whole_rows(height), rows);
}
