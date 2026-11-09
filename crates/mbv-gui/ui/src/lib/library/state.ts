// The Library panel's own state (add-gui-library-panel design D1): what the
// window content owns and the core never reads. Two stores keep the two kinds
// apart — the content the frame carries, kept from the last frame that carried
// it, and the interaction state the panel's keys and controls move.
//
// The core owns the data selection (which collections are selected, the picked
// item), so nothing here mirrors it: the pill toggle the keys request is an
// intent the pill row turns into the set-collections command (design D6).

import { get, writable } from 'svelte/store';
import type { GuiArrangement } from '$lib/bridge/arrangement';
import { sendLibraryCommand } from '$lib/bridge/library';
import type {
	GuiLibraryContent,
	GuiLibraryFrame,
	GuiMusicFrame,
	GuiMusicPill,
} from '$lib/bridge/types';
import { heroTracks, moveTrackCursor, resolveTrackCursor } from './hero';
import {
	buildLibraryRows,
	clampScrollTop,
	letterRail,
	listWindow,
	moveListCursor,
	scrollTopToShowCursor,
} from './list';
import type { LibraryNodeRow, LibraryRow, LibraryTree, ListMove } from './list';

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
	/** The pill selection the keys asked for: Space and Enter select the
	 * the focused pill (design D6). The pill row turns it into the
	 * select-collection command. */
	pillSelect: { groupId: string } | null;
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
	/** The tracklist's cursor track id (design D3): the Hero's tracklist row
	 * the movement keys rest on while the Hero pane has the pane focus. */
	trackCursor: string | null;
	/** Whether the Narrow arrangement's Hero overlay is dismissed (design D4,
	 * D3): a pick paints it again, and Esc or a press on its backdrop closes
	 * it. */
	heroDismissed: boolean;
	/** Whether the picked item's menu is open (design D8): More actions opens
	 * it, a chosen entry runs and closes it, and Esc closes it. */
	itemMenuOpen: boolean;
	/** The right-click item menu (the row-menu command): the item a row
	 * right-press named with the pointer position it opened at. `null` while
	 * no right-click menu is up. The menu paints only while the frame carries
	 * that item's entries, so a stale open state paints nothing. */
	rowMenu: { itemId: string; x: number; y: number } | null;
	/** The arrangement the panel paints, reported by the panel (design D4): the
	 * keys close the Narrow Hero overlay only where one paints, so Esc in Wide
	 * stays the core's. `null` until the panel reports. */
	arrangement: GuiArrangement | null;
}

