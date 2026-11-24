//! The appearance-focus contract: a pure decision over the presentation a
//! Client reports and the effective focus, so it needs no `App`.

use super::{PanelPresentation, panel_appearance_focus_decision};
use crate::app::PanelFocus;
use rstest::rstest;

/// The appearance-focus contract: unpinned mini view always rests; pinned
/// mini view follows the window focus and the effective focus; wide view
/// ignores the window focus and the focus bit selects the palette per panel.
/// (A pinned launch needs the `PINWIN_PANEL` claim, so the pure decision is
/// tabled rather than driven through `App`.)
#[rstest]
#[case::unpinned_mini_focused(PanelPresentation::MiniUnpinned, PanelFocus::Library, false)]
#[case::pinned_mini_window_lost(
    PanelPresentation::MiniPinned { window_focused: false },
    PanelFocus::Library,
    false
)]
#[case::pinned_mini_window_focused(
    PanelPresentation::MiniPinned { window_focused: true },
    PanelFocus::Library,
    true
)]
#[case::pinned_mini_other_panel_focused(
    PanelPresentation::MiniPinned { window_focused: true },
    PanelFocus::Queue,
    false
)]
#[case::wide_focused(PanelPresentation::Wide, PanelFocus::Library, true)]
#[case::wide_sibling(PanelPresentation::Wide, PanelFocus::Queue, false)]
fn panel_appearance_focus_for_library(
    #[case] presentation: PanelPresentation,
    #[case] focus: PanelFocus,
    #[case] expected: bool,
) {
    assert_eq!(
        panel_appearance_focus_decision(presentation, focus, PanelFocus::Library),
        expected
    );
}
