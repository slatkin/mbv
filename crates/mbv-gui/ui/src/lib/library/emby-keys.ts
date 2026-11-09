// The plain Emby-library destination's key routing (add-gui-library-panel
// design D3, D12, row 8.1): the Library keys while an Emby library is active.
// `./keys` stays the panel's one routing site and hands an Emby key here after
// the window keys and the open-field checks, so this module never installs a
// router. The pane focus decides the control a key acts on: the feed-group
// pills, the list, or the Hero — which carries no Workspace and no cursor.
//
// Everything else matches the Music router: the movements and Enter never reach
// the core, Esc's overlay and menu handling runs before this, the Emby library
// paints no filter in Phase 2, and Space with no focused pill stays the core's
// play and pause.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiEmbyFrame } from '$lib/bridge/types';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryEmby } from './state';
import {
	embyCursorRow,
	focusedEmbyPill,
	moveEmbyListCursor,
	moveEmbyPillFocus,
	playEmbyRow,
	playEmbyRowNext,
	queueEmbyRow,
	requestEmbyPillSelect,
} from './emby';
import type { EmbyListRow } from './emby-list';

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/**
 * Route a key to the Emby library's keys (row 8.1): the feed-group pills walk
 * their groups and select the focused one, the list's cursor moves and Enter
 * plays the row under it — a folder or a boxset too, through the core's
 * folder-play rule — and the Hero consumes the movements it has no cursor for.
 * `n` and `a` act on the item the focused pane names: the list's cursor row, or
 * the Hero's selected row. Every one of them is consumed, so no Library key
 * reaches the core's playback chords from any pane.
 */
export function routeEmbyPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	const emby = libraryEmby();
	if (emby === null) {
		return 'core';
	}
	// The Emby library paints no filter in Phase 2 (row 8.1): `/` keeps its core
	// effect, as it does on Home and Shows.
	if (event.key === '/') {
		return 'core';
	}
	// Space selects the focused pill (design D6). With no pill focused — and in
	// plain mode, which paints none — it is the core's: Space plays and pauses.
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = focusedEmbyPill(emby);
		if (pill === null) {
			return 'core';
		}
		requestEmbyPillSelect(pill);
		return 'handled';
	}
	if (!bare(event)) {
		return 'core';
	}
	if (ui.pane === 'pills' && emby.feedGroup) {
		return orRemaining(routePillKey(event, emby), event);
	}
	if (ui.pane === 'list') {
		return routeListKey(event);
	}
	if (ui.pane === 'hero' && emby.hero !== null) {
		return routeHeroKey(event, emby);
	}
	// The Hero pane a frame without a Hero does not paint, and any pane that
	// left a key unclaimed, still keep the Library's keys from the core: the
	// movements change nothing, Enter plays nothing, and `n` and `a` act on the
	// list's cursor row.
	return routeRemainingKey(event);
}

/** The key's answer, or the remaining Library keys' answer when the pane left
 * it unclaimed: a pane that does not act on a key never passes it to the core's
 * playback chords (design D3, step 4). */
function orRemaining(outcome: KeyOutcome, event: KeyboardEvent): KeyOutcome {
	return outcome === 'core' ? routeRemainingKey(event) : outcome;
}

/** The Library's own keys no Emby pane claims (design D3, step 4): `n` and `a`
 * act on the list's cursor row, as the Music router's item key acts from every
 * pane, and every row is playable so the key always has a row to act on; the
 * movements and Enter change nothing at all. Every one of them is consumed, so
 * a playback chord never runs from an Emby key. */
function routeRemainingKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeListItemKey(event, embyCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the feed-group pills (row 8.1): Right and Left walk the
 * pills, the first of them with no pill focused taking the end it walks from,
 * and Enter selects the focused pill as Space does. */
function routePillKey(event: KeyboardEvent, emby: GuiEmbyFrame): KeyOutcome {
	if (event.key === 'ArrowRight') {
		moveEmbyPillFocus(1);
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		moveEmbyPillFocus(-1);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const pill = focusedEmbyPill(emby);
		if (pill !== null) {
			requestEmbyPillSelect(pill);
		}
		return 'handled';
	}
	return 'core';
}

/** Route a key to the list's cursor (row 8.1): the arrows, Page Up, Page Down,
 * Home, and End move the cursor, Enter plays the row it rests on, and `n` and
 * `a` play it next or queue it. Every row is playable, a folder and a boxset
 * included. */
function routeListKey(event: KeyboardEvent): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveEmbyListCursor(move);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const row = embyCursorRow();
		if (row !== null) {
			playEmbyRow(row.row.id);
		}
		return 'handled';
	}
	if (event.key === 'n' || event.key === 'a') {
		routeListItemKey(event, embyCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route `n` and `a` to the row's item (row 8.1): play next and add to queue.
 * A cursor on no row sends no command. */
function routeListItemKey(event: KeyboardEvent, row: EmbyListRow | null): void {
	routeItemKey(event, row === null ? null : row.id);
}

/** Route `n` and `a` to one row's item (row 8.1): the list's cursor row, or the
 * row the Hero paints. */
function routeItemKey(event: KeyboardEvent, itemId: string | null): void {
	if (itemId === null) {
		return;
	}
	if (event.key === 'n') {
		playEmbyRowNext(itemId);
		return;
	}
	queueEmbyRow(itemId);
}

/** Route a key to the Hero (row 8.1): it carries no Workspace and no cursor, so
 * the movements and Enter change nothing but are consumed, and `n` and `a` act
 * on the row the core selected — the row the Hero paints. */
function routeHeroKey(event: KeyboardEvent, emby: GuiEmbyFrame): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeItemKey(event, emby.selected);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}
