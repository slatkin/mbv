//! The shared whole-rows capacity rule, the port of the former report's
//! `wholeRows` arithmetic, under the delineated 40 px row and its 0 px gap
//! (gui-delineated-rows design D3).

use rstest::rstest;

/// Contract (`gui-queue-panel-redesign` task 6.1, unit 6 review finding P2):
/// the capacity counts every whole row a list box can show. The gap between
/// rows is not a row's worth of space, so a box exactly two rows and their gap
/// tall fits both.
#[rstest]
#[case(0.0, 0)]
#[case(39.0, 0)]
#[case(40.0, 1)]
#[case(79.0, 1)]
#[case(80.0, 2)]
#[case(81.0, 2)]
fn the_capacity_counts_every_whole_row_that_fits(#[case] height: f32, #[case] rows: usize) {
    assert_eq!(super::super::whole_rows(height), rows);
}
