//! The Shows Hero's Workspace (`add-gui-library-panel` design D12, the port of
//! `lib/library/shows-workspace.ts`): the selected season's episodes. The season
//! pills, the selected season, and the episodes are the core's, carried by the
//! frame, and the Workspace keeps no cursor of its own — the row a key or a
//! control acts on is the row the pointer marks
//! (`gui-pointer-target-interaction` D2, D6).
//!
//! Every command names an episode's stable Service id, never a row position. The
//! resolution from a pointer marker back to the frame's episode is
//! [`shows_episode_at`]; the functions below send the commands the Hero's keys
//! and controls name.

use crate::bridge::frame::library::{
    GuiHomeRowStateFrame, GuiShowsDetail, GuiShowsEpisodeRowFrame, GuiShowsWorkspaceFrame,
};
use crate::bridge::library::GuiLibraryCommand;

use super::state::LibraryState;

/// The selected series' Workspace, or `None` on the Latest mode's episode Hero
/// and while the selected series' detail has not landed.
#[must_use]
pub fn workspace_of(detail: &GuiShowsDetail) -> Option<&GuiShowsWorkspaceFrame> {
    detail
        .hero
        .as_ref()
        .and_then(|hero| hero.workspace.as_ref())
}

/// The Workspace episode the pointer's marker names (design D1, D2): the marker
/// carries the episode's stable Service id, so the frame's episodes resolve it
/// and a stale or foreign id names no episode. `None` while the Workspace paints
/// no such episode.
#[must_use]
pub fn shows_episode_at<'a>(
    detail: &'a GuiShowsDetail,
    episode_id: &str,
) -> Option<&'a GuiShowsEpisodeRowFrame> {
    workspace_of(detail)?
        .episodes
        .iter()
        .find(|episode| episode.episode_id == episode_id)
}

/// Move the Workspace onto the neighbouring season (design D12): Right and
/// Left walk the pills, and a move past either end changes nothing. The press
/// sends the select-season command, and the core's frame carries the season's
/// episodes back.
pub fn step_shows_season(
    state: &LibraryState,
    delta: i32,
    lib_idx: usize,
    detail: &GuiShowsDetail,
) {
    let Some(workspace) = workspace_of(detail) else {
        return;
    };
    let Some(selected) = workspace.selected_season.as_deref() else {
        return;
    };
    let at = workspace
        .seasons
        .iter()
        .position(|season| season.season_id == selected);
    let Some(at) = at else {
        return;
    };
    let next = match delta {
        ..=0 => usize::try_from(delta.unsigned_abs())
            .ok()
            .and_then(|step| at.checked_sub(step)),
        _ => usize::try_from(delta)
            .ok()
            .and_then(|step| at.checked_add(step)),
    };
    let Some(next) = next.and_then(|next| workspace.seasons.get(next)) else {
        return;
    };
    state.link.library(GuiLibraryCommand::ShowsSelectSeason {
        lib_idx,
        season_id: next.season_id.clone(),
    });
}

/// Play an episode (design D12): Enter on the Workspace, and the Hero's Play.
/// The Owner owns the queue, so the command replaces it through the shared play
/// path.
pub fn play_shows_episode(state: &LibraryState, lib_idx: usize, episode_id: &str) {
    state.link.library(GuiLibraryCommand::ShowsPlay {
        lib_idx,
        item_id: episode_id.to_owned(),
    });
}

/// Play an episode directly after the active slot (design D12): `n` on the
/// Workspace.
pub fn play_next_shows_episode(state: &LibraryState, lib_idx: usize, episode_id: &str) {
    state.link.library(GuiLibraryCommand::ShowsPlayNext {
        lib_idx,
        item_id: episode_id.to_owned(),
    });
}

/// Add an episode to the queue (design D12): `a` on the Workspace.
pub fn queue_shows_episode(state: &LibraryState, lib_idx: usize, episode_id: &str) {
    state.link.library(GuiLibraryCommand::ShowsQueue {
        lib_idx,
        item_id: episode_id.to_owned(),
    });
}

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

#[cfg(test)]
mod tests;
