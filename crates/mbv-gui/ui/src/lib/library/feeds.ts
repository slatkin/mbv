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

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import { clampScrollTop, listWindow, scrollTopToShowCursor } from './list';
import type { ListMove } from './list';
import type { GuiFeedPill, GuiFeedsFrame } from '$lib/bridge/types';
import { buildFeedsRows, moveFeedsCursor } from './feeds-list';
import type { FeedsItemRow } from './feeds-list';
import { libraryContent, libraryUi } from './state';

/** The carried Feeds content (design D12, row 11.1), or `null` while any
 * other destination is active. The Feeds destination is active while the
 * window holds this content, and every other destination's slice is then
 * `null`. */
export function libraryFeeds(): GuiFeedsFrame | null {
	return get(libraryContent)?.feeds ?? null;
}

/** The Feeds list's interaction state. */
export interface FeedsUiState {
	/** The cursor's row id, or `null` while no row carries the cursor
	 * (design D12). The cursor moves over the entry rows only. */
	cursor: string | null;
	/** The list's native scroll offset in pixels (design D12). */
	scrollTop: number;
	/** The list's measured viewport height in pixels: `0` until the list
	 * measures, and then the rows that paint are the rows in view. */
	viewportHeight: number;
	/** The pill the pill row's Left and Right move the focus to, or `null`
	 * while no pill has it (design D6, D12). */
	focusedPill: GuiFeedPill | null;
	/** The pill selection the keys asked for: Space and Enter select the
	 * focused pill, and the pill row turns it into the set-pill command. */
	pillSelect: GuiFeedPill | null;
	/** The `/` filter's text: the list paints only the entries whose title
	 * contains it, without case (design D7's filter rule). */
	filter: string;
}

/** The state a freshly mounted Feeds list starts with. */
export function initialFeedsUi(): FeedsUiState {
	return {
		cursor: null,
		scrollTop: 0,
		viewportHeight: 0,
		focusedPill: null,
		pillSelect: null,
		filter: '',
	};
}

/** The Feeds list's interaction state. */
export const feedsUi = writable<FeedsUiState>(initialFeedsUi());

/** The pill's stable key: the each-block's identity and the pill row's focus
 * key, over the pill object the frame carries (design D12). */
export function feedsPillKey(pill: GuiFeedPill): string {
	return pill.kind === 'latest' ? 'latest' : `subscription:${pill.id}`;
}

/** The painted rows of the carried Feeds content (design D12). */
function entryRows(filter = get(feedsUi).filter): FeedsItemRow[] {
	const feeds = libraryFeeds();
	return feeds === null ? [] : buildFeedsRows(feeds.rows, filter);
}

/** The row the Feeds cursor rests on, or `null` while no painted row carries
 * it: the keys read it to play the entry or queue it. */
export function feedsCursorRow(): FeedsItemRow | null {
	const cursor = get(feedsUi).cursor;
	const row = entryRows().find((candidate) => candidate.id === cursor);
	return row ?? null;
}

/** The state once the painted rows may have narrowed (design D12): the
 * cursor keeps its row while that row still paints, and falls to the first
 * painted row otherwise, with the scroll offset showing it — the Music
 * filter's own resolve (design D7). */
function resolveFeedsCursor(ui: FeedsUiState): FeedsUiState {
	const list = entryRows(ui.filter);
	const cursor = list.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveFeedsCursor(list, null, 'home');
	return {
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	};
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setFeedsFilter(filter: string): void {
	feedsUi.update((ui) => resolveFeedsCursor({ ...ui, filter }));
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closeFeedsFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
	feedsUi.update((ui) => resolveFeedsCursor({ ...ui, filter: '' }));
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptFeedsFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Select the row (design D12): the select command the core stores, whose
 * frame paints the Hero. A select repaints the Narrow arrangement's Hero
 * overlay and closes an open menu, because the facts they paint belong to the
 * previous entry, as a Music pick's do. */
export function selectFeedsRow(rowId: string): void {
	if (libraryFeeds() === null) {
		return;
	}
	void sendLibraryCommand({ command: 'feedsSelect', row: rowId });
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
	}));
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
	if (libraryFeeds() === null) {
		return;
	}
	void sendLibraryCommand({ command: 'feedsPlay', itemId });
}

