//! The Status bar's headless tests (row 6.5): the egui `Context::run_ui`
//! composition with synthetic `RawInput`, asserting the projection the frame
//! adopts and the painter reads, never appearance (design D11).

use mbv::StatusBarProjection;
use mbv_ui_model::toast::{StatusToast, ToastSeverity};

use crate::headless::{context, frame_input, run_frame};
use crate::tick_host::GuiTickHost;

/// A frame over an App with prefix mode armed and a live toast presents both
/// to the Status bar: the projection the frame adopts carries the armed flag
/// and the toast's text and severity (row 6.5's verification).
#[test]
fn the_status_bar_presents_the_armed_prefix_and_the_live_toast() {
    let ctx = context();
    let mut host = GuiTickHost::new(
        mbv::stub_app_with_prefix_and_toast("Scanning 'Movies'..."),
        ctx.clone(),
    );
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    assert_eq!(
        host.status_test_projection(),
        StatusBarProjection {
            prefix_armed: true,
            toast: Some(StatusToast {
                text: "Scanning 'Movies'...".into(),
                severity: ToastSeverity::Neutral,
            }),
        }
    );
}
