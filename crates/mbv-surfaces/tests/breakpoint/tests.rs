//! Public-API tests for the shared breakpoint classes (extract-client-core row
//! 6.6): the width on each side of a threshold edge takes the class on that
//! side.

use mbv_surfaces::breakpoint::{BreakpointClass, BreakpointThresholds, classify};

/// A fixture threshold set: `Mini` below 80, `Wide` from 100. These are test
/// values, not a Client's thresholds.
const FIXTURE: BreakpointThresholds<u16> = BreakpointThresholds {
    mini_below: 80,
    wide_from: 100,
};

/// Each edge switches the class at exactly its width. The width just below an
/// edge keeps the class on that side, and the edge width takes the next class.
#[rstest::rstest]
#[case::just_below_the_mini_edge(79, BreakpointClass::Mini)]
#[case::at_the_mini_edge(80, BreakpointClass::Narrow)]
#[case::just_below_the_wide_edge(99, BreakpointClass::Narrow)]
#[case::at_the_wide_edge(100, BreakpointClass::Wide)]
fn classify_switches_class_exactly_at_each_threshold_edge(
    #[case] width: u16,
    #[case] expected: BreakpointClass,
) {
    assert_eq!(classify(width, FIXTURE), expected);
}
