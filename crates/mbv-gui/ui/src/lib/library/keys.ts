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
import type { GuiFrame, GuiPanel } from '$lib/bridge/types';
import {
	acceptLibraryFilter,
	closeLibraryFilter,
	focusLibraryPane,
	focusLibraryPill,
	focusedLibraryPill,
	libraryContent,
	libraryUi,
	moveLibraryPane,
	openLibraryFilter,
	requestLibraryPillToggle,
} from './state';
import type { LibraryPane } from './state';

/** The frame facts the routing reads. A slice `GuiFrame` satisfies it whole, so
 * the installer passes the frame it holds; a test builds only these fields. */
export type LibraryKeyFrame = Pick<
	GuiFrame,
	'focus' | 'overlay' | 'status' | 'library'
> & {
	summary: Pick<GuiFrame['summary'], 'filterActive'>;
};

/** The keys whose target the list and the Hero own (rows 4.5 and 4.7): Enter
 * plays or presses the item under the cursor, `n` and `a` act on it, and the
 * movement keys move the cursor or open and close a node. The Library takes
 * them, so the core never reads them with Library Panel focus (design D3,
 * step 4); the components that act on them arrive with those rows. */
const LIST_AND_HERO_KEYS: ReadonlySet<string> = new Set([
	'Enter',
	'n',
	'a',
	'ArrowUp',
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'PageUp',
	'PageDown',
	'Home',
	'End',
]);

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/** The destination tab a digit opens (design D3, step 3): `1` opens the first
 * tab after Home, so the digit is the position the frame's tabs are indexed
 * by. */
function destinationPosition(event: KeyboardEvent): number | null {
	if (!bare(event) || event.key.length !== 1) {
		return null;
	}
	const position = Number(event.key);
	return position >= 1 && position <= 7 ? position : null;
}

/** Move the pane focus (design D3, step 3): Tab and Shift+Tab walk the pill
 * row, the list, and the Hero. Tab across the panel boundary sends the
 * take-focus command, because Panel focus lives in the core (design D4). */
function takePaneFocus(panel: GuiPanel, step: number): void {
	if (panel === 'queue') {
		sendLibraryCommand({ command: 'takeFocus' });
		focusLibraryPane(step > 0 ? 'pills' : 'hero');
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

/** Route a key to the Library keys (design D3, step 4), with the pane the
 * window content focused deciding which control the key acts on. */
function routePanelKey(event: KeyboardEvent, pane: LibraryPane): KeyOutcome {
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	// Space toggles the focused pill and Alt+Space solos it (design D6). With
	// no pill focused, it is the core's: Space plays and pauses (design D3,
	// step 5).
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = focusedLibraryPill(pills(), get(libraryUi));
		if (pill === null) {
			return 'core';
		}
		requestLibraryPillToggle(pill.groupId, event.altKey);
		return 'handled';
	}
	if (pane === 'pills' && bare(event)) {
		if (event.key === 'ArrowRight') {
			movePillFocus(1);
			return 'handled';
		}
		if (event.key === 'ArrowLeft') {
			movePillFocus(-1);
			return 'handled';
		}
	}
	if (bare(event) && LIST_AND_HERO_KEYS.has(event.key)) {
		return 'handled';
	}
	// Esc closes the open filter, overlay, or menu. The filter closes above,
	// the overlay is the core's, and no other menu is open here.
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
	// keys: the core takes the key.
	if (!frame) {
		return 'core';
	}
	// (1) An open core overlay or an armed prefix keeps the keyboard with the
	// core: both are in the frame, and the core's own router answers them.
	if (frame.overlay !== null || frame.status.prefixArmed) {
		return 'core';
	}
	if (frame.library === null) {
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
	const destination = destinationPosition(event);
	if (destination !== null) {
		sendLibraryCommand({ command: 'openDestination', position: destination });
		return 'handled';
	}
	// (4) The Library keys run while the Library panel has Panel focus.
	if (frame.focus.panel === 'library') {
		return routePanelKey(event, ui.pane);
	}
	// (5) Every other key goes to the core's keyboard router.
	return 'core';
}
