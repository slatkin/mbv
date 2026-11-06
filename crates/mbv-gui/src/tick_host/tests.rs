//! The GUI frame's breakpoint class (row 6.6, design D10): the window's width
//! classifies each frame, through the production frame order of the headless
//! harness (design D11).

use mbv_surfaces::breakpoint::BreakpointClass;

use crate::headless::{context, frame_input_at_width, run_frame};
use crate::tick_host::GuiTickHost;

/// The class follows the window width frame by frame. An 800-point window
/// sits between the GUI thresholds, so its frame is Narrow. A 1200-point
/// window reaches `wide_from`, so the next frame is Wide.
#[test]
fn the_frame_class_follows_the_window_width() {
    let ctx = context();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(0), ctx.clone());

    run_frame(&ctx, &mut host, frame_input_at_width(800.0, Vec::new()));
    assert_eq!(host.breakpoint_test_class(), BreakpointClass::Narrow);

    run_frame(&ctx, &mut host, frame_input_at_width(1200.0, Vec::new()));
    assert_eq!(host.breakpoint_test_class(), BreakpointClass::Wide);
}
