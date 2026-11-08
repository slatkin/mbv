// The Library panel's own state (add-gui-library-panel design D1): what the
// window content owns and the core never reads. Two stores keep the two kinds
// apart — the content the frame carries, kept from the last frame that carried
// it, and the interaction state the panel's keys and controls move.
//
// The core owns the data selection (which collections are selected, the picked
// item), so nothing here mirrors it: the pill toggle the keys request is an
// intent the pill row turns into the set-collections command (design D6).

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type {
	GuiLibraryContent,
	GuiLibraryFrame,
	GuiMusicFrame,
	GuiMusicPill,
} from '$lib/bridge/types';
import {
	buildLibraryRows,
	clampScrollTop,
	letterRail,
	listWindow,
	moveListCursor,
	scrollTopToShowCursor,
} from './list';
import type { LibraryNodeRow, LibraryRow, LibraryTree, ListMove } from './list';
import { nextPillSelection } from './pills';
import type { PillGesture } from './pills';

/** The panes Tab moves focus across (design D3, step 3). The Queue panel is
 * the pane past the Hero, but it takes Panel focus through the core's focus
 * stack, which no key command reaches (design D4). */
export type LibraryPane = 'pills' | 'list' | 'hero';

/** The order Tab and Shift+Tab walk the panes in (design D3, step 3). */
const PANES: readonly LibraryPane[] = ['pills', 'list', 'hero'];

/** The Library panel's interaction state. */
export interface LibraryUiState {
	/** The pane the window content focuses. */
	pane: LibraryPane;
	/** Whether a text field of the panel holds the keyboard (design D3,
	 * step 2). */
	filterOpen: boolean;
	/** The filter's text: the `/` field types it, and it narrows the list
	 * without case (design D7). */
	filter: string;
	/** The pill Left and Right move the focus to, or `null` while no pill has
	 * it (design D6). */
	focusedPill: string | null;
	/** The pill toggle the keys asked for: Space toggles and Alt+Space solos
	 * the focused pill (design D6). The pill row turns it into the
	 * set-collections command. */
	pillToggle: { groupId: string; solo: boolean } | null;
	/** The list's cursor row id, or `null` while no row carries the cursor
	 * (design D7). The cursor and the picked item are different: the cursor
	 * moves by keys, and the picked album is the core's. */
	cursor: string | null;
	/** The list's open tree node row ids (design D7): an artist and an album
	 * each open and close on their own. */
	openNodes: string[];
	/** The node row ids the user closed while the filter narrowed the rows:
	 * the filter opens a node that holds a match, so the close half needs an
	 * override to stay shut (design D7's open and close). A new filter and a
	 * cleared one drop it. */
	filterClosed: string[];
	/** The list's native scroll offset in pixels (design D7). */
	scrollTop: number;
	/** The list's measured viewport height in pixels: `0` until the list
	 * measures, and then the rows that paint are the rows in view. */
	viewportHeight: number;
}

/** The state a freshly mounted window content starts with. */
export function initialLibraryUi(): LibraryUiState {
	return {
		pane: 'pills',
		filterOpen: false,
		filter: '',
		focusedPill: null,
		pillToggle: null,
		cursor: null,
		openNodes: [],
		filterClosed: [],
		scrollTop: 0,
		viewportHeight: 0,
	};
}

/** The Library panel's interaction state. */
export const libraryUi = writable<LibraryUiState>(initialLibraryUi());

/** The last Library content the core carried (design D2). A frame re-carries
 * it only when its revision changes, so the window keeps what the last
 * carrying frame held. */
export const libraryContent = writable<GuiLibraryContent | null>(null);

/** Keep the content of a frame that carries it (design D2). */
export function carryLibraryContent(frame: GuiLibraryFrame | null): void {
	if (frame?.content) {
		libraryContent.set(frame.content);
	}
}

/** Open the list's filter: `/` with Library Panel focus (design D3, step 4). */
export function openLibraryFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: true }));
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor and the scroll follow the
 * widened rows, and the filter's own open and close over the nodes goes with
 * the text. */
export function closeLibraryFilter(): void {
	libraryUi.update((ui) =>
		resolveCursor({ ...ui, filterOpen: false, filter: '', filterClosed: [] }),
	);
}

/** Accept the filter with Enter: the field closes and the list keeps the text
 * (design D3, step 2). The cursor and the scroll follow the rows the same way
 * a text change moves them. */
export function acceptLibraryFilter(): void {
	libraryUi.update((ui) => resolveCursor({ ...ui, filterOpen: false }));
}

/** Focus a pane: the window content of Tab and Shift+Tab (design D3, step 3). */
export function focusLibraryPane(pane: LibraryPane): void {
	libraryUi.update((ui) => ({ ...ui, pane }));
}