/** Add an entry to the queue (design D12): `a` on the row's entry. There is
 * no play-next on the destination, so `n` sends no command. */
export function queueFeedsRow(itemId: string): void {
	if (libraryFeeds() === null) {
		return;
	}
	void sendLibraryCommand({ command: 'feedsQueue', itemId });
}

/** Move the list's cursor one movement along (design D12): Up and Down move a
 * row, Page Up and Page Down move the rows in view, and Home and End take the
 * list's ends. The scroll offset follows, so the cursor row stays fully
 * visible. */
export function moveFeedsListCursor(move: ListMove): void {
	const list = entryRows();
	const current = get(feedsUi);
	const view = listWindow(list, current.scrollTop, current.viewportHeight);
	const cursor = moveFeedsCursor(list, current.cursor, move, view.end - view.start);
	if (cursor === current.cursor) {
		return;
	}
	feedsUi.update((ui) => ({
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	}));
}

/** Report the list's measured viewport height (design D12): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measureFeedsList(viewportHeight: number): void {
	feedsUi.update((ui) => ({
		...ui,
		viewportHeight,
		scrollTop: clampScrollTop(entryRows(), ui.scrollTop, viewportHeight),
	}));
}

/** Report the list's native scroll offset (design D12): a user scroll moves
 * the rows in view, and the offset stays inside the list's range. */
export function scrollFeedsList(scrollTop: number): void {
	feedsUi.update((ui) => {
		const top = clampScrollTop(entryRows(), Math.max(scrollTop, 0), ui.viewportHeight);
		return top === ui.scrollTop ? ui : { ...ui, scrollTop: top };
	});
}

/** The pill the pill row's focus holds, resolved against the carried pills so
 * a stale pill focuses nothing: the pill the keys' Space and Enter select, and
 * the pill the row paints the accent outline on (design D6, D12). */
export function focusedFeedsPill(feeds: GuiFeedsFrame): GuiFeedPill | null {
	const { focusedPill } = get(feedsUi);
	if (focusedPill === null) {
		return null;
	}
	return feeds.pills.some((pill) => feedsPillKey(pill.pill) === feedsPillKey(focusedPill))
		? focusedPill
		: null;
}

/** Focus a pill, or no pill while `pill` is `null` (design D6, D12). */
function focusFeedsPill(pill: GuiFeedPill | null): void {
	feedsUi.update((ui) => ({ ...ui, focusedPill: pill }));
}

/** Move the focused pill one pill along the row (design D12): Right and Left
 * walk the pills, and the first of them with no pill focused takes the end it
 * walks from. */
export function moveFeedsPillFocus(step: number): void {
	const row = libraryFeeds()?.pills ?? [];
	if (row.length === 0) {
		return;
	}
	// The pill's place is its key, not its object: every carried frame is a
	// fresh deserialization, so an object-identity find would lose the focus
	// across frames and the walk would restart from a row end (row 10.1
	// review P2).
	const key = get(feedsUi).focusedPill;
	const focusedKey = key === null ? null : feedsPillKey(key);
	const at =
		focusedKey === null
			? -1
			: row.findIndex(
					(candidate: GuiFeedsFrame['pills'][number]) =>
						feedsPillKey(candidate.pill) === focusedKey,
			  );
	const next =
		at === -1
			? step > 0
				? 0
				: row.length - 1
			: Math.min(Math.max(at + step, 0), row.length - 1);
	const pill = row[next];
	if (pill !== undefined) {
		focusFeedsPill(pill.pill);
	}
}

/** Ask for the focused pill to be selected: Space and Enter request it
 * (design D6, D12). */
export function requestFeedsPillSelect(pill: GuiFeedPill): void {
	feedsUi.update((ui) => ({ ...ui, pillSelect: pill }));
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
	const { pillSelect } = get(feedsUi);
	if (pillSelect === null) {
		return;
	}
	feedsUi.update((ui) => ({ ...ui, pillSelect: null }));
	selectFeedsPill(pillSelect);
}
