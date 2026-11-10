// The Podcasts destination's interaction state (add-gui-library-panel design
// D12, row 10.1): what the window content owns and the core never reads — the
// cursor, the scroll offset, the measured viewport, and the pill row's focus.
// The core owns the pill selection and the selected episode (the frame's
// `pills` and `selected`), so nothing here mirrors them: a press sends the
// select command, and the frame carries the choice back. The row model is
// `$lib/library/podcasts-list`'s; the panel's pane focus and the Hero overlay
// stay in `./state`'s shared store. There is no play-next: `n` is consumed
// and does nothing.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import { clampScrollTop, listWindow, moveCursor, scrollTopToShowCursor } from './list';
import type { ListMove } from './list';
import type { GuiPodcastPill, GuiPodcastsFrame } from '$lib/bridge/types';
import { buildPodcastsRows } from './podcasts-list';
import type { PodcastsItemRow } from './podcasts-list';
import { libraryPodcasts, libraryUi } from './state';

/** The Podcasts list's interaction state. */
export interface PodcastsUiState {
	/** The cursor's row id, or `null` while no row carries the cursor
	 * (design D12). The cursor moves over the episode rows only. */
	cursor: string | null;
	/** The list's native scroll offset in pixels (design D12). */
	scrollTop: number;
	/** The list's measured viewport height in pixels: `0` until the list
	 * measures, and then the rows that paint are the rows in view. */
	viewportHeight: number;
	/** The pill the pill row's Left and Right move the focus to, or `null`
	 * while no pill has it (design D6, D12). */
	focusedPill: GuiPodcastPill | null;
	/** The pill selection the keys asked for: Space and Enter select the
	 * focused pill, and the pill row turns it into the set-pill command. */
	pillSelect: GuiPodcastPill | null;
	/** The `/` filter's text: the list paints only the episodes whose title
	 * contains it, without case (design D7's filter rule). */
	filter: string;
}

/** The state a freshly mounted Podcasts list starts with. */
export function initialPodcastsUi(): PodcastsUiState {
	return {
		cursor: null,
		scrollTop: 0,
		viewportHeight: 0,
		focusedPill: null,
		pillSelect: null,
		filter: '',
	};
}

/** The Podcasts list's interaction state. */
export const podcastsUi = writable<PodcastsUiState>(initialPodcastsUi());

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

/** The painted rows of the carried Podcasts content (design D12). */
function episodeRows(filter = get(podcastsUi).filter): PodcastsItemRow[] {
	const podcasts = libraryPodcasts();
	return podcasts === null ? [] : buildPodcastsRows(podcasts.rows, filter);
}

/** The row the Podcasts cursor rests on, or `null` while no painted row
 * carries it: the keys read it to play the episode or queue it. */
export function podcastsCursorRow(): PodcastsItemRow | null {
	const cursor = get(podcastsUi).cursor;
	const row = episodeRows().find((candidate) => candidate.id === cursor);
	return row ?? null;
}

/** The state once the painted rows may have narrowed (design D12): the
 * cursor keeps its row while that row still paints, and falls to the first
 * painted row otherwise, with the scroll offset showing it — the Music
 * filter's own resolve (design D7). */
function resolvePodcastsCursor(ui: PodcastsUiState): PodcastsUiState {
	const list = episodeRows(ui.filter);
	const cursor = list.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveCursor(list, null, 'home');
	return {
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	};
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setPodcastsFilter(filter: string): void {
	podcastsUi.update((ui) => resolvePodcastsCursor({ ...ui, filter }));
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closePodcastsFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
	podcastsUi.update((ui) => resolvePodcastsCursor({ ...ui, filter: '' }));
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptPodcastsFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Select the row (design D12): the select command the core stores, whose
 * frame paints the Hero. A select repaints the Narrow arrangement's Hero
 * overlay and closes an open menu, because the facts they paint belong to
 * the previous episode, as a Music pick's do. */
export function selectPodcastsRow(rowId: string): void {
	const podcasts = libraryPodcasts();
	if (podcasts === null) {
		return;
	}
	void sendLibraryCommand({ command: 'podcastsSelect', libIdx: podcasts.libIdx, row: rowId });
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
	}));
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
	const podcasts = libraryPodcasts();
	if (podcasts === null) {
		return;
	}
	void sendLibraryCommand({ command: 'podcastsPlay', libIdx: podcasts.libIdx, itemId });
}

