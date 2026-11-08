// The Library panel's key routing (add-gui-library-panel design D3): the router
// the window content installs in `input.ts`, where it runs before the forwarder
// and decides whether the core ever sees a keydown. The order is the design's:
// an open core overlay or an armed prefix keeps the keyboard with the core, an
// open text field of the panel types, the window keys (Tab across the panes,
// `1`–`7` for the destination tabs) and the Library keys run in the front end
// while the Library panel shows, and every other key goes to the core.

import { get } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiFrame, GuiLibraryTab, GuiPanel } from '$lib/bridge/types';
import type { ListMove } from './list';
import {
	acceptLibraryFilter,
	closeLibraryFilter,
	closeLibraryHero,
	closeLibraryItemMenu,
	focusLibraryPane,
	focusLibraryPill,
	focusedItemId,
	focusedLibraryPill,
	heroOverlayOpen,
	libraryContent,
	libraryMusic,
	libraryShown,
	libraryUi,
	moveLibraryCursor,
	moveLibraryPane,
	moveLibraryTrackCursor,
	openLibraryFilter,
	pickLibraryCursor,
	requestLibraryPillToggle,
	setLibraryCursorOpen,
} from './state';
import type { LibraryUiState } from './state';

/** The frame facts the routing reads. A slice `GuiFrame` satisfies it whole, so
 * the installer passes the frame it holds; a test builds only these fields. */
export type LibraryKeyFrame = Pick<
	GuiFrame,
	'focus' | 'overlay' | 'status' | 'library'
> & {
	summary: Pick<GuiFrame['summary'], 'filterActive'>;
};

/** The keys the Library keeps from the core's router (design D3, step 4): the
 * movements, Right and Left's open and close, and Enter. Each pane acts on the
 * ones it owns — the list's cursor or the Hero's tracklist — and the others
 * change nothing but still never reach the core. `n` and `a` are the Library's
 * too, and `routeItemKey` answers them for every pane. */
const LIST_AND_HERO_KEYS: ReadonlySet<string> = new Set([
	'Enter',
	'ArrowUp',
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'PageUp',
	'PageDown',
	'Home',
	'End',
]);

/** The list movement each cursor key names (design D3, D7). */
const CURSOR_MOVES: Readonly<Record<string, ListMove>> = {
	ArrowUp: 'up',
	ArrowDown: 'down',
	PageUp: 'pageUp',
	PageDown: 'pageDown',
	Home: 'home',
	End: 'end',
};

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/** The carried destination tabs (design D2): the tabs the top bar paints, and
 * the ones the digits open. */
function carriedTabs(): readonly GuiLibraryTab[] {
	return get(libraryContent)?.tabs ?? [];
}

/** The place of the destination tab a digit opens (design D3, step 3): `1`
 * opens the first tab after Home, so the digit is the tab's place in the
 * carried list rather than a tab position. */
function destinationPlace(event: KeyboardEvent): number | null {
	if (!bare(event) || event.key.length !== 1) {
		return null;
	}
	const place = Number(event.key);
	return place >= 1 && place <= 7 ? place : null;
}

/** Move the pane focus (design D3, step 3): Tab and Shift+Tab walk the pill
 * row, the list, and the Hero. The ends of that walk cross the panel boundary
 * (design D4): Tab past the Hero and Shift+Tab before the pill row give the
 * Queue panel Panel focus, and Queue Panel focus gives the Library panel's
 * near end the keyboard, because Panel focus lives in the core. */
function takePaneFocus(panel: GuiPanel, step: number): void {
	if (panel === 'queue') {
		sendLibraryCommand({ command: 'takeFocus' });
		focusLibraryPane(step > 0 ? 'pills' : 'hero');
		return;
	}
	const { pane } = get(libraryUi);
	if ((step > 0 && pane === 'hero') || (step < 0 && pane === 'pills')) {
		sendLibraryCommand({ command: 'takeQueueFocus' });
		return;
	}
	moveLibraryPane(step);
}

/** The pills of the carried content. */
function pills() {
	return get(libraryContent)?.music?.pills ?? [];
}

/** Move the focused pill one pill along the row (design D3, step 4): Right and
 * Left walk the pills, and the first of them with no pill focused takes the
 * end it walks from. */
function movePillFocus(step: number): void {
	const row = pills();
	if (row.length === 0) {
		return;
	}
	const at = row.findIndex((pill) => pill.groupId === get(libraryUi).focusedPill);
	const next =
		at === -1
			? step > 0
				? 0
				: row.length - 1
			: Math.min(Math.max(at + step, 0), row.length - 1);
	const pill = row[next];
	if (pill) {
		focusLibraryPill(pill.groupId);
	}
}

/** Route a key while the filter field holds the keyboard (design D3, step 2). */
function routeFilterKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === 'Escape') {
		closeLibraryFilter();
		return 'handled';
	}
	if (event.key === 'Enter') {
		acceptLibraryFilter();
		return 'handled';
	}
	// The field takes the rest: the browser types the key, and the core gets
	// none.
	return 'text';
}

/** Route a movement, open and close, or pick key to the list's cursor (design
 * D3, D7): the arrows, Page Up, Page Down, Home, and End move the cursor, Right
 * opens the cursor's node, Left closes it, and Enter picks the item under it. */
function routeListKey(event: KeyboardEvent): boolean {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveLibraryCursor(move);
		return true;
	}
	if (event.key === 'ArrowRight') {
		setLibraryCursorOpen(true);
		return true;
	}
	if (event.key === 'ArrowLeft') {
		setLibraryCursorOpen(false);
		return true;
	}
	if (event.key === 'Enter') {
		pickLibraryCursor();
		return true;
	}
	return false;
}

