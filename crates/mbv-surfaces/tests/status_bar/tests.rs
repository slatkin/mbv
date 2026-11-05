//! Public-API tests for the Status bar Surface state (add-gui-front-end row
//! 4.6): the prefix-armed display state and the toast presentation the
//! adapters project through and paint from.

use mbv_surfaces::status_bar::StatusBarSurface;
use mbv_ui_model::toast::{StatusToast, ToastSeverity};

/// The armed flag and the toast presentation round-trip through the Surface
/// state the adapters project through; clearing the toast presents none.
#[test]
fn the_surface_carries_the_armed_flag_and_the_toast_presentation() {
    let mut surface = StatusBarSurface::new();
    assert!(!surface.prefix_armed());
    assert!(surface.toast().is_none());

    surface.set_prefix_armed(true);
    surface.set_toast(Some(StatusToast {
        text: "Scanning 'Movies'...".into(),
        severity: ToastSeverity::Neutral,
    }));
    assert!(surface.prefix_armed());
    let toast = surface.toast().expect("the presented toast");
    assert_eq!(toast.text, "Scanning 'Movies'...");
    assert_eq!(toast.severity, ToastSeverity::Neutral);

    surface.set_prefix_armed(false);
    surface.set_toast(None);
    assert!(!surface.prefix_armed());
    assert!(surface.toast().is_none());
}
