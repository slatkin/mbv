// The Shows Hero's Workspace interaction state (add-gui-library-panel design
// D1, D12, row 7.1): the cursor over the selected season's episodes. It is
// the window content's own — the core never reads it, and the hover is the
// Workspace table's own paint — and the season pills, the selected season, and the episodes
// themselves are the core's, carried by the frame.
//
// The cursor is an episode id, never a row position: the frame carries stable
// Service ids, so a cursor whose episode left the frame (a season change, or a
// new detail) resolves to the first episode the Workspace paints. The pure
// cursor model is `moveEpisodeCursor`; the store functions below send the
// commands the Hero's keys and controls name.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type {
	GuiHomeRowState,
	GuiShowsDetail,
	GuiShowsEpisodeRow,
	GuiShowsWorkspace,
} from '$lib/bridge/types';
import type { ListMove } from './list';

/** The Workspace's interaction state (design D12). */
export interface ShowsWorkspaceState {
	/** The cursor's episode id, or `null` while no episode carries the cursor
	 * (a fresh series, and a season whose episodes have not landed). */
	cursor: string | null;
}

/** The state a freshly mounted Workspace starts with. */
export function initialShowsWorkspace(): ShowsWorkspaceState {
	return { cursor: null };
}

/** The Workspace's interaction state. */
export const showsWorkspace = writable<ShowsWorkspaceState>(initialShowsWorkspace());

/** The selected series' Workspace, or `null` on the Latest mode's episode Hero
 * and while the selected series' detail has not landed. */
function workspaceOf(detail: GuiShowsDetail): GuiShowsWorkspace | null {
	return detail.hero?.workspace ?? null;
}

/**
 * The episode the cursor rests on (design D12): the episode the state's id
 * names while the Workspace still paints it, and the Workspace's first episode
 * otherwise — the cursor re-resolves to the first episode when its id leaves
 * the frame, so a season change never leaves it resting on nothing.
 */
export function resolveShowsEpisodeCursor(
	workspace: GuiShowsWorkspace | null,
	cursor: string | null,
): GuiShowsEpisodeRow | null {
	const episodes = workspace?.episodes ?? [];
	return episodes.find((episode) => episode.episodeId === cursor) ?? episodes[0] ?? null;
}

/** The cursor's episode in the carried frame, or `null` while the Workspace
 * holds no episode. The Hero's Enter, `n`, and `a` act on this episode. */
export function showsCursorEpisode(detail: GuiShowsDetail): GuiShowsEpisodeRow | null {
	return resolveShowsEpisodeCursor(workspaceOf(detail), get(showsWorkspace).cursor);
}

/**
 * The episode id a movement leaves (design D12): the next, the previous, or a
 * page away episode, clamped to the season's ends. With no cursor on a painted
 * episode, Up and End start from the last episode and every other movement
 * from the first, so one key press always lands the cursor. The Workspace
 * carries no measured viewport in this unit, so a page move spans one row.
 */
export function moveEpisodeCursor(
	episodes: readonly GuiShowsEpisodeRow[],
	cursor: string | null,
	move: ListMove,
): string | null {
	const last = episodes.length - 1;
	if (last < 0) {
		return null;
	}
	const at = episodes.findIndex((episode) => episode.episodeId === cursor);
	if (at < 0) {
		return (move === 'up' || move === 'end' ? episodes[last] : episodes[0]).episodeId;
	}
	const to = {
		up: at - 1,
		down: at + 1,
		pageUp: at - 1,
		pageDown: at + 1,
		home: 0,
		end: last,
	}[move];
	return episodes[Math.min(Math.max(to, 0), last)].episodeId;
}

/** Move the Workspace's cursor one movement along (design D12): the movement
 * keys while the Hero pane has the pane focus. */
export function moveShowsEpisodeCursor(move: ListMove, detail: GuiShowsDetail): void {
	const workspace = workspaceOf(detail);
	const current = resolveShowsEpisodeCursor(workspace, get(showsWorkspace).cursor);
	const cursor = moveEpisodeCursor(workspace?.episodes ?? [], current?.episodeId ?? null, move);
	showsWorkspace.update((state) => (state.cursor === cursor ? state : { ...state, cursor }));
}

/** Move the Workspace onto the neighbouring season (design D12): Right and
 * Left walk the pills, and a move past either end changes nothing. The press
 * sends the select-season command, and the core's frame carries the season's
 * episodes back. */
export function stepShowsSeason(delta: -1 | 1, libIdx: number, detail: GuiShowsDetail): void {
	const workspace = workspaceOf(detail);
	if (workspace === null || workspace.selectedSeason === null) {
		return;
	}
	const at = workspace.seasons.findIndex(
		(season) => season.seasonId === workspace.selectedSeason,
	);
	const next = workspace.seasons[at + delta];
	if (at < 0 || next === undefined) {
		return;
	}
	void sendLibraryCommand({
		command: 'showsSelectSeason',
		libIdx,
		seasonId: next.seasonId,
	});
}

/** Play the cursor's episode (design D12): Enter on the Workspace, and the
 * Hero's Play. The Owner owns the queue, so the command replaces it through
 * the shared play path. A Workspace with no episode sends nothing. */
export function playShowsEpisode(libIdx: number, detail: GuiShowsDetail): void {
	actOnCursor(libIdx, detail, 'showsPlay');
}

/** Play the cursor's episode directly after the active slot (design D12). */
export function playNextShowsEpisode(libIdx: number, detail: GuiShowsDetail): void {
	actOnCursor(libIdx, detail, 'showsPlayNext');
}

/** Add the cursor's episode to the queue (design D12). */
export function queueShowsEpisode(libIdx: number, detail: GuiShowsDetail): void {
	actOnCursor(libIdx, detail, 'showsQueue');
}

/** Send one episode command for the cursor's episode, and nothing while the
 * Workspace holds none. */
function actOnCursor(
	libIdx: number,
	detail: GuiShowsDetail,
	command: 'showsPlay' | 'showsPlayNext' | 'showsQueue',
): void {
	const episode = showsCursorEpisode(detail);
	if (episode === null) {
		return;
	}
	void sendLibraryCommand({ command, libIdx, itemId: episode.episodeId });
}

/** Start the cursor over when the selected series changes (design D12): the
 * season pills start on the first season and the episodes are the new series',
 * so the previous series' cursor names nothing. */
export function resetShowsEpisodeCursor(): void {
	showsWorkspace.set(initialShowsWorkspace());
}

/** Settle the cursor on the row a press names (design D12): a press moves the
 * cursor onto the row it hit, as the Music list's track press does. */
export function pressShowsEpisode(episodeId: string): void {
	showsWorkspace.update((state) =>
		state.cursor === episodeId ? state : { ...state, cursor: episodeId },
	);
}

/** Whether a row paints the played state: a played episode without a resume
 * position (the shared row state the Home rows carry). */
export function isPlayedEpisode(state: GuiHomeRowState): boolean {
	return state === 'played';
}

/** The resume percent an active episode row paints, or `null`. */
export function episodeProgress(state: GuiHomeRowState): number | null {
	return typeof state === 'object' ? state.active.progress : null;
}
