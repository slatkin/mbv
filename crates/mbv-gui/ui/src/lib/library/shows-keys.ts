// The Shows destination's key routing (add-gui-library-panel design D3, D12,
// row 7.1): the Library keys while a Shows library is active. `./keys` stays
// the panel's one routing site and hands a Shows key here after the window keys
// and the open-field checks, so this module never installs a router. The pane
// focus decides the control a key acts on: the mode pills, the list, or the
// Hero's Workspace.
//
// Everything else matches the Music router: the movements and Enter never reach
// the core, Esc's overlay and menu handling runs before this, Shows paints no
// filter in Phase 2, and Space with no focused pill stays the core's play and
// pause.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiShowsFrame } from '$lib/bridge/types';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryShows } from './state';
import {
	focusedShowsPill,
	moveShowsListCursor,
	moveShowsPillFocus,
	playShowsRow,
	playShowsRowNext,
	queueShowsRow,
	requestShowsPillSelect,
	selectShowsRow,
	showsCursorRow,
} from './shows';
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
	const shows = libraryShows();
	if (shows === null) {
		return 'core';
	}
	// Shows paints no filter in Phase 2 (row 7.1): `/` keeps its core effect,
	// as it does on Home.
	if (event.key === '/') {
		return 'core';
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
		return routePillKey(event, shows);
	}
	if (ui.pane === 'list') {
		return routeListKey(event);
	}
	if (ui.pane === 'hero' && shows.hero !== null) {
		return routeWorkspaceKey(event, shows);
	}
	// The arrow keys and Enter the pane owns answered above; the rest of the
	// Library's keys never reach the core (design D3, step 4).
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the mode pills (design D12): Right and Left walk the pills,
 * the first of them with no pill focused taking the end it walks from, and
 * Enter selects the focused pill as Space does. */
function routePillKey(event: KeyboardEvent, shows: GuiShowsFrame): KeyOutcome {
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

/** Route a key to the list's cursor (design D12): the arrows, Page Up, Page
 * Down, Home, and End move the cursor, and Enter selects the series row it
 * rests on or plays the flat row. `n` and `a` act on a flat row only, because a
 * series row is not playable. */
function routeListKey(event: KeyboardEvent): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveShowsListCursor(move);
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

/** Enter on the cursor row (design D12): a series row selects, and a flat row
 * plays. A cursor on no row sends nothing. */
function routeListEnter(): void {
	const row = showsCursorRow();
	if (row === null) {
		return;
	}
	if (row.kind === 'series') {
		selectShowsRow(row);
		return;
	}
	playShowsRow(row.row.id);
}

/** Route `n` and `a` to a flat row's episode (design D12): play next and add to
 * queue. A series row is not playable, so its key sends no command. */
function routeListItemKey(event: KeyboardEvent, row: ShowsListRow | null): void {
	if (row === null || row.kind !== 'flat') {
		return;
	}
	if (event.key === 'n') {
		playShowsRowNext(row.row.id);
		return;
	}
	queueShowsRow(row.row.id);
}

/** Route a key to the Hero's Workspace (design D12): the movement keys move the
 * episode cursor, Left and Right step the season, Enter plays the episode, and
 * `n` and `a` play it next or queue it. */
function routeWorkspaceKey(event: KeyboardEvent, shows: GuiShowsFrame): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveShowsEpisodeCursor(move, shows);
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		stepShowsSeason(-1, shows);
		return 'handled';
	}
	if (event.key === 'ArrowRight') {
		stepShowsSeason(1, shows);
		return 'handled';
	}
	if (event.key === 'Enter' || event.key === 'n' || event.key === 'a') {
		routeWorkspaceItemKey(event, shows);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Run Enter, `n`, or `a` against the Workspace's selected episode (design
 * D12): the Hero owns the episode cursor, so the key names no episode id. */
function routeWorkspaceItemKey(event: KeyboardEvent, shows: GuiShowsFrame): void {
	if (event.key === 'n') {
		playNextShowsEpisode(shows);
		return;
	}
	if (event.key === 'a') {
		queueShowsEpisode(shows);
		return;
	}
	playShowsEpisode(shows);
}
