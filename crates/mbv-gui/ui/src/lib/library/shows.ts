// The Shows destination's interaction state (add-gui-library-panel design D1,
// D12, row 7.1): what the window content owns and the core never reads — the
// cursor, the scroll offset, the measured viewport, and the pill row's focus.
// The core owns the view mode and the selection (the frame's `pills` and
// `selected`), so nothing here mirrors them: a press and an Enter send the
// select command, and the frame carries the selection back. The row model is
// `$lib/library/shows-list`'s; the Hero's episode cursor belongs to the Hero
// (`$lib/library/shows-workspace`), and the panel's pane focus and the Hero
// overlay stay in `./state`'s shared store.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiShowsFrame, GuiShowsMode } from '$lib/bridge/types';
import { clampScrollTop, listWindow, scrollTopToShowCursor } from './list';
import type { ListMove } from './list';
import { libraryShows, libraryUi } from './state';
import {
	activeShowsMode,
	buildShowsRows,
	moveShowsCursor,
	showsLetterRail,
} from './shows-list';
import type { ShowsItemRow, ShowsListRow } from './shows-list';

/** The Shows list's interaction state. */
export interface ShowsUiState {
	/** The cursor's row id, or `null` while no row carries the cursor
	 * (design D12). The cursor moves over the item rows only. */
	cursor: string | null;
	/** The list's native scroll offset in pixels (design D12). */
	scrollTop: number;
	/** The list's measured viewport height in pixels: `0` until the list
	 * measures, and then the rows that paint are the rows in view. */
	viewportHeight: number;
	/** The mode pill the pill row's Left and Right move the focus to, or
	 * `null` while no pill has it (design D6, D12). */
	focusedPill: GuiShowsMode | null;
	/** The mode selection the keys asked for: Space and Enter select the
	 * focused pill, and the pill row turns it into the set-mode command. */
	pillSelect: GuiShowsMode | null;
	/** The `/` filter's text: the list paints only the rows whose title
	 * contains it, without case (design D7's filter rule, the rail's keyboard
	 * path). */
	filter: string;
}

/** The state a freshly mounted Shows list starts with. */
export function initialShowsUi(): ShowsUiState {
	return {
		cursor: null,
		scrollTop: 0,
		viewportHeight: 0,
		focusedPill: null,
		pillSelect: null,
		filter: '',
	};
}

/** The Shows list's interaction state. */
export const showsUi = writable<ShowsUiState>(initialShowsUi());

/** The painted rows of the carried Shows content (design D12), narrowed by
 * the state's filter text. */
function showsRows(filter = get(showsUi).filter): ShowsListRow[] {
	const shows = libraryShows();
	return shows === null ? [] : buildShowsRows(shows.rows, activeShowsMode(shows), filter);
}

/** The row the cursor rests on, or `null` while no painted row carries it. */
function cursorRow(list: readonly ShowsListRow[], cursor: string | null): ShowsItemRow | null {
	const row = list.find((candidate) => candidate.id === cursor);
	return row === undefined || row.kind === 'heading' ? null : row;
}

/** The row the Shows cursor rests on, or `null`: the keys read the row's kind
 * to decide whether Enter selects it or plays it. */
export function showsCursorRow(): ShowsItemRow | null {
	return cursorRow(showsRows(), get(showsUi).cursor);
}

/** The state once the painted rows may have narrowed (design D12): the
 * cursor keeps its row while that row still paints, and falls to the first
 * painted row otherwise, with the scroll offset showing it — the Music
 * filter's own resolve (design D7). */
function resolveShowsCursor(ui: ShowsUiState): ShowsUiState {
	const list = showsRows(ui.filter);
	const cursor = list.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveShowsCursor(list, null, 'home');
	return {
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	};
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setShowsFilter(filter: string): void {
	showsUi.update((ui) => resolveShowsCursor({ ...ui, filter }));
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closeShowsFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
	showsUi.update((ui) => resolveShowsCursor({ ...ui, filter: '' }));
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptShowsFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Select the row (design D12): the select command the core stores, whose
 * frame paints the Hero. A select repaints the Narrow arrangement's Hero
 * overlay and closes an open menu, because the facts they paint belong to
 * the previous row, as a Music pick's do. */
export function selectShowsRow(row: ShowsListRow): void {
	const shows = libraryShows();
	if (shows === null || row.kind === 'heading') {
		return;
	}
	void sendLibraryCommand({ command: 'showsSelect', libIdx: shows.libIdx, row: row.row.id });
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
	}));
}

/** Press a Shows row (design D12): the press selects the row the Hero shows,
 * as a Home press does. A heading names no row, so its press changes
 * nothing. */
export function pressShowsRow(row: ShowsListRow): void {
	selectShowsRow(row);
}

/** Play a Shows episode (design D12): Enter on a flat row, and a
 * double-click's play. The row id is the episode's stable Service id. */
export function playShowsRow(rowId: string): void {
	const shows = libraryShows();
	if (shows === null) {
		return;
	}
	void sendLibraryCommand({ command: 'showsPlay', libIdx: shows.libIdx, itemId: rowId });
}