/** The state a freshly mounted window content starts with. */
export function initialLibraryUi(): LibraryUiState {
	return {
		pane: 'pills',
		filterOpen: false,
		filter: '',
		focusedPill: null,
		pillSelect: null,
		cursor: null,
		openNodes: [],
		filterClosed: [],
		scrollTop: 0,
		viewportHeight: 0,
		trackCursor: null,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
		arrangement: null,
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

/** Whether the Library panel shows (design D2, D10): the frame carries the
 * Library section and the window holds the Music content the panel paints. The
 * window mounts the panel on this one fact and the key router gates on it, so
 * no Library key acts while the panel paints nothing. */
export function libraryShown(
	library: GuiLibraryFrame | null,
	music: GuiMusicFrame | null,
): boolean {
	return library !== null && music !== null;
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

/** Focus a pane: the window content of Tab and Shift+Tab (design D3, step 3).
 * Taking the Hero's pane paints the Hero again where the arrangement overlays
 * it, so the keys never act on a pane that paints nothing. */
export function focusLibraryPane(pane: LibraryPane): void {
	libraryUi.update((ui) => withHeroPane({ ...ui, pane }));
}

/** Move the pane focus `step` panes along, wrapping at the ends. */
export function moveLibraryPane(step: number): void {
	libraryUi.update((ui) => {
		const at = PANES.indexOf(ui.pane);
		const next = (at + step + PANES.length) % PANES.length;
		return withHeroPane({ ...ui, pane: PANES[next] });
	});
}

/** Focus a pill, or no pill while `groupId` is `null`. */
export function focusLibraryPill(groupId: string | null): void {
	libraryUi.update((ui) => ({ ...ui, focusedPill: groupId }));
}

/** Ask for the focused pill to be selected: Space and Enter request it (design
 * D6). */
export function requestLibraryPillSelect(groupId: string): void {
	libraryUi.update((ui) => ({ ...ui, pillSelect: { groupId } }));
}

/** Select the collection pill `groupId` of the Grouped Music library `libIdx`
 * (design D6): one group is always selected, as in the TUI. The core stores the
 * choice per destination and loads the group, and the pills paint the core's
 * choice back, so the row and the list cannot disagree. */
export function selectLibraryPill(libIdx: number, groupId: string): void {
	void sendLibraryCommand({ command: 'selectCollection', libIdx, groupId });
}

/** Turn the keys' pill request into the select-collection command the pill
 * sends (design D6). The intent is consumed here, so one key sends one
 * command. */
export function applyLibraryPillSelect(libIdx: number): void {
	const { pillSelect } = get(libraryUi);
	if (pillSelect === null) {
		return;
	}
	libraryUi.update((ui) => ({ ...ui, pillSelect: null }));
	selectLibraryPill(libIdx, pillSelect.groupId);
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
 * the next frame carries the Hero. No command for a row that names no item. A
 * pick paints the Narrow arrangement's Hero overlay again (design D4), closes
 * the item menu the previous item opened, and starts the tracklist's cursor
 * over, because the tracks it rested on belong to the previous album. */
function showDetail(itemId: string | null): void {
	if (itemId === null) {
		return;
	}
	void sendLibraryCommand({ command: 'showDetail', itemId });
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
		trackCursor: null,
	}));
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

/** The item an action on the cursor names (design D3, D8): an artist row's
 * Service item, an album's own id, and a track's id, so the play-next and
 * add-to-queue keys act on what the cursor rests on. */
function actionItemId(row: LibraryRow): string | null {
	switch (row.kind) {
		case 'letter':
			return null;
		case 'artist':
			return row.itemId;
		case 'album':
			return row.albumId;
		case 'track':
			return row.trackId;
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

/** Add a track to the queue (add-gui-library-panel, the user's double-click
 * contract): the track's own stable id, through the core's add path, which
 * applies the library playback policy's autoload rule. The list's track rows
 * and the Hero's tracklist rows both send it, so one gesture cannot drift
 * from the other. */
export function addLibraryTrackToQueue(trackId: string): void {
	void sendLibraryCommand({ command: 'addToQueue', itemId: trackId });
}

/** Press a row (design D7): an artist or album row opens or closes and is
 * picked; a track row moves the cursor onto itself and is picked — the pick's
 * album shows in the Hero, and the cursor is the press's visible half, as the
 * Queue panel's click moves the queue cursor. The row's painted open state is
 * the half to flip, so a press under a filter closes the node the filter
 * opened. A press on an artist or album row moves no cursor — that is the
 * keys' work. */
export function pressLibraryRow(row: LibraryRow): void {
	if (row.kind === 'letter') {
		return;
	}
	if (row.kind === 'track') {
		libraryUi.update((ui) => {
			const rows = rowsOf(ui);
			return {
				...ui,
				cursor: row.id,
				scrollTop: scrollTopToShowCursor(rows, row.id, ui.scrollTop, ui.viewportHeight),
			};
		});
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

/** The state with the Hero pane ready while it takes the pane focus (design
 * D3, D4): the Narrow arrangement paints the Hero again, so the keys never act
 * on a pane that paints nothing, and the tracklist's cursor is derived from the
 * tracks that paint rather than seeded here. */
function withHeroPane(ui: LibraryUiState): LibraryUiState {
	return ui.pane === 'hero' ? { ...ui, heroDismissed: false } : ui;
}

/** Move the Hero's tracklist cursor one movement along (design D3): the
 * movement keys move the tracklist's cursor, not the list's, while the Hero
 * pane has the pane focus. The movement starts from the cursor the painted
 * tracklist shows, so the first key lands beside it. */
export function moveLibraryTrackCursor(move: ListMove): void {
	libraryUi.update((ui) => {
		const tracks = heroTracks(libraryMusic()?.hero ?? null);
		const cursor = moveTrackCursor(
			tracks,
			resolveTrackCursor(tracks, ui.trackCursor),
			move,
		);
		return cursor === ui.trackCursor ? ui : { ...ui, trackCursor: cursor };
	});
}

/** The item the focused pane's cursor names (design D3, D8): the tracklist's
 * track while the Hero pane has the pane focus, and the list's row while the
 * list has it. `null` while no cursor rests on an item, so the key that acts
 * on it sends no command. The Hero pane's track is the one the painted
 * tracklist shows, so Enter acts before any movement key has moved the
 * cursor. */
export function focusedItemId(): string | null {
	const ui = get(libraryUi);
	if (ui.pane === 'hero') {
		return resolveTrackCursor(
			heroTracks(libraryMusic()?.hero ?? null),
			ui.trackCursor,
		);
	}
	if (ui.pane === 'list') {
		const row = cursorRow(ui);
		return row === null ? null : actionItemId(row);
	}
	return null;
}

/** Report the arrangement the panel paints (design D4): the panel measures it
 * from the Library column, and the keys read it to close the Narrow Hero
 * overlay only where one paints. */
export function reportLibraryArrangement(arrangement: GuiArrangement): void {
	libraryUi.update((ui) =>
		ui.arrangement === arrangement ? ui : { ...ui, arrangement },
	);
}

/** Whether the Narrow arrangement paints the Hero overlay (design D4, D3): a
 * picked item, an arrangement that overlays rather than splits, and no
 * dismissal since that pick. Esc and a press on the backdrop close it. */
export function heroOverlayOpen(ui: LibraryUiState): boolean {
	return (
		ui.arrangement === 'narrow' &&
		!ui.heroDismissed &&
		(libraryMusic()?.hero ?? null) !== null
	);
}

/** Close the Narrow Hero overlay (design D3): Esc while it paints, and a press
 * on its backdrop. A later pick, or a Tab onto the Hero's pane, paints it
 * again. While the Hero pane held the pane focus, the focus falls back to the
 * list, so the keys never rest on a pane that paints nothing. */
export function closeLibraryHero(): void {
	libraryUi.update((ui) => ({
		...ui,
		pane: ui.pane === 'hero' ? 'list' : ui.pane,
		heroDismissed: true,
		itemMenuOpen: false,
	}));
}

/** Open the picked item's menu (design D8): More actions paints the entries the
 * frame carries. */
export function openLibraryItemMenu(): void {
	libraryUi.update((ui) =>
		ui.itemMenuOpen ? ui : { ...ui, itemMenuOpen: true, rowMenu: null },
	);
}

/** Close the picked item's menu (design D8): a chosen entry closes it, and so
 * does Esc. */
export function closeLibraryItemMenu(): void {
	libraryUi.update((ui) => (ui.itemMenuOpen ? { ...ui, itemMenuOpen: false } : ui));
}

/** Open the right-click item menu for `itemId` (the row-menu command): the
 * next frame carries the item's full entries for the menu to paint at the
 * pointer position, and a chosen entry sends its index for that item. The
 * Hero's own menu closes with it — one menu is up at a time. */
export function openLibraryRowMenu(itemId: string, x: number, y: number): void {
	void sendLibraryCommand({ command: 'rowMenu', itemId });
	libraryUi.update((ui) => ({ ...ui, itemMenuOpen: false, rowMenu: { itemId, x, y } }));
}

/** Close the right-click item menu: a chosen entry closes it, and so do Esc,
 * a press outside it, and a scroll of the surface the row painted in. */
export function closeLibraryRowMenu(): void {
	libraryUi.update((ui) => (ui.rowMenu === null ? ui : { ...ui, rowMenu: null }));
}

/** Run the right-click menu's entry at `index` for the item it opened on: the
 * same menu-entry command the Hero's menu sends, resolved by that item's
 * stable id in the core. The menu closes behind the press. */
export function runLibraryRowMenuEntry(index: number): void {
	const { rowMenu } = get(libraryUi);
	const itemId = rowMenu?.itemId;
	closeLibraryRowMenu();
	if (itemId === undefined) {
		return;
	}
	void sendLibraryCommand({ command: 'menuEntry', itemId, index });
}
