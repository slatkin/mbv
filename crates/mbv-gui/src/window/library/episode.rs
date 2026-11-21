//! The episode row helpers (design D6): the played test and the resume percent
//! every episode row and Hero reads, whatever destination carries the row. They
//! live here, not in `shows_workspace`, so the Feeds Hero and the shared item
//! row do not import Shows logic.

use crate::bridge::frame::library::GuiHomeRowStateFrame;

/// Whether a row paints the played state: a played episode without a resume
/// position (the shared row state the Home rows carry).
#[must_use]
pub fn is_played_episode(state: &GuiHomeRowStateFrame) -> bool {
    matches!(state, GuiHomeRowStateFrame::Played)
}

/// The resume percent an active episode row paints, or `None`.
#[must_use]
pub fn episode_progress(state: &GuiHomeRowStateFrame) -> Option<u8> {
    match state {
        GuiHomeRowStateFrame::Active { progress } => *progress,
        _ => None,
    }
}
