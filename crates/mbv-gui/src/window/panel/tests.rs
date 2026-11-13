//! Tests of the panel state (D16): the contracts of `panel.test.ts` — the
//! precedence of the two toggles over the container size, the 520 px boundary
//! the compact strip starts below, and the unmeasured container's first paint.
//! Pure functions over the measured size, so no test opens a window.

use super::{COMPACT_STRIP_MAX_HEIGHT, GuiPanelState, panel_state};
use crate::bridge::frame::GuiPanelToggles;

/// A container tall enough for the expanded artboard.
const TALL: (f32, f32) = (640.0, 900.0);

/// A container one pixel short of the compact strip's 520 px boundary.
const SHORT: (f32, f32) = (640.0, COMPACT_STRIP_MAX_HEIGHT - 1.0);

fn container(size: (f32, f32)) -> crate::bridge::layout::LayoutSize {
    crate::bridge::layout::LayoutSize {
        width: size.0,
        height: size.1,
    }
}

fn toggles(collapsed: bool, queue_folded: bool) -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed,
        queue_folded,
        ui_reset_epoch: 0,
    }
}

#[rstest::rstest]
#[case::the_collapse_toggle_wins_over_a_short_container(SHORT, true, true, GuiPanelState::Rail)]
#[case::a_container_one_pixel_short_of_520_paints_the_strip(SHORT, false, false, GuiPanelState::CompactStrip)]
#[case::a_520_px_container_keeps_the_expanded_artboard((640.0, COMPACT_STRIP_MAX_HEIGHT), false, false, GuiPanelState::Expanded)]
#[case::the_queue_fold_paints_the_expanded_panel_without_its_queue(TALL, false, true, GuiPanelState::ExpandedWithoutQueue)]
#[case::an_unmeasured_container_paints_the_expanded_artboard((0.0, 0.0), false, false, GuiPanelState::Expanded)]
fn the_panel_state_follows_the_toggles_and_the_container(
    #[case] size: (f32, f32),
    #[case] collapsed: bool,
    #[case] queue_folded: bool,
    #[case] expected: GuiPanelState,
) {
    assert_eq!(panel_state(container(size), &toggles(collapsed, queue_folded)), expected);
}