/** Add an episode to the queue (design D12): `a` on the row's episode. There
 * is no play-next on the destination, so `n` sends no command. */
export function queuePodcastsRow(itemId: string): void {
	const podcasts = libraryPodcasts();
	if (podcasts === null) {
		return;
	}
	void sendLibraryCommand({ command: 'podcastsQueue', libIdx: podcasts.libIdx, itemId });
}

/** Move the list's cursor one movement along (design D12): Up and Down move a
 * row, Page Up and Page Down move the rows in view, and Home and End take the
 * list's ends. The scroll offset follows, so the cursor row stays fully
 * visible. */
export function movePodcastsListCursor(move: ListMove): void {
	const list = episodeRows();
	const current = get(podcastsUi);
	const view = listWindow(list, current.scrollTop, current.viewportHeight);
	const cursor = moveCursor(list, current.cursor, move, view.end - view.start);
	if (cursor === current.cursor) {
		return;
	}
	podcastsUi.update((ui) => ({
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(list, cursor, ui.scrollTop, ui.viewportHeight),
	}));
}

/** Report the list's measured viewport height (design D12): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measurePodcastsList(viewportHeight: number): void {
	podcastsUi.update((ui) => ({
		...ui,
		viewportHeight,
		scrollTop: clampScrollTop(episodeRows(), ui.scrollTop, viewportHeight),
	}));
}

/** Report the list's native scroll offset (design D12): a user scroll moves
 * the rows in view, and the offset stays inside the list's range. */
export function scrollPodcastsList(scrollTop: number): void {
	podcastsUi.update((ui) => {
		const top = clampScrollTop(episodeRows(), Math.max(scrollTop, 0), ui.viewportHeight);
		return top === ui.scrollTop ? ui : { ...ui, scrollTop: top };
	});
}

/** The pill the pill row's focus holds, resolved against the carried pills so
 * a stale pill focuses nothing: the pill the keys' Space and Enter select, and
 * the pill the row paints the accent outline on (design D6, D12). */
export function focusedPodcastsPill(podcasts: GuiPodcastsFrame): GuiPodcastPill | null {
	const { focusedPill } = get(podcastsUi);
	if (focusedPill === null) {
		return null;
	}
	return podcasts.pills.some((pill) => podcastPillKey(pill.pill) === podcastPillKey(focusedPill))
		? focusedPill
		: null;
}

/** Focus a pill, or no pill while `pill` is `null` (design D6, D12). */
function focusPodcastsPill(pill: GuiPodcastPill | null): void {
	podcastsUi.update((ui) => ({ ...ui, focusedPill: pill }));
}

/** Move the focused pill one pill along the row (design D12): Right and Left
 * walk the pills, and the first of them with no pill focused takes the end it
 * walks from. */
export function movePodcastsPillFocus(step: number): void {
	const row = libraryPodcasts()?.pills ?? [];
	if (row.length === 0) {
		return;
	}
	// The pill's place is its key, not its object: every carried frame is a
	// fresh deserialization, so an object-identity find would lose the focus
	// across frames and the walk would restart from a row end (row 10.1
	// review P2).
	// The pill's place is its key, not its object: every carried frame is a
	// fresh deserialization, so an object-identity find would lose the focus
	// across frames and the walk would restart from a row end (row 10.1
	// review P2).
	const key = get(podcastsUi).focusedPill;
	const focusedKey = key === null ? null : podcastPillKey(key);
	const at =
		focusedKey === null
			? -1
			: row.findIndex(
					(candidate: GuiPodcastsFrame['pills'][number]) =>
						podcastPillKey(candidate.pill) === focusedKey,
			  );
	const next =
		at === -1
			? step > 0
				? 0
				: row.length - 1
			: Math.min(Math.max(at + step, 0), row.length - 1);
	const pill = row[next];
	if (pill !== undefined) {
		focusPodcastsPill(pill.pill);
	}
}

/** Ask for the focused pill to be selected: Space and Enter request it
 * (design D6, D12). */
export function requestPodcastsPillSelect(pill: GuiPodcastPill): void {
	podcastsUi.update((ui) => ({ ...ui, pillSelect: pill }));
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
	const { pillSelect } = get(podcastsUi);
	if (pillSelect === null) {
		return;
	}
	podcastsUi.update((ui) => ({ ...ui, pillSelect: null }));
	selectPodcastsPill(libIdx, pillSelect);
}
