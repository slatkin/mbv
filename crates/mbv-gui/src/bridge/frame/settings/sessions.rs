//! The Sessions section's content (`add-gui-settings-screen` design D9):
//! the same playback targets the header's chooser lists, so the section's rows
//! and the chooser cannot disagree inside one tick.

use mbv::PlaybackTargetChoice;

use crate::bridge::frame::playback::GuiTargetChoice;

/// The chooser's targets as the Sessions section paints them (design D9). The
/// tick's one playback projection supplies the choices, and
/// [`GuiTargetChoice`] is the same row DTO the chooser's overlay carries, so a
/// selection sends the identity the window painted.
#[must_use]
pub fn targets(choices: &[PlaybackTargetChoice]) -> Vec<GuiTargetChoice> {
    choices.iter().map(GuiTargetChoice::from).collect()
}
