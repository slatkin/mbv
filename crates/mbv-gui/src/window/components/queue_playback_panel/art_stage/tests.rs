//! The control rows' scale contract (`gui-art-player-overlay` design D3, task
//! 4.2): the rows take at most a third of the stage's measured height, and
//! they never grow past their full size.

use rstest::rstest;

use super::{BOTTOM_FULL_HEIGHT_PX, player_scale};

/// Contract (`gui-art-player-overlay` design D3; `gui-queue-panel-redesign`
/// design D3): the rows' scale is the stage's one-third share of their full
/// height, capped at 1.0 — a stage three times the rows' height or taller
/// keeps them at full size, and a shorter stage scales them down by that
/// ratio. The full height is the canvas's 104 px: the two rows of the
/// redesign and their pads.
#[rstest]
#[case::tall_image(312.0, 1.0)]
#[case::short_image(156.0, 0.5)]
fn the_control_rows_scale_with_the_stage_height(#[case] stage_height: f32, #[case] expected: f32) {
    let scale = player_scale(stage_height);
    assert!(
        (scale - expected).abs() < f32::EPSILON,
        "a {stage_height} px stage gives {scale}, not {expected}"
    );
}

/// Contract (`gui-queue-panel-redesign` design D3): the bottom layer's full
/// height is the canvas's 104 px — the top pad, the 14 px seek row, the gap,
/// the 44 px transport row, and the bottom pad. The scale divides by this
/// height, so a row or pad that drifts off the canvas would overflow the
/// stage's one-third limit.
#[test]
fn the_bottom_layer_is_the_canvas_104_px() {
    assert!((BOTTOM_FULL_HEIGHT_PX - 104.0).abs() < f32::EPSILON);
}
