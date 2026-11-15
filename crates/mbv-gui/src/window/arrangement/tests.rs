//! Unit tests of the arrangement thresholds: the Settings screen's measure,
//! which subtracts its own fixed sidebar instead of the Queue column's
//! painted width.

use rstest::rstest;

use super::{
    GuiArrangement, LIBRARY_MINI_MAX_WIDTH_PX, SETTINGS_SIDEBAR_WIDTH_PX, library_arrangement,
    settings_arrangement,
};
use crate::bridge::layout::LayoutSize;

/// A container the window measured.
fn container(width: f32, height: f32) -> LayoutSize {
    LayoutSize { width, height }
}

/// The Settings screen's Mini threshold is the window width minus the 260 px
/// sidebar, whatever the Queue column's painted width is: at 800 px the
/// Library column drops to Mini under the 400 px Queue column, and the
/// screen beside its own 260 px sidebar stays Narrow.
#[test]
fn settings_arrangement_ignores_the_queue_column_width() {
    let measured = container(800.0, 900.0);

    assert_eq!(
        library_arrangement(measured, crate::window::arrangement::QUEUE_PANEL_WIDTH_PX),
        GuiArrangement::Mini
    );
    assert_eq!(settings_arrangement(measured), GuiArrangement::Narrow);
}

/// One pixel under the sidebar plus the Mini maximum is still Mini, and the
/// exact sum is not: the threshold keeps `library_arrangement`'s comparison.
#[rstest]
#[case::one_under(LIBRARY_MINI_MAX_WIDTH_PX + SETTINGS_SIDEBAR_WIDTH_PX - 1.0, GuiArrangement::Mini)]
#[case::at_the_threshold(LIBRARY_MINI_MAX_WIDTH_PX + SETTINGS_SIDEBAR_WIDTH_PX, GuiArrangement::Narrow)]
fn settings_arrangement_measures_the_window_minus_the_sidebar(
    #[case] width: f32,
    #[case] expected: GuiArrangement,
) {
    assert_eq!(settings_arrangement(container(width, 900.0)), expected);
}
