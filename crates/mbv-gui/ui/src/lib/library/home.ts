// The Home destination's interaction state (add-gui-library-panel design D1,
// D12): what the window content owns and the core never reads — the cursor.
// The scroll offset and the measured viewport live in `./scroll` (design D9).
// The core owns the selection
// (the frame's `selected` row), so nothing here mirrors it: a press and the
// cursor's settle send the select command, and the frame carries the
// selection back. The row model is `$lib/library/home-list`'s; the panel's
// pane focus and the Hero overlay stay in `./state`'s shared store.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiHomeRowId } from '$lib/bridge/types';
import { homeRowsOf, moveHomeCursor } from './home-list';
import type { HomeListRow } from './home-list';
import type { ListMove } from './list';
import { clampScrollTop, listWindow, scrollTopToShowCursor } from './list';
import { libraryHomeList } from './parts';
import { libraryUi } from './state';
import { homeScroll } from './scroll';

/** The Home list's interaction state. */
export interface HomeUiState {
	/** The cursor's row key, or `null` while no row carries the cursor
	 * (design D12). The cursor moves over the item rows only. */
	cursor: string | null;
	/** The `/` filter's text: the list paints only the item rows whose title
	 * contains it, without case (design D7's filter rule). */
	filter: string;
}

/** The state a freshly mounted Home list starts with. */
export function initialHomeUi(): HomeUiState {
	return { cursor: null, filter: '' };
}

/** The Home list's interaction state. */
export const homeUi = writable<HomeUiState>(initialHomeUi());

/** The painted rows of the carried Home content (design D12), narrowed by the
 * state's filter text. */
function homeRows(filter = get(homeUi).filter): HomeListRow[] {
	const home = libraryHomeList();
	return home === null ? [] : homeRowsOf(home.rows, filter);
}

/** The row the cursor rests on, or `null` while no painted row carries it. */
function cursorRow(list: HomeListRow[], cursor: string | null): HomeListRow | null {
	return list.find((row) => row.id === cursor) ?? null;
}

/** The item row id the Home cursor rests on (design D12): the opaque
 * identity the play, play-next, and add-to-queue keys carry. `null` while
 * the cursor rests on no item row. */
export function homeCursorRowId(): GuiHomeRowId | null {
	const row = cursorRow(homeRows(), get(homeUi).cursor);
	return row !== null && row.kind === 'item' ? row.rowId : null;
}

/** Scroll the list so the cursor's row paints fully, in the scroll store
 * (design D12, D9). */
function followHomeCursor(list: readonly HomeListRow[], cursor: string | null): void {
	homeScroll.update((scroll) => {
		const scrollTop = scrollTopToShowCursor(list, cursor, scroll.scrollTop, scroll.viewportHeight);
		return scrollTop === scroll.scrollTop ? scroll : { ...scroll, scrollTop };
	});
}

/** Settle the cursor once the painted rows may have narrowed (design D12): the
 * cursor keeps its row while that row still paints, and falls to the first
 * item row otherwise. The scroll store then shows the cursor — the Music
 * filter's own resolve (design D7). */
function settleHomeCursor(): void {
	const ui = get(homeUi);
	const list = homeRows(ui.filter);
	const cursor = list.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveHomeCursor(list, null, 'home');
	if (cursor !== ui.cursor) {
		homeUi.update((current) => ({ ...current, cursor }));
	}
	followHomeCursor(list, cursor);
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows, and its settle selects the
 * row it rests on, as a cursor move does. */
export function setHomeFilter(filter: string): void {
	const before = get(homeUi).cursor;
	homeUi.update((ui) => ({ ...ui, filter }));
	settleHomeCursor();
	const after = get(homeUi).cursor;
	if (after !== before) {
		settleSelection(cursorRow(homeRows(), after));
	}
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows,
 * and its settle selects the row it rests on. */
export function closeHomeFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
	const before = get(homeUi).cursor;
	homeUi.update((ui) => ({ ...ui, filter: '' }));
	settleHomeCursor();
	const after = get(homeUi).cursor;
	if (after !== before) {
		settleSelection(cursorRow(homeRows(), after));
	}
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptHomeFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Select the row (design D12): the select command the core stores, whose
 * frame paints the Hero. A select repaints the Narrow arrangement's Hero
 * overlay and closes an open menu, because the facts they paint belong to the
 * previous row (the pick's reset, as Music's does). */
function settleSelection(row: HomeListRow | null): void {
	if (row === null || row.kind !== 'item') {
		return;
	}
	void sendLibraryCommand({ command: 'homeSelect', row: row.rowId });
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
	}));
}

/** Press a Home row (design D12): the press selects, and Enter or a press on
 * an already-selected row plays it — the press never plays on its own. A
 * heading or spacer names no item, so its press changes nothing. */
export function pressHomeRow(row: HomeListRow): void {
	settleSelection(row);
}

/** Play a Home row (design D12): Enter's play, and a double-click's. */
export function playHomeRow(rowId: GuiHomeRowId): void {
	void sendLibraryCommand({ command: 'homePlay', row: rowId });
}

/** Enqueue a Home row directly after the active slot, or at the tail with no
 * active slot (design D12): `n` and the Hero's Play next. */
export function playHomeRowNext(rowId: GuiHomeRowId): void {
	void sendLibraryCommand({ command: 'homePlayNext', row: rowId });
}

/** Add a Home row to the queue's tail (design D12): `a` and the Hero's Add to
 * queue. */
export function queueHomeRow(rowId: GuiHomeRowId): void {
	void sendLibraryCommand({ command: 'homeQueue', row: rowId });
}

/** Move the Home cursor one movement along (design D12): Up and Down move an
 * item row, Page Up and Page Down move the rows in view, and Home and End
 * take the list's ends, stepping over the headings and spacers. The scroll
 * offset follows, so the cursor row stays fully visible, and the cursor's
 * settle selects the row it rests on — the Hero follows the cursor. */
export function moveHomeListCursor(move: ListMove): void {
	const list = homeRows();
	const current = get(homeUi);
	const scroll = get(homeScroll);
	const view = listWindow(list, scroll.scrollTop, scroll.viewportHeight);
	const cursor = moveHomeCursor(list, current.cursor, move, view.end - view.start);
	if (cursor === current.cursor) {
		return;
	}
	homeUi.update((ui) => ({ ...ui, cursor }));
	followHomeCursor(list, cursor);
	settleSelection(cursorRow(list, cursor));
}

/** Report the Home list's measured viewport height (design D12): the rows in
 * view follow it, and the scroll offset stays inside the narrower range. */
export function measureHomeList(viewportHeight: number): void {
	homeScroll.update((scroll) => ({
		...scroll,
		viewportHeight,
		scrollTop: clampScrollTop(homeRows(), scroll.scrollTop, viewportHeight),
	}));
}

/** Report the Home list's native scroll offset (design D12): a user scroll
 * moves the rows in view, and the offset stays inside the list's range. */
export function scrollHomeList(scrollTop: number): void {
	homeScroll.update((scroll) => {
		const top = clampScrollTop(homeRows(), Math.max(scrollTop, 0), scroll.viewportHeight);
		return top === scroll.scrollTop ? scroll : { ...scroll, scrollTop: top };
	});
}
