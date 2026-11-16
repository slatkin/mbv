//! The Shows Hero's Workspace interaction state (`add-gui-library-panel`
//! design D1, D12, the port of `lib/library/shows-workspace.ts`): the cursor
//! over the selected season's episodes. It is the window content's own — the
//! core never reads it, and the hover is the Workspace table's own paint — and
//! the season pills, the selected season, and the episodes themselves are the
//! core's, carried by the frame.
//!
//! The cursor is an episode id, never a row position: the frame carries stable
//! Service ids, so a cursor whose episode left the frame (a season change, or
//! a new detail) resolves to the first episode the Workspace paints. The pure
//! cursor model is [`move_episode_cursor`]; the store functions below send the
//! commands the Hero's keys and controls name.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiHomeRowStateFrame, GuiShowsDetail, GuiShowsEpisodeRowFrame, GuiShowsWorkspaceFrame,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::list::ListMove;

use super::state::LibraryState;

/// The Workspace's interaction state (`shows-workspace.ts`'s
/// `ShowsWorkspaceState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShowsWorkspaceState {
    /// The cursor's episode id, or `None` while no episode carries the cursor
    /// (a fresh series, and a season whose episodes have not landed).
    pub cursor: Option<String>,
}

impl ShowsWorkspaceState {
    /// The state a freshly mounted Workspace starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self::default()
    }
}

/// The selected series' Workspace, or `None` on the Latest mode's episode Hero
/// and while the selected series' detail has not landed.
#[must_use]
pub fn workspace_of(detail: &GuiShowsDetail) -> Option<&GuiShowsWorkspaceFrame> {
    detail
        .hero
        .as_ref()
        .and_then(|hero| hero.workspace.as_ref())
}

/// The episode the cursor rests on (design D12): the episode the state's id
/// names while the Workspace still paints it, and the Workspace's first
/// episode otherwise — the cursor re-resolves to the first episode when its id
/// leaves the frame, so a season change never leaves it resting on nothing.
#[must_use]
pub fn resolve_shows_episode_cursor<'a>(
    workspace: Option<&'a GuiShowsWorkspaceFrame>,
    cursor: Option<&str>,
) -> Option<&'a GuiShowsEpisodeRowFrame> {
    let episodes: &[GuiShowsEpisodeRowFrame] =
        workspace.map_or(&[], |workspace| &workspace.episodes);
    episodes
        .iter()
        .find(|episode| Some(episode.episode_id.as_str()) == cursor)
        .or_else(|| episodes.first())
}

/// The cursor's episode in the carried frame, or `None` while the Workspace
/// holds no episode. The Hero's Enter, `n`, and `a` act on this episode.
#[must_use]
pub fn shows_cursor_episode<'a>(
    state: &LibraryState,
    detail: &'a GuiShowsDetail,
) -> Option<&'a GuiShowsEpisodeRowFrame> {
    resolve_shows_episode_cursor(
        workspace_of(detail),
        state.shows_workspace.peek().cursor.as_deref(),
    )
}

/// The episode id a movement leaves (design D12): the next, the previous, or a
/// page away episode, clamped to the season's ends. With no cursor on a
/// painted episode, Up and End start from the last episode and every other
/// movement from the first, so one key press always lands the cursor. The
/// Workspace carries no measured viewport in this unit, so a page move spans
/// one row.
#[must_use]
pub fn move_episode_cursor(
    episodes: &[GuiShowsEpisodeRowFrame],
    cursor: Option<&str>,
    mv: ListMove,
) -> Option<String> {
    let last = episodes.len().checked_sub(1)?;
    let at = episodes
        .iter()
        .position(|episode| Some(episode.episode_id.as_str()) == cursor);
    let Some(at) = at else {
        let row = match mv {
            ListMove::Up | ListMove::End => &episodes[last],
            _ => &episodes[0],
        };
        return Some(row.episode_id.clone());
    };
    let to = match mv {
        ListMove::Up | ListMove::PageUp => at.saturating_sub(1),
        ListMove::Down | ListMove::PageDown => at.saturating_add(1),
        ListMove::Home => 0,
        ListMove::End => last,
    };
    Some(episodes[to.min(last)].episode_id.clone())
}

/// Move the Workspace's cursor one movement along (design D12): the movement
/// keys while the pointer rests on the Workspace.
pub fn move_shows_episode_cursor(state: &LibraryState, mv: ListMove, detail: &GuiShowsDetail) {
    let workspace = workspace_of(detail);
    let current =
        resolve_shows_episode_cursor(workspace, state.shows_workspace.peek().cursor.as_deref());
    let cursor = move_episode_cursor(
        workspace.map_or(&[], |workspace| &workspace.episodes),
        current.map(|episode| episode.episode_id.as_str()),
        mv,
    );
    let mut shows_workspace = state.shows_workspace;
    shows_workspace.with_mut(|held| {
        if held.cursor != cursor {
            held.cursor = cursor;
        }
    });
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

/// Play the cursor's episode (design D12): Enter on the Workspace, and the
/// Hero's Play. The Owner owns the queue, so the command replaces it through
/// the shared play path.
pub fn play_shows_episode(state: &LibraryState, lib_idx: usize, detail: &GuiShowsDetail) {
    act_on_cursor(state, detail, |item_id| GuiLibraryCommand::ShowsPlay {
        lib_idx,
        item_id: item_id.to_owned(),
    });
}

/// Play the cursor's episode directly after the active slot (design D12).
pub fn play_next_shows_episode(state: &LibraryState, lib_idx: usize, detail: &GuiShowsDetail) {
    act_on_cursor(state, detail, |item_id| GuiLibraryCommand::ShowsPlayNext {
        lib_idx,
        item_id: item_id.to_owned(),
    });
}

/// Add the cursor's episode to the queue (design D12).
pub fn queue_shows_episode(state: &LibraryState, lib_idx: usize, detail: &GuiShowsDetail) {
    act_on_cursor(state, detail, |item_id| GuiLibraryCommand::ShowsQueue {
        lib_idx,
        item_id: item_id.to_owned(),
    });
}

/// Send one episode command for the cursor's episode, and nothing while the
/// Workspace holds none.
fn act_on_cursor(
    state: &LibraryState,
    detail: &GuiShowsDetail,
    command_of: impl FnOnce(&str) -> GuiLibraryCommand,
) {
    let Some(episode) = shows_cursor_episode(state, detail) else {
        return;
    };
    state.link.library(command_of(&episode.episode_id));
}

/// Start the cursor over when the selected series changes (design D12): the
/// season pills start on the first season and the episodes are the new
/// series', so the previous series' cursor names nothing.
pub fn reset_shows_episode_cursor(state: &LibraryState) {
    let mut shows_workspace = state.shows_workspace;
    shows_workspace.set(ShowsWorkspaceState::initial());
}

/// Settle the cursor on the row a press names (design D12): a press moves the
/// cursor onto the row it hit, as the Music list's track press does.
pub fn press_shows_episode(state: &LibraryState, episode_id: &str) {
    let mut shows_workspace = state.shows_workspace;
    shows_workspace.with_mut(|held| {
        if held.cursor.as_deref() != Some(episode_id) {
            held.cursor = Some(episode_id.to_owned());
        }
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
