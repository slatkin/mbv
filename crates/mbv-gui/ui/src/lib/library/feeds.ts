// The Feeds destination's interaction state (add-gui-library-panel design
// D12, row 11.1): what the window content owns and the core never reads — the
// cursor, the scroll offset, the measured viewport, and the pill row's focus.
// The core owns the pill selection and the selected entry (the frame's
// `pills` and `selected`), so nothing here mirrors them: a press sends the
// select command, and the frame carries the choice back. The row model is
// `$lib/library/feeds-list`'s; the panel's pane focus and the Hero overlay
// stay in `./state`'s shared store. There is no play-next: `n` is consumed
// and does nothing. A Client holds one feed tab,
// so no library index rides the commands.
//
// The state, the filter, the cursor, the scroll, and the pill focus are the
// pill-list controller's (`./pill-list`, row 7C.1); this module keeps the
// Feeds frame, the Feeds rows, the Feeds command shapes, and the Feeds names.

import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiFeedPill, GuiFeedsList } from '$lib/bridge/types';
import { moveCursor } from './list';
import type { ListMove } from './list';
import { buildFeedsRows } from './feeds-list';
import type { FeedsItemRow } from './feeds-list';
import { createPillList, initialPillListUi, sendIfLoaded } from './pill-list';
import type { PillListUiState } from './pill-list';
import { feedsScroll } from './scroll';
import { libraryFeedsList } from './parts';

/** The Feeds list's interaction state: the shared pill-list state over the
 * Feeds pill (design D12). */
export type FeedsUiState = PillListUiState<GuiFeedPill>;

/** The state a freshly mounted Feeds list starts with. */
export function initialFeedsUi(): FeedsUiState {
	return initialPillListUi();
}

/** The pill's stable key: the each-block's identity and the pill row's focus
 * key, over the pill object the frame carries (design D12). */
export function feedsPillKey(pill: GuiFeedPill): string {
	return pill.kind === 'latest' ? 'latest' : `subscription:${pill.id}`;
}

/** The Feeds pill list: the entry rows the carried frame builds under the
 * filter, the plain cursor movement, and the pill focus stored as the pill
 * itself (row 7C.1). */
const pillList = createPillList<
	GuiFeedsList,
	FeedsItemRow,
	FeedsItemRow,
	GuiFeedPill,
	GuiFeedPill,
	GuiFeedsList['pills'][number]
>({
	frame: libraryFeedsList,
	scroll: feedsScroll,
	buildRows: (feeds, filter) => buildFeedsRows(feeds.rows, filter),
	itemOf: (row) => row,
	move: moveCursor,
	pills: (feeds) => feeds.pills,
	pillOf: (entry) => entry.pill,
	pillKey: feedsPillKey,
	focusOf: (pill) => pill,
	focusKey: feedsPillKey,
});

/** The Feeds list's interaction state. */
export const feedsUi = pillList.ui;

/** The row the Feeds cursor rests on, or `null` while no painted row carries
 * it: the keys read it to play the entry or queue it. */
export function feedsCursorRow(): FeedsItemRow | null {
	return pillList.cursorRow();
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setFeedsFilter(filter: string): void {
	pillList.setFilter(filter);
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closeFeedsFilter(): void {
	pillList.closeFilter();
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptFeedsFilter(): void {
	pillList.acceptFilter();
}

/** Select the row (design D12): the select command the core stores, whose
 * frame paints the Hero. A select repaints the Narrow arrangement's Hero
 * overlay and closes an open menu, because the facts they paint belong to the
 * previous entry, as a Music pick's do. */
export function selectFeedsRow(rowId: string): void {
	pillList.select(() => ({ command: 'feedsSelect', row: rowId }));
}

/** Press an entry row (design D12): the press selects the entry the Hero
 * shows. */
export function pressFeedsRow(row: FeedsItemRow): void {
	selectFeedsRow(row.id);
}

/** Play an entry (design D12): Enter on the cursor row, and a double-click's
 * play. The queue is replaced, as the TUI's feeds play replaces it. The row
 * id is the entry's stable identity the frame carried. */
export function playFeedsRow(itemId: string): void {
	sendIfLoaded(libraryFeedsList(), () => ({ command: 'feedsPlay', itemId }));
}

/** Add an entry to the queue (design D12): `a` on the row's entry. There is
 * no play-next on the destination, so `n` sends no command. */
export function queueFeedsRow(itemId: string): void {
	sendIfLoaded(libraryFeedsList(), () => ({ command: 'feedsQueue', itemId }));
}

/** Move the list's cursor one movement along (design D12): Up and Down move a
 * row, Page Up and Page Down move the rows in view, and Home and End take the
 * list's ends. The scroll offset follows, so the cursor row stays fully
 * visible. */
export function moveFeedsListCursor(move: ListMove): void {
	pillList.moveCursor(move);
}

/** Report the list's measured viewport height (design D12): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measureFeedsList(viewportHeight: number): void {
	pillList.measure(viewportHeight);
}

/** Report the list's native scroll offset (design D12): a user scroll moves
 * the rows in view, and the offset stays inside the list's range. */
export function scrollFeedsList(scrollTop: number): void {
	pillList.scroll(scrollTop);
}

/** The pill the pill row's focus holds, resolved against the carried pills so
 * a stale pill focuses nothing: the pill the keys' Space and Enter select, and
 * the pill the row paints the accent outline on (design D6, D12). */
export function focusedFeedsPill(feeds: GuiFeedsList): GuiFeedPill | null {
	return pillList.focusedPill(feeds);
}

/** Move the focused pill one pill along the row (design D12): Right and Left
 * walk the pills, and the first of them with no pill focused takes the end it
 * walks from. */
export function moveFeedsPillFocus(step: number): void {
	pillList.movePillFocus(step);
}

/** Ask for the focused pill to be selected: Space and Enter request it
 * (design D6, D12). */
export function requestFeedsPillSelect(pill: GuiFeedPill): void {
	pillList.requestPillSelect(pill);
}

/** Set the Feeds destination's pill `pill` (design D12, row 11.1): one pill
 * is always selected, and Latest is the default (design D6). The core stores
 * the choice and loads the pill's entries, and the pills paint the core's
 * choice back, so the row and the list cannot disagree. */
export function selectFeedsPill(pill: GuiFeedPill): void {
	void sendLibraryCommand({ command: 'feedsSetPill', pill });
}

/** Turn the keys' pill request into the set-pill command the pill sends
 * (design D12). The intent is consumed here, so one key sends one command. */
export function applyFeedsPillSelect(): void {
	const pill = pillList.takePillSelect();
	if (pill !== null) {
		selectFeedsPill(pill);
	}
}
