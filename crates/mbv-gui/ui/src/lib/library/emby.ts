// The plain Emby-library destination's interaction state (add-gui-library-panel
// design D12, row 8.1): what the window content owns and the core never reads —
// the cursor, the scroll offset, the measured viewport, and the feed-group pill
// row's focus. The core owns the selected row and the feed-group selection (the
// frame's `selected` and `pills`), so nothing here mirrors them: a press and an
// Enter send the command, and the frame carries the choice back. The row model
// is `$lib/library/emby-list`'s; the panel's pane focus and the Hero overlay
// stay in `./state`'s shared store, and the Hero carries no Workspace and no
// cursor of its own.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiEmbyFeedGroup, GuiEmbyFrame } from '$lib/bridge/types';
import { clampScrollTop, listWindow, scrollTopToShowCursor } from './list';
import type { ListMove } from './list';
import {
	buildEmbyRows,
	embyGroupKey,
	embyLetterRail,
	moveEmbyCursor,
} from './emby-list';
import type { EmbyItemRow, EmbyListRow } from './emby-list';
import { libraryEmby, libraryUi } from './state';

/** The Emby list's interaction state. */
export interface EmbyUiState {
	/** The cursor's row id, or `null` while no row carries the cursor (row
	 * 8.1). The cursor moves over the item rows only. */
	cursor: string | null;
	/** The list's native scroll offset in pixels (row 8.1). */
	scrollTop: number;
	/** The list's measured viewport height in pixels: `0` until the list
	 * measures, and then the rows that paint are the rows in view. */
	viewportHeight: number;
	/** The feed-group pill the pill row's Left and Right move the focus to, as
	 * the group's stable key (`embyGroupKey`), or `null` while no pill has it
	 * (design D6, row 8.1). */
	focusedPill: string | null;
	/** The group the keys asked for: Space and Enter select the focused pill,
	 * and the pill row turns it into the set-feed-group command. */
	pillSelect: GuiEmbyFeedGroup | null;
	/** The `/` filter's text: the list paints only the rows whose title
	 * contains it, without case (design D7's filter rule, the rail's keyboard
	 * path). */
	filter: string;
}

/** The state a freshly mounted Emby list starts with. */
export function initialEmbyUi(): EmbyUiState {
	return {
		cursor: null,
		scrollTop: 0,
		viewportHeight: 0,
		focusedPill: null,
		pillSelect: null,
		filter: '',
	};
}

/** The Emby list's interaction state. */
export const embyUi = writable<EmbyUiState>(initialEmbyUi());

/** The painted rows of the carried Emby content (row 8.1): the plain mode's
 * rows under their letter headings, or the feed-group mode's flat rows. */
function embyRows(filter = get(embyUi).filter): EmbyListRow[] {
	const emby = libraryEmby();
	return emby === null ? [] : buildEmbyRows(emby.rows, emby.feedGroup, filter);
}

/** The row the cursor rests on, or `null` while no painted row carries it. */
function cursorRow(list: readonly EmbyListRow[], cursor: string | null): EmbyItemRow | null {
	const row = list.find((candidate) => candidate.id === cursor);
	return row === undefined || row.kind === 'heading' ? null : row;
}

/** The row the Emby cursor rests on, or `null` (row 8.1): the keys read it to
 * play the row, play it next, or queue it. */
export function embyCursorRow(): EmbyItemRow | null {
	return cursorRow(embyRows(), get(embyUi).cursor);
}

/** The state once the painted rows may have narrowed (row 8.1): the cursor
 * keeps its row while that row still paints, and falls to the first painted
 * row otherwise, with the scroll offset showing it — the Music filter's own
 * resolve (design D7). */
function resolveEmbyCursor(ui: EmbyUiState): EmbyUiState {
	const list = embyRows(ui.filter);
	const cursor = list.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveEmbyCursor(list, null, 'home');
	return {
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	};
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setEmbyFilter(filter: string): void {
	embyUi.update((ui) => resolveEmbyCursor({ ...ui, filter }));
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closeEmbyFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
	embyUi.update((ui) => resolveEmbyCursor({ ...ui, filter: '' }));
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptEmbyFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Select the row (row 8.1): the select command the core stores, whose frame
 * paints the Hero. A select repaints the Narrow arrangement's Hero overlay and
 * closes an open menu, because the facts they paint belong to the previous row,
 * as a Music pick's do. */
export function selectEmbyRow(row: EmbyListRow): void {
	const emby = libraryEmby();
	if (emby === null || row.kind === 'heading') {
		return;
	}
	void sendLibraryCommand({ command: 'embySelect', libIdx: emby.libIdx, row: row.row.id });
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
	}));
}

/** Press a row (row 8.1): the press selects the row the Hero shows, as a Home
 * press does. A heading names no row, so its press changes nothing. */
export function pressEmbyRow(row: EmbyListRow): void {
	selectEmbyRow(row);
}

/** Play a row (row 8.1): Enter on the cursor row, and a double-click's play.
 * Every row is playable, a folder and a boxset included, because the core's
 * folder-play rule expands a collection row. The row id is the item's stable
 * Service id. */
