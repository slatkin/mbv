// The Podcasts destination's interaction state (add-gui-library-panel design
// D12, row 10.1): what the window content owns and the core never reads — the
// cursor, the scroll offset, the measured viewport, and the pill row's focus.
// The core owns the pill selection and the selected episode (the frame's
// `pills` and `selected`), so nothing here mirrors them: a press sends the
// select command, and the frame carries the choice back. The row model is
// `$lib/library/podcasts-list`'s; the panel's pane focus and the Hero overlay
// stay in `./state`'s shared store. There is no play-next: `n` is consumed
// and does nothing.
//
// The state, the filter, the cursor, the scroll, and the pill focus are the
// pill-list controller's (`./pill-list`, row 7C.1); this module keeps the
// Podcasts frame, the Podcasts rows, the Podcasts command shapes, and the
// Podcasts names.

import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiPodcastPill, GuiPodcastsList } from '$lib/bridge/types';
import { moveCursor } from './list';
import type { ListMove } from './list';
import { buildPodcastsRows } from './podcasts-list';
import type { PodcastsItemRow } from './podcasts-list';
import { createPillList, initialPillListUi, sendIfLoaded } from './pill-list';
import type { PillListUiState } from './pill-list';
import { podcastsScroll } from './scroll';
import { libraryPodcastsList } from './parts';

/** The Podcasts list's interaction state: the shared pill-list state over
 * the Podcasts pill (design D12). */
export type PodcastsUiState = PillListUiState<GuiPodcastPill>;

/** The state a freshly mounted Podcasts list starts with. */
export function initialPodcastsUi(): PodcastsUiState {
	return initialPillListUi();
}

/** The pill's stable key: the each-block's identity and the pill row's focus
 * key, over the pill object the frame carries (design D12). */
export function podcastPillKey(pill: GuiPodcastPill): string {
	switch (pill.kind) {
		case 'latest':
			return 'latest';
		case 'state':
			return `state:${pill.id}`;
		case 'show':
			return `show:${pill.id}`;
	}
}

/** The Podcasts pill list: the episode rows the carried frame builds under
 * the filter, the plain cursor movement, and the pill focus stored as the
 * pill itself (row 7C.1). */
const pillList = createPillList<
	GuiPodcastsList,
	PodcastsItemRow,
	PodcastsItemRow,
	GuiPodcastPill,
	GuiPodcastPill,
	GuiPodcastsList['pills'][number]
>({
	frame: libraryPodcastsList,
	scroll: podcastsScroll,
	buildRows: (podcasts, filter) => buildPodcastsRows(podcasts.rows, filter),
	itemOf: (row) => row,
	move: moveCursor,
	pills: (podcasts) => podcasts.pills,
	pillOf: (entry) => entry.pill,
	pillKey: podcastPillKey,
	focusOf: (pill) => pill,
	focusKey: podcastPillKey,
});

/** The Podcasts list's interaction state. */
export const podcastsUi = pillList.ui;

/** The row the Podcasts cursor rests on, or `null` while no painted row
 * carries it: the keys read it to play the episode or queue it. */
export function podcastsCursorRow(): PodcastsItemRow | null {
	return pillList.cursorRow();
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setPodcastsFilter(filter: string): void {
	pillList.setFilter(filter);
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closePodcastsFilter(): void {
	pillList.closeFilter();
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptPodcastsFilter(): void {
	pillList.acceptFilter();
}

/** Select the row (design D12): the select command the core stores, whose
 * frame paints the Hero. A select repaints the Narrow arrangement's Hero
 * overlay and closes an open menu, because the facts they paint belong to
 * the previous episode, as a Music pick's do. */
export function selectPodcastsRow(rowId: string): void {
	pillList.select((podcasts) => ({ command: 'podcastsSelect', libIdx: podcasts.libIdx, row: rowId }));
}

/** Press an episode row (design D12): the press selects the episode the Hero
 * shows. */
export function pressPodcastsRow(row: PodcastsItemRow): void {
	selectPodcastsRow(row.id);
}

/** Play an episode (design D12): Enter on the cursor row, and a double-click's
 * play. The queue is replaced under the Audiobookshelf admission rule, through
 * the shared play path. The row id is the episode's stable identity the frame
 * carried. */
export function playPodcastsRow(itemId: string): void {
	sendIfLoaded(libraryPodcastsList(), (podcasts) => ({
		command: 'podcastsPlay',
		libIdx: podcasts.libIdx,
		itemId,
	}));
}

/** Add an episode to the queue (design D12): `a` on the row's episode. There
 * is no play-next on the destination, so `n` sends no command. */
export function queuePodcastsRow(itemId: string): void {
	sendIfLoaded(libraryPodcastsList(), (podcasts) => ({
		command: 'podcastsQueue',
		libIdx: podcasts.libIdx,
		itemId,
	}));
}

/** Move the list's cursor one movement along (design D12): Up and Down move a
 * row, Page Up and Page Down move the rows in view, and Home and End take the
 * list's ends. The scroll offset follows, so the cursor row stays fully
 * visible. */
export function movePodcastsListCursor(move: ListMove): void {
	pillList.moveCursor(move);
}

/** Report the list's measured viewport height (design D12): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measurePodcastsList(viewportHeight: number): void {
	pillList.measure(viewportHeight);
}

/** Report the list's native scroll offset (design D12): a user scroll moves
 * the rows in view, and the offset stays inside the list's range. */
export function scrollPodcastsList(scrollTop: number): void {
	pillList.scroll(scrollTop);
}

/** The pill the pill row's focus holds, resolved against the carried pills so
 * a stale pill focuses nothing: the pill the keys' Space and Enter select, and
 * the pill the row paints the accent outline on (design D6, D12). */
export function focusedPodcastsPill(podcasts: GuiPodcastsList): GuiPodcastPill | null {
	return pillList.focusedPill(podcasts);
}

/** Move the focused pill one pill along the row (design D12): Right and Left
 * walk the pills, and the first of them with no pill focused takes the end it
 * walks from. */
export function movePodcastsPillFocus(step: number): void {
	pillList.movePillFocus(step);
}

/** Ask for the focused pill to be selected: Space and Enter request it
 * (design D6, D12). */
export function requestPodcastsPillSelect(pill: GuiPodcastPill): void {
	pillList.requestPillSelect(pill);
}

/** Select the pill `pill` of the Podcasts library `libIdx` (design D12, row
 * 10.1): one pill is always selected, and Latest is the default (design D6).
 * The core stores the choice and loads the pill's episodes, and the pills
 * paint the core's choice back, so the row and the list cannot disagree. */
export function selectPodcastsPill(libIdx: number, pill: GuiPodcastPill): void {
	void sendLibraryCommand({ command: 'podcastsSetPill', libIdx, pill });
}

/** Turn the keys' pill request into the set-pill command the pill sends
 * (design D12). The intent is consumed here, so one key sends one command. */
export function applyPodcastsPillSelect(libIdx: number): void {
	const pill = pillList.takePillSelect();
	if (pill !== null) {
		selectPodcastsPill(libIdx, pill);
	}
}