/** Route a movement or Enter key to the Hero's tracklist cursor (design D3):
 * the movement keys move the tracklist's cursor, and Enter plays the track it
 * rests on. */
function routeTracklistKey(event: KeyboardEvent): boolean {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveLibraryTrackCursor(move);
		return true;
	}
	if (event.key === 'Enter') {
		const itemId = focusedItemId();
		if (itemId !== null) {
			void sendLibraryCommand({ command: 'play', itemId });
		}
		return true;
	}
	return false;
}

/** Route `n` and `a` to the item the focused pane's cursor names (design D3,
 * step 4, D8): play next and add to queue. The Library takes both keys while
 * it has Library Panel focus, so the core's `a` chord never runs from here;
 * with no item under the cursor the key sends no command. */
function routeItemKey(event: KeyboardEvent): void {
	const itemId = focusedItemId();
	if (itemId === null) {
		return;
	}
	if (event.key === 'n') {
		void sendLibraryCommand({ command: 'playNext', itemId });
		return;
	}
	void sendLibraryCommand({ command: 'addToQueue', itemId });
}

/** Close the front end's open menu or overlay with Esc (design D3): the picked
 * item's menu first, then the Narrow arrangement's Hero overlay. Whether one
 * paints decides the answer, so an Esc with nothing open stays the core's. */
function routeEscapeKey(ui: LibraryUiState): KeyOutcome {
	if (ui.itemMenuOpen) {
		closeLibraryItemMenu();
		return 'handled';
	}
	if (heroOverlayOpen(ui)) {
		closeLibraryHero();
		return 'handled';
	}
	return 'core';
}

/** Route a key to the Library keys (design D3, step 4), with the pane the
 * window content focused deciding which control the key acts on. */
function routePanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	if (event.key === 'Escape' && bare(event)) {
		return routeEscapeKey(ui);
	}
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	// Space toggles the focused pill and Alt+Space solos it (design D6). With
	// no pill focused, it is the core's: Space plays and pauses (design D3,
	// step 5).
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = focusedLibraryPill(pills(), ui);
		if (pill === null) {
			return 'core';
		}
		requestLibraryPillToggle(pill.groupId, event.altKey);
		return 'handled';
	}
	if (ui.pane === 'pills' && bare(event)) {
		if (event.key === 'ArrowRight') {
			movePillFocus(1);
			return 'handled';
		}
		if (event.key === 'ArrowLeft') {
			movePillFocus(-1);
			return 'handled';
		}
		// Enter presses the focused pill, as Space does (design D3, step 4, D6).
		if (event.key === 'Enter') {
			const pill = focusedLibraryPill(pills(), ui);
			if (pill !== null) {
				requestLibraryPillToggle(pill.groupId, false);
				return 'handled';
			}
		}
	}
	if (ui.pane === 'list' && bare(event) && routeListKey(event)) {
		return 'handled';
	}
	if (ui.pane === 'hero' && bare(event) && routeTracklistKey(event)) {
		return 'handled';
	}
	if (bare(event) && (event.key === 'n' || event.key === 'a')) {
		routeItemKey(event);
		return 'handled';
	}
	if (bare(event) && LIST_AND_HERO_KEYS.has(event.key)) {
		return 'handled';
	}
	// Nothing else of the Library's answers this key: the core's router does.
	return 'core';
}

/**
 * Route one keydown (design D3): the front end does its part, and the answer is
 * what the input forwarder must do with the key.
 */
export function routeLibraryKey(
	event: KeyboardEvent,
	frame: LibraryKeyFrame | null,
): KeyOutcome {
	// The attach states carry no slice, and no Library panel means no Library
	// keys: the core takes every one of them. The panel's presence is the
	// window's mount fact (design D2, D10), so a key never acts on a panel that
	// paints nothing.
	if (!frame || !libraryShown(frame.library, libraryMusic())) {
		return 'core';
	}
	// (1) An open core overlay or an armed prefix keeps the keyboard with the
	// core: both are in the frame, and the core's own router answers them.
	if (frame.overlay !== null || frame.status.prefixArmed) {
		return 'core';
	}
	// The Queue filter's text entry is the core's, so while it holds the
	// keyboard every key is the core's to type (`text_entry_focused` in
	// `src/app/input/gui.rs`).
	if (frame.summary.filterActive) {
		return 'core';
	}
	// (2) An open text field of the panel takes the keys.
	const ui = get(libraryUi);
	if (ui.filterOpen) {
		return routeFilterKey(event);
	}
	// (3) The window keys run whenever the Library panel shows.
	if (event.key === 'Tab' && !event.ctrlKey && !event.metaKey && !event.altKey) {
		takePaneFocus(frame.focus.panel, event.shiftKey ? -1 : 1);
		return 'handled';
	}
	const place = destinationPlace(event);
	if (place !== null) {
		// A digit names a carried tab, never a raw position: it opens the tab
		// the top bar paints at that place, and a digit past the carried tabs
		// opens nothing (design D3, step 3).
		const tab = carriedTabs()[place - 1];
		if (tab !== undefined) {
			sendLibraryCommand({ command: 'openDestination', position: tab.position });
		}
		return 'handled';
	}
	// (4) The Library keys run while the Library panel has Panel focus.
	if (frame.focus.panel === 'library') {
		return routePanelKey(event, ui);
	}
	// (5) Every other key goes to the core's keyboard router.
	return 'core';
}
