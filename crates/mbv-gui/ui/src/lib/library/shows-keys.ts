// The Shows destination's key routing (add-gui-library-panel design D3, D14):
// the Library keys while a Shows library is active. `./keys` stays the panel's
// one routing site and hands a Shows key here after the window keys and the
// open-field checks, so this module never installs a router. The pane focus
// decides the control a key acts on: the mode pills, the tree list, or the
// Hero's Workspace.
//
// Everything else matches the Music router: the movements and Enter never reach
// the core, Right and Left open and close the tree node under the cursor (the
// tree meaning design D3 gives them), Esc's overlay and menu handling runs
// before this, `/` opens the list's filter, and Space with no focused pill
// stays the core's play and pause.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiShowsDetail, GuiShowsList } from '$lib/bridge/types';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryShowsDetail, libraryShowsList } from './parts';
import { openLibraryFilter } from './state';
import {
	closeShowsNode,
	focusedShowsPill,
	moveShowsListCursor,
	moveShowsPillFocus,
	openShowsNode,
	playShowsRow,
	playShowsRowNext,
	queueShowsRow,
	requestShowsPillSelect,
	selectShowsRow,
	showsCursorRow,
	showsNodeOpen,
	toggleShowsNode,
} from './shows';
import { isShowsNodeRow } from './shows-list';
import type { ShowsListRow } from './shows-list';
import {
	moveShowsEpisodeCursor,
	playNextShowsEpisode,
	playShowsEpisode,
	queueShowsEpisode,
	stepShowsSeason,
} from './shows-workspace';

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/**
 * Route a key to the Shows keys (design D12): the pill row walks its modes and
 * selects the focused one, the list's cursor moves and Enter selects a series
 * or plays a flat row, and the Hero's Workspace cursor moves over the season's
 * episodes. `n` and `a` act on the item the focused pane names — a flat row in
 * the list, an episode in the Workspace — and a series row is not playable, so
 * its `n` and `a` change nothing.
 */
export function routeShowsPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	const shows = libraryShowsList();
	const detail = libraryShowsDetail();
	if (shows === null || detail === null) {
		return 'core';
	}
	// `/` opens the list's filter (design D3, D7): the rail's keyboard path.
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	// Space selects the focused pill (design D6). With no pill focused, it is
	// the core's: Space plays and pauses.
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = focusedShowsPill(shows);
		if (pill === null) {
			return 'core';
		}
		requestShowsPillSelect(pill);
		return 'handled';
	}
	if (!bare(event)) {
		return 'core';
	}
	if (ui.pane === 'pills') {
		return orRemaining(routePillKey(event, shows), event);
	}
	if (ui.pane === 'list') {
		return routeListKey(event);
	}
	if (ui.pane === 'hero' && detail.hero !== null) {
		return orRemaining(routeWorkspaceKey(event, shows.libIdx, detail), event);
	}
	// The Hero pane the Upcoming mode does not paint, and any pane that left a
	// key unclaimed, still keep the Library's keys from the core: the movements
	// and Enter change nothing, and `n` and `a` act on the list's cursor row.
	return routeRemainingKey(event);
}

/** The key's answer, or the remaining Library keys' answer when the pane left
 * it unclaimed: a pane that does not act on a key never passes it to the
 * core's playback chords (design D3, step 4). */
function orRemaining(outcome: KeyOutcome, event: KeyboardEvent): KeyOutcome {
	return outcome === 'core' ? routeRemainingKey(event) : outcome;
}

/** The Library's own keys no Shows pane claims (design D3, step 4): `n` and `a`
 * act on the list's cursor row, as the Music router's item key acts from every
 * pane, and a series row is not playable so its key changes nothing; the
 * movements and Enter change nothing at all. Every one of them is consumed, so
 * a playback chord never runs from a Shows key. */
function routeRemainingKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeListItemKey(event, showsCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the mode pills (design D12): Right and Left walk the pills,
 * the first of them with no pill focused taking the end it walks from, and
 * Enter selects the focused pill as Space does. */
function routePillKey(event: KeyboardEvent, shows: GuiShowsList): KeyOutcome {
	if (event.key === 'ArrowRight') {
		moveShowsPillFocus(1);
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		moveShowsPillFocus(-1);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const pill = focusedShowsPill(shows);
		if (pill !== null) {
			requestShowsPillSelect(pill);
		}
		return 'handled';
	}
	return 'core';
}

/** Route a key to the tree list's cursor (design D14): the arrows, Page Up,
 * Page Down, Home, and End move the cursor, Right opens the node under the
 * cursor, Left closes it, and Enter toggles a node or plays an episode leaf.
 * `n` and `a` act on an episode or a flat row, because a series and a season
 * are not playable. */
function routeListKey(event: KeyboardEvent): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveShowsListCursor(move);
		return 'handled';
	}
	if (event.key === 'ArrowRight') {
		openCursorNode();
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		closeCursorNode();
		return 'handled';
	}
	if (event.key === 'Enter') {
		routeListEnter();
		return 'handled';
	}
	if (event.key === 'n' || event.key === 'a') {
		routeListItemKey(event, showsCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Right on the cursor row (design D14): a node opens. A leaf has no children,
 * so the key changes nothing. */
function openCursorNode(): void {
	const row = showsCursorRow();
	if (isShowsNodeRow(row)) {
		openShowsNode(row);
	}
}

/** Left on the cursor row (design D14): an open node closes. A closed node and
 * a leaf change nothing. */
function closeCursorNode(): void {
	const row = showsCursorRow();
	if (isShowsNodeRow(row) && showsNodeOpen(row)) {
		closeShowsNode(row);
	}
}

/** Enter on the cursor row (design D14): a series or season node toggles and
 * selects, and an episode leaf or a flat row plays. A cursor on no row sends
 * nothing. */
function routeListEnter(): void {
	const row = showsCursorRow();
	if (row === null) {
		return;
	}
	if (isShowsNodeRow(row)) {
		toggleShowsNode(row);
		selectShowsRow(row);
		return;
	}
	if (row.kind === 'flat') {
		playShowsRow(row.row.id);
		return;
	}
	playShowsRow(row.sourceId);
}

/** Route `n` and `a` to a playable row (design D14): an episode leaf or a flat
 * row. A series or a season is not playable, so its key sends no command. */
function routeListItemKey(event: KeyboardEvent, row: ShowsListRow | null): void {
	if (row === null || row.kind === 'heading') {
		return;
	}
	if (row.kind === 'series' || row.kind === 'season') {
		return;
	}
	const id = row.kind === 'flat' ? row.row.id : row.sourceId;
	if (event.key === 'n') {
		playShowsRowNext(id);
		return;
	}
	queueShowsRow(id);
}

/** Route a key to the Hero's Workspace (design D12): the movement keys move the
 * episode cursor, Left and Right step the season, Enter plays the episode, and
 * `n` and `a` play it next or queue it. */
function routeWorkspaceKey(
	event: KeyboardEvent,
	libIdx: number,
	detail: GuiShowsDetail,
): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveShowsEpisodeCursor(move, detail);
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		stepShowsSeason(-1, libIdx, detail);
		return 'handled';
	}
	if (event.key === 'ArrowRight') {
		stepShowsSeason(1, libIdx, detail);
		return 'handled';
	}
	if (event.key === 'Enter' || event.key === 'n' || event.key === 'a') {
		routeWorkspaceItemKey(event, libIdx, detail);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Run Enter, `n`, or `a` against the Workspace's selected episode (design
 * D12): the Hero owns the episode cursor, so the key names no episode id. */
function routeWorkspaceItemKey(
	event: KeyboardEvent,
	libIdx: number,
	detail: GuiShowsDetail,
): void {
	if (event.key === 'n') {
		playNextShowsEpisode(libIdx, detail);
		return;
	}
	if (event.key === 'a') {
		queueShowsEpisode(libIdx, detail);
		return;
	}
	playShowsEpisode(libIdx, detail);
}