/** Move the pane focus `step` panes along, wrapping at the ends. */
export function moveLibraryPane(step: number): void {
	libraryUi.update((ui) => {
		const at = PANES.indexOf(ui.pane);
		const next = (at + step + PANES.length) % PANES.length;
		return { ...ui, pane: PANES[next] };
	});
}

/** Focus a pill, or no pill while `groupId` is `null`. */
export function focusLibraryPill(groupId: string | null): void {
	libraryUi.update((ui) => ({ ...ui, focusedPill: groupId }));
}

/** Ask for the focused pill to toggle, or to solo with Alt+Space (design D6). */
export function requestLibraryPillToggle(groupId: string, solo: boolean): void {
	libraryUi.update((ui) => ({ ...ui, pillToggle: { groupId, solo } }));
}

/** Send the set a pill gesture leaves selected (design D6): the core stores it
 * per destination and loads the new groups. The set reads the core's carried
 * selection, so the pills and the command cannot disagree. */
export function selectLibraryPills(
	libIdx: number,
	pills: readonly GuiMusicPill[],
	gesture: PillGesture,
): void {
	void sendLibraryCommand({
		command: 'setCollections',
		libIdx,
		groupIds: nextPillSelection(pills, gesture),
	});
}

/** Turn the keys' pill toggle into the set-collections command a press on the
 * pill sends (design D6): Space toggles the focused pill and Alt+Space solos
 * it. The intent is consumed here, so one key sends one command. */
export function applyLibraryPillToggle(
	libIdx: number,
	pills: readonly GuiMusicPill[],
): void {
	const { pillToggle } = get(libraryUi);
	if (pillToggle === null) {
		return;
	}
	libraryUi.update((ui) => ({ ...ui, pillToggle: null }));
	selectLibraryPills(libIdx, pills, {
		kind: 'press',
		groupId: pillToggle.groupId,
		solo: pillToggle.solo,
	});
}

/** The carried Music content, or `null` while the active destination carries
 * none. */
export function libraryMusic(): GuiMusicFrame | null {
	return get(libraryContent)?.music ?? null;
}

/** The row model under one interaction state (design D7): the carried union,
 * the open nodes, and the filter text. */
function rowsOf(ui: LibraryUiState): LibraryRow[] {
	return buildLibraryRows(libraryTree(ui));
}

function libraryTree(ui: LibraryUiState): LibraryTree {
	return {
		artists: libraryMusic()?.artists ?? [],
		open: ui.openNodes,
		closed: ui.filterClosed,
		filter: ui.filter,
	};
}

/** The interaction state once the painted rows may have changed: the cursor
 * keeps its row while that row still paints, and falls to the first painted row
 * otherwise, with the scroll offset showing it (design D7). A filter's text
 * change and its clear both run this, so the cursor never rests on a row that
 * no longer paints. */
function resolveCursor(ui: LibraryUiState): LibraryUiState {
	const rows = rowsOf(ui);
	const cursor = rows.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveListCursor(rows, null, 'home');
	return {
		...ui,
		cursor,
		scrollTop: scrollTopToShowCursor(rows, cursor, ui.scrollTop, ui.viewportHeight),
	};
}

/** The cursor's row, or `null` while no painted row carries the cursor. */
function cursorRow(ui: LibraryUiState): LibraryRow | null {
	return rowsOf(ui).find((row) => row.id === ui.cursor) ?? null;
}

/** Send the detail of the item a row picks (design D7): the core stores it and
 * the next frame carries the Hero. No command for a row that names no item. */
function showDetail(itemId: string | null): void {
	if (itemId !== null) {
		void sendLibraryCommand({ command: 'showDetail', itemId });
	}
}

/** The item a row picks (design D7, D5): an album's own id, and a track's album
 * — a picked track picks its album. An artist row picks nothing: the Hero is
 * album-scoped (design D5), so an artist's detail would empty it. */
function pickedItemId(row: LibraryRow): string | null {
	switch (row.kind) {
		case 'letter':
			return null;
		case 'artist':
			return null;
		case 'album':
			return row.albumId;
		case 'track':
			return row.albumId;
	}
}

/** Open or close one tree node (design D7): opening an album whose tracklist
 * the carried content does not hold asks the core to load it (design D5). While
 * the filter narrows the rows, the press overrides the filter's auto-open for
 * that node instead of the stored open state, so the node the user closed stays
 * closed until the filter changes. */
