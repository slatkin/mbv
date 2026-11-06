//! Public-API tests of the `report_layout` command's DTO wire contract
//! (`mbv_gui::bridge::layout`): the window width arrives in logical pixels
//! (design D10), and a payload that omits it reads as 0.0.

use mbv_gui::bridge::layout::LayoutReport;

/// A `report_layout` payload carries the window's inner width in logical
/// pixels (design D10).
#[test]
fn layout_report_carries_the_window_width() {
    let report: LayoutReport =
        serde_json::from_str(r#"{"width":800.0,"lists":[],"regions":[]}"#).unwrap();
    assert_eq!(report.width, 800.0);
}

/// A payload from before the `width` field existed reads as 0.0.
#[test]
fn layout_report_without_width_reads_zero() {
    let report: LayoutReport = serde_json::from_str(r#"{"lists":[],"regions":[]}"#).unwrap();
    assert_eq!(report.width, 0.0);
}