export function playEmbyRow(rowId: string): void {
	const emby = libraryEmby();
	if (emby === null) {
		return;
	}
	void sendLibraryCommand({ command: 'embyPlay', libIdx: emby.libIdx, itemId: rowId });
}

/** Play a row next (row 8.1): `n` on the cursor row. */
export function playEmbyRowNext(rowId: string): void {
	const emby = libraryEmby();
	if (emby === null) {
		return;
	}
	void sendLibraryCommand({ command: 'embyPlayNext', libIdx: emby.libIdx, itemId: rowId });
}

/** Add a row to the queue's tail (row 8.1): `a` on the cursor row. */
export function queueEmbyRow(rowId: string): void {
	const emby = libraryEmby();
	if (emby === null) {
		return;
	}
	void sendLibraryCommand({ command: 'embyQueue', libIdx: emby.libIdx, itemId: rowId });
}

/** Move the list's cursor one movement along (row 8.1): Up and Down move a row,
 * Page Up and Page Down move the rows in view, and Home and End take the list's
 * ends, stepping over the letter headings. The scroll offset follows, so the
 * cursor row stays fully visible. */
export function moveEmbyListCursor(move: ListMove): void {
	const list = embyRows();
	const current = get(embyUi);
	const view = listWindow(list, current.scrollTop, current.viewportHeight);
	const cursor = moveEmbyCursor(list, current.cursor, move, view.end - view.start);
	if (cursor === current.cursor) {
		return;
	}
	embyUi.update((ui) => ({
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	}));
}

/** Jump to a letter of the rail (row 8.1, design D7): the cursor moves to the
 * first row under the letter and that row scrolls into view. A letter without
 * rows changes nothing. */
export function jumpEmbyToLetter(letter: string): void {
	const list = embyRows();
	const target = embyLetterRail(list).find((entry) => entry.letter === letter)?.rowId;
	if (target === null || target === undefined) {
		return;
	}
	embyUi.update((ui) => ({
		...ui,
		cursor: target,
		scrollTop: scrollTopToShowCursor(list, target, ui.scrollTop, ui.viewportHeight),
	}));
}

/** Report the list's measured viewport height (row 8.1): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measureEmbyList(viewportHeight: number): void {
	embyUi.update((ui) => ({
		...ui,
		viewportHeight,
		scrollTop: clampScrollTop(embyRows(), ui.scrollTop, viewportHeight),
	}));
}

/** Report the list's native scroll offset (row 8.1): a user scroll moves the
 * rows in view, and the offset stays inside the list's range. */
export function scrollEmbyList(scrollTop: number): void {
	embyUi.update((ui) => {
		const top = clampScrollTop(embyRows(), Math.max(scrollTop, 0), ui.viewportHeight);
		return top === ui.scrollTop ? ui : { ...ui, scrollTop: top };
	});
}

/** The pill the pill row's focus holds, resolved against the carried pills so a
 * stale group focuses nothing (row 8.1). */
export function focusedEmbyPill(emby: GuiEmbyFrame): GuiEmbyFeedGroup | null {
	const { focusedPill } = get(embyUi);
	if (focusedPill === null) {
		return null;
	}
	return emby.pills.find((pill) => embyGroupKey(pill.group) === focusedPill)?.group ?? null;
}

/** Move the focused pill one pill along the row (row 8.1): Right and Left walk
 * the pills, and the first of them with no pill focused takes the end it walks
 * from. */
export function moveEmbyPillFocus(step: number): void {
	const row = libraryEmby()?.pills ?? [];
	if (row.length === 0) {
		return;
	}
	const at = row.findIndex((pill) => embyGroupKey(pill.group) === get(embyUi).focusedPill);
	const next =
		at === -1
			? step > 0
				? 0
				: row.length - 1
			: Math.min(Math.max(at + step, 0), row.length - 1);
	const pill = row[next];
	if (pill !== undefined) {
		embyUi.update((ui) => ({ ...ui, focusedPill: embyGroupKey(pill.group) }));
	}
}

/** Ask for the focused pill to be selected: Space and Enter request it (row
 * 8.1). */
export function requestEmbyPillSelect(group: GuiEmbyFeedGroup): void {
	embyUi.update((ui) => ({ ...ui, pillSelect: group }));
}

/** Select the feed group `group` of the plain Emby library `libIdx` (row 8.1):
 * one group is always selected, and a press on a pill loads that group's
 * items. The core stores the choice, and the pills paint it back, so the row
 * and the list cannot disagree. */
export function selectEmbyPill(libIdx: number, group: GuiEmbyFeedGroup): void {
	void sendLibraryCommand({ command: 'embySetFeedGroup', libIdx, group });
}

/** Turn the keys' pill request into the set-feed-group command the pill sends
 * (row 8.1). The intent is consumed here, so one key sends one command. */
export function applyEmbyPillSelect(libIdx: number): void {
	const { pillSelect } = get(embyUi);
	if (pillSelect === null) {
		return;
	}
	embyUi.update((ui) => ({ ...ui, pillSelect: null }));
	selectEmbyPill(libIdx, pillSelect);
}