function setNodeOpen(row: LibraryNodeRow, open: boolean): void {
	libraryUi.update((ui) =>
		ui.filter.trim() === ''
			? {
					...ui,
					openNodes: withId(ui.openNodes, row.id, open),
				}
			: {
					...ui,
					// The closed set names the nodes to keep shut, so it takes the id
					// when the press closes the node.
					filterClosed: withId(ui.filterClosed, row.id, !open),
				},
	);
	if (open && row.kind === 'album' && !row.tracksLoaded) {
		void sendLibraryCommand({ command: 'openAlbum', albumId: row.albumId });
	}
}

/** The id list with `id` present when `present` and absent otherwise, keeping
 * the other ids' order. */
function withId(ids: string[], id: string, present: boolean): string[] {
	const rest = ids.filter((candidate) => candidate !== id);
	return present ? [...rest, id] : rest;
}

/** Press a row (design D7): an artist or album row opens or closes and is
 * picked; a track row is picked. The row's painted open state is the half to
 * flip, so a press under a filter closes the node the filter opened. A press
 * moves no cursor — that is the keys' work. */
export function pressLibraryRow(row: LibraryRow): void {
	if (row.kind === 'letter') {
		return;
	}
	if (row.kind === 'track') {
		showDetail(pickedItemId(row));
		return;
	}
	setNodeOpen(row, !row.open);
	showDetail(pickedItemId(row));
}

/** Move the list's cursor one movement along (design D3, D7): Up and Down
 * move a row, Page Up and Page Down move the rows in view, and Home and End
 * take the list's ends. The scroll offset follows, so the cursor row stays
 * fully visible. */
export function moveLibraryCursor(move: ListMove): void {
	libraryUi.update((ui) => {
		const rows = rowsOf(ui);
		const view = listWindow(rows, ui.scrollTop, ui.viewportHeight);
		const cursor = moveListCursor(rows, ui.cursor, move, view.end - view.start);
		return {
			...ui,
			cursor,
			scrollTop: scrollTopToShowCursor(rows, cursor, ui.scrollTop, ui.viewportHeight),
		};
	});
}

/** Pick the item under the cursor (design D3's Enter): the Hero shows it on the
 * next frame. Enter's play lands with the Hero's own actions. */
export function pickLibraryCursor(): void {
	const row = cursorRow(get(libraryUi));
	if (row !== null) {
		showDetail(pickedItemId(row));
	}
}

/** Open or close the cursor's node (design D7's Right and Left). */
export function setLibraryCursorOpen(open: boolean): void {
	const row = cursorRow(get(libraryUi));
	if (row !== null && (row.kind === 'artist' || row.kind === 'album')) {
		setNodeOpen(row, open);
	}
}

/** Narrow the list to the names that contain `filter`, without case (design
 * D7). The cursor keeps its row while that row stays painted, and otherwise
 * falls to the first painted row; the scroll offset follows the narrower list.
 * New text starts the filter's own open and close over: the nodes the user
 * closed under the old text open again. */
export function setLibraryFilter(filter: string): void {
	libraryUi.update((ui) => {
		const next = {
			...ui,
			filter,
			filterClosed: filter === ui.filter ? ui.filterClosed : [],
		};
		return resolveCursor(next);
	});
}

/** Jump to a letter of the rail (design D7): the cursor moves to the first row
 * under the letter and that row scrolls into view. A letter without rows
 * changes nothing. */
export function jumpToLetter(letter: string): void {
	libraryUi.update((ui) => {
		const rows = rowsOf(ui);
		const target = letterRail(rows).find((entry) => entry.letter === letter)?.rowId;
		if (target === null || target === undefined) {
			return ui;
		}
		return {
			...ui,
			cursor: target,
			scrollTop: scrollTopToShowCursor(rows, target, ui.scrollTop, ui.viewportHeight),
		};
	});
}

/** Report the list's measured viewport height (design D7): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measureLibraryList(viewportHeight: number): void {
	libraryUi.update((ui) => {
		const next = { ...ui, viewportHeight };
		return {
			...next,
			scrollTop: clampScrollTop(rowsOf(next), next.scrollTop, viewportHeight),
		};
	});
}

/** Report the list's native scroll offset (design D7): a user scroll moves the
 * rows in view, and the offset stays inside the list's range. */
export function scrollLibraryList(scrollTop: number): void {
	libraryUi.update((ui) => {
		const top = clampScrollTop(rowsOf(ui), scrollTop, ui.viewportHeight);
		return top === ui.scrollTop ? ui : { ...ui, scrollTop: top };
	});
}

/** The pill the pill row's focus holds, resolved against the carried content
 * so a stale id focuses nothing. */
export function focusedLibraryPill(
	pills: readonly GuiMusicPill[],
	ui: LibraryUiState,
): GuiMusicPill | null {
	if (ui.pane !== 'pills' || ui.focusedPill === null) {
		return null;
	}
	return pills.find((pill) => pill.groupId === ui.focusedPill) ?? null;
}