/** Play a Shows episode next (design D12): `n` on a flat row. */
export function playShowsRowNext(rowId: string): void {
	const shows = libraryShows();
	if (shows === null) {
		return;
	}
	void sendLibraryCommand({ command: 'showsPlayNext', libIdx: shows.libIdx, itemId: rowId });
}

/** Add a Shows episode to the queue's tail (design D12): `a` on a flat row. */
export function queueShowsRow(rowId: string): void {
	const shows = libraryShows();
	if (shows === null) {
		return;
	}
	void sendLibraryCommand({ command: 'showsQueue', libIdx: shows.libIdx, itemId: rowId });
}

/** Move the Shows cursor one movement along (design D12): Up and Down move a
 * row, Page Up and Page Down move the rows in view, and Home and End take the
 * list's ends, stepping over the letter headings. The scroll offset follows,
 * so the cursor row stays fully visible. */
export function moveShowsListCursor(move: ListMove): void {
	const list = showsRows();
	const current = get(showsUi);
	const view = listWindow(list, current.scrollTop, current.viewportHeight);
	const cursor = moveShowsCursor(list, current.cursor, move, view.end - view.start);
	if (cursor === current.cursor) {
		return;
	}
	showsUi.update((ui) => ({
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	}));
}

/** Jump to a letter of the rail (design D12): the cursor moves to the first
 * series row under the letter and that row scrolls into view. A letter
 * without rows changes nothing. */
export function jumpShowsToLetter(letter: string): void {
	const list = showsRows();
	const target = showsLetterRail(list).find((entry) => entry.letter === letter)?.rowId;
	if (target === null || target === undefined) {
		return;
	}
	showsUi.update((ui) => ({
		...ui,
		cursor: target,
		scrollTop: scrollTopToShowCursor(list, target, ui.scrollTop, ui.viewportHeight),
	}));
}

/** Report the list's measured viewport height (design D12): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measureShowsList(viewportHeight: number): void {
	showsUi.update((ui) => ({
		...ui,
		viewportHeight,
		scrollTop: clampScrollTop(showsRows(), ui.scrollTop, viewportHeight),
	}));
}

/** Report the list's native scroll offset (design D12): a user scroll moves
 * the rows in view, and the offset stays inside the list's range. */
export function scrollShowsList(scrollTop: number): void {
	showsUi.update((ui) => {
		const top = clampScrollTop(showsRows(), Math.max(scrollTop, 0), ui.viewportHeight);
		return top === ui.scrollTop ? ui : { ...ui, scrollTop: top };
	});
}

/** The pill the pill row's focus holds, resolved against the carried pills so
 * a stale mode focuses nothing: the mode the keys' Space and Enter select, and
 * the pill the row paints the accent outline on (design D6, D12). */
export function focusedShowsPill(shows: GuiShowsFrame): GuiShowsMode | null {
	const { focusedPill } = get(showsUi);
	if (focusedPill === null) {
		return null;
	}
	return shows.pills.some((pill) => pill.mode === focusedPill) ? focusedPill : null;
}

/** Focus a mode pill, or no pill while `mode` is `null` (design D6,
 * D12). */
function focusShowsPill(mode: GuiShowsMode | null): void {
	showsUi.update((ui) => ({ ...ui, focusedPill: mode }));
}

/** Move the focused pill one pill along the row (design D12): Right and Left
 * walk the pills, and the first of them with no pill focused takes the end it
 * walks from. */
export function moveShowsPillFocus(step: number): void {
	const row = libraryShows()?.pills ?? [];
	if (row.length === 0) {
		return;
	}
	const at = row.findIndex((pill) => pill.mode === get(showsUi).focusedPill);
	const next =
		at === -1
			? step > 0
				? 0
				: row.length - 1
			: Math.min(Math.max(at + step, 0), row.length - 1);
	const pill = row[next];
	if (pill !== undefined) {
		focusShowsPill(pill.mode);
	}
}

/** Ask for the focused pill to be selected: Space and Enter request it
 * (design D6, D12). */
export function requestShowsPillSelect(mode: GuiShowsMode): void {
	showsUi.update((ui) => ({ ...ui, pillSelect: mode }));
}

/** Select the view mode `mode` of the Shows library `libIdx` (design D12): one
 * mode is always selected. The core stores the choice, loads the mode's root
 * level, and the pills paint the core's choice back, so the row and the list
 * cannot disagree. */
export function selectShowsPill(libIdx: number, mode: GuiShowsMode): void {
	void sendLibraryCommand({ command: 'showsSetMode', libIdx, mode });
}

/** Turn the keys' pill request into the set-mode command the pill sends
 * (design D12). The intent is consumed here, so one key sends one command. */
export function applyShowsPillSelect(libIdx: number): void {
	const { pillSelect } = get(showsUi);
	if (pillSelect === null) {
		return;
	}
	showsUi.update((ui) => ({ ...ui, pillSelect: null }));
	selectShowsPill(libIdx, pillSelect);
}
