// The plain Emby-library destination's interaction state (add-gui-library-panel
// design D12, row 8.1): what the window content owns and the core never reads —
// the cursor, the scroll offset, the measured viewport, and the feed-group pill
// row's focus. The core owns the selected row and the feed-group selection (the
// frame's `selected` and `pills`), so nothing here mirrors them: a press and an
// Enter send the command, and the frame carries the choice back. The row model
// is `$lib/library/emby-list`'s; the panel's pane focus and the Hero overlay
// stay in `./state`'s shared store, and the Hero carries no Workspace and no
// cursor of its own.
//
// The state, the filter, the cursor, the scroll, and the pill focus are the
// pill-list controller's (`./pill-list`, row 7C.1); this module keeps the Emby
// frame, the Emby rows and their letter headings, the letter rail, the play-next
// command, the Emby command shapes, and the Emby names.

import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiEmbyFeedGroup, GuiEmbyFrame } from '$lib/bridge/types';
import { scrollTopToShowCursor } from './list';
import type { ListMove } from './list';
import {
	buildEmbyRows,
	embyGroupKey,
	embyLetterRail,
	moveEmbyCursor,
} from './emby-list';
import type { EmbyItemRow, EmbyListRow } from './emby-list';
import { createPillList, initialPillListUi, sendIfLoaded } from './pill-list';
import type { PillListUiState } from './pill-list';
import { libraryEmby } from './state';

/** The Emby list's interaction state: the shared pill-list state over the
 * feed-group pill, whose focus the pill row keeps as the group's stable key
 * (`embyGroupKey`) rather than the group itself (design D6, row 8.1). */
export type EmbyUiState = PillListUiState<GuiEmbyFeedGroup, string>;

/** The state a freshly mounted Emby list starts with. */
export function initialEmbyUi(): EmbyUiState {
	return initialPillListUi<GuiEmbyFeedGroup, string>();
}

/** The Emby pill list: the rows the carried frame builds under the filter —
 * the plain mode's rows under their letter headings, or the feed-group mode's
 * flat rows — the heading-skipping cursor movement, and the pill focus stored
 * as the group's key (row 7C.1). */
const pillList = createPillList<
	GuiEmbyFrame,
	EmbyListRow,
	EmbyItemRow,
	GuiEmbyFeedGroup,
	string,
	GuiEmbyFrame['pills'][number]
>({
	frame: libraryEmby,
	buildRows: (emby, filter) => buildEmbyRows(emby.rows, emby.feedGroup, filter),
	itemOf: (row) => (row.kind === 'item' ? row : null),
	move: moveEmbyCursor,
	pills: (emby) => emby.pills,
	pillOf: (entry) => entry.group,
	pillKey: embyGroupKey,
	focusOf: embyGroupKey,
	focusKey: (key) => key,
});

/** The Emby list's interaction state. */
export const embyUi = pillList.ui;

/** The painted rows of the carried Emby content (row 8.1): the plain mode's
 * rows under their letter headings, or the feed-group mode's flat rows. */
function embyRows(): EmbyListRow[] {
	return pillList.rows();
}

/** The row the Emby cursor rests on, or `null` (row 8.1): the keys read it to
 * play the row, play it next, or queue it. */
export function embyCursorRow(): EmbyItemRow | null {
	return pillList.cursorRow();
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setEmbyFilter(filter: string): void {
	pillList.setFilter(filter);
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closeEmbyFilter(): void {
	pillList.closeFilter();
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptEmbyFilter(): void {
	pillList.acceptFilter();
}

/** Select the row (row 8.1): the select command the core stores, whose frame
 * paints the Hero. A select repaints the Narrow arrangement's Hero overlay and
 * closes an open menu, because the facts they paint belong to the previous row,
 * as a Music pick's do. */
export function selectEmbyRow(row: EmbyListRow): void {
	if (row.kind === 'heading') {
		return;
	}
	pillList.select((emby) => ({ command: 'embySelect', libIdx: emby.libIdx, row: row.row.id }));
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
	sendIfLoaded(libraryEmby(), (emby) => ({
		command: 'embyPlay',
		libIdx: emby.libIdx,
		itemId: rowId,
	}));
}

/** Play a row next (row 8.1): `n` on the cursor row. */
export function playEmbyRowNext(rowId: string): void {
	sendIfLoaded(libraryEmby(), (emby) => ({
		command: 'embyPlayNext',
		libIdx: emby.libIdx,
		itemId: rowId,
	}));
}

/** Add a row to the queue's tail (row 8.1): `a` on the cursor row. */
export function queueEmbyRow(rowId: string): void {
	sendIfLoaded(libraryEmby(), (emby) => ({
		command: 'embyQueue',
		libIdx: emby.libIdx,
		itemId: rowId,
	}));
}

/** Move the list's cursor one movement along (row 8.1): Up and Down move a row,
 * Page Up and Page Down move the rows in view, and Home and End take the list's
 * ends, stepping over the letter headings. The scroll offset follows, so the
 * cursor row stays fully visible. */
export function moveEmbyListCursor(move: ListMove): void {
	pillList.moveCursor(move);
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
	pillList.measure(viewportHeight);
}

/** Report the list's native scroll offset (row 8.1): a user scroll moves the
 * rows in view, and the offset stays inside the list's range. */
export function scrollEmbyList(scrollTop: number): void {
	pillList.scroll(scrollTop);
}

/** The pill the pill row's focus holds, resolved against the carried pills so a
 * stale group focuses nothing (row 8.1). */
export function focusedEmbyPill(emby: GuiEmbyFrame): GuiEmbyFeedGroup | null {
	return pillList.focusedPill(emby);
}

/** Move the focused pill one pill along the row (row 8.1): Right and Left walk
 * the pills, and the first of them with no pill focused takes the end it walks
 * from. */
export function moveEmbyPillFocus(step: number): void {
	pillList.movePillFocus(step);
}

/** Ask for the focused pill to be selected: Space and Enter request it (row
 * 8.1). */
export function requestEmbyPillSelect(group: GuiEmbyFeedGroup): void {
	pillList.requestPillSelect(group);
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
	const group = pillList.takePillSelect();
	if (group !== null) {
		selectEmbyPill(libIdx, group);
	}
}
