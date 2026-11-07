//! Public-API tests of the `report_layout` command's DTO wire contract
//! (`mbv_gui::bridge::layout`): the panel container's painted size arrives
//! in logical pixels (design D10), and a payload that omits it reads as the
//! zero size.

use mbv_gui::bridge::layout::{LayoutReport, LayoutSize};

/// A `report_layout` payload carries the panel container's size in logical
/// pixels (design D10).
#[test]
fn layout_report_carries_the_panel_container_size() {
    let report: LayoutReport = serde_json::from_str(
        r#"{"containerSize":{"width":800.0,"height":600.0},"lists":[],"regions":[]}"#,
    )
    .unwrap();
    assert_eq!(
        report.container_size,
        LayoutSize {
            width: 800.0,
            height: 600.0,
        }
    );
}

/// A payload that omits the container size reads as the zero size.
#[test]
fn layout_report_without_container_size_reads_zero() {
    let report: LayoutReport = serde_json::from_str(r#"{"lists":[],"regions":[]}"#).unwrap();
    assert_eq!(report.container_size, LayoutSize::default());
}
