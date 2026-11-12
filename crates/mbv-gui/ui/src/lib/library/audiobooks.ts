// The Audiobooks destination's interaction state (add-gui-library-panel
// design D12, row 9.1): what the window content owns and the core never reads
// — the cursor, the scroll offset, and the measured viewport. The core owns
// the selected book (the frame's `selected` and its Hero), so nothing here
// mirrors it: a press sends the select command, and the frame carries the
// choice back. The row model is `$lib/library/audiobooks-list`'s; the Hero's
// chapter cursor belongs to the Hero (`$lib/library/audiobooks-workspace`),
// and the panel's pane focus and the Hero overlay stay in `./state`'s shared
// store. There is no pill row on the destination.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import { clampScrollTop, jumpToLetter, listWindow, scrollTopToShowCursor } from './list';
import type { ListMove } from './list';
import { libraryAudiobooksList } from './parts';
import { libraryUi } from './state';
import { audiobooksScroll } from './scroll';
import {
	buildAudiobooksRows,
	moveAudiobooksCursor,
	audiobooksLetterRail,
} from './audiobooks-list';
import type { AudiobooksItemRow, AudiobooksListRow } from './audiobooks-list';

/** The book list's interaction state. */
export interface AudiobooksUiState {
	/** The cursor's row id, or `null` while no row carries the cursor (row
	 * 9.1). The cursor moves over the book rows only. */
	cursor: string | null;
	/** The `/` filter's text: the list paints only the books whose title
	 * contains it, without case (design D7's filter rule, the rail's keyboard
	 * path). */
	filter: string;
}

/** The state a freshly mounted book list starts with. */
export function initialAudiobooksUi(): AudiobooksUiState {
	return { cursor: null, filter: '' };
}

/** The book list's interaction state. */
export const audiobooksUi = writable<AudiobooksUiState>(initialAudiobooksUi());

/** The painted rows of the carried book content (row 9.1). */
function bookRows(filter = get(audiobooksUi).filter): AudiobooksListRow[] {
	const audiobooks = libraryAudiobooksList();
	return audiobooks === null ? [] : buildAudiobooksRows(audiobooks.rows, filter);
}

/** The row the cursor rests on, or `null` while no painted row carries it. */
function cursorRow(
	list: readonly AudiobooksListRow[],
	cursor: string | null,
): AudiobooksItemRow | null {
	const row = list.find((candidate) => candidate.id === cursor);
	return row === undefined || row.kind === 'heading' ? null : row;
}

/** The row the book cursor rests on, or `null` (row 9.1): the keys read it to
 * play the book or queue it. */
export function audiobooksCursorRow(): AudiobooksItemRow | null {
	return cursorRow(bookRows(), get(audiobooksUi).cursor);
}

/** Scroll the list so the cursor's row paints fully, in the scroll store
 * (row 9.1, design D9). */
function followAudiobooksCursor(list: readonly AudiobooksListRow[], cursor: string | null): void {
	audiobooksScroll.update((scroll) => {
		const scrollTop = scrollTopToShowCursor(list, cursor, scroll.scrollTop, scroll.viewportHeight);
		return scrollTop === scroll.scrollTop ? scroll : { ...scroll, scrollTop };
	});
}

/** Settle the cursor once the painted rows may have narrowed (row 9.1): the
 * cursor keeps its row while that row still paints, and falls to the first
 * painted row otherwise. The scroll store then shows the cursor — the Music
 * filter's own resolve (design D7). */
function settleAudiobooksCursor(): void {
	const ui = get(audiobooksUi);
	const list = bookRows(ui.filter);
	const cursor = list.some((row) => row.id === ui.cursor)
		? ui.cursor
		: moveAudiobooksCursor(list, null, 'home');
	if (cursor !== ui.cursor) {
		audiobooksUi.update((current) => ({ ...current, cursor }));
	}
	followAudiobooksCursor(list, cursor);
}

/** Narrow the list to the titles that contain `filter`, without case (design
 * D7's filter rule): the cursor follows the rows. */
export function setAudiobooksFilter(filter: string): void {
	audiobooksUi.update((ui) => ({ ...ui, filter }));
	settleAudiobooksCursor();
}

/** Close the filter with Esc: the text goes with it, so a closed filter
 * narrows nothing (design D3, step 2). The cursor follows the widened rows. */
export function closeAudiobooksFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
	audiobooksUi.update((ui) => ({ ...ui, filter: '' }));
	settleAudiobooksCursor();
}

/** Accept the filter with Enter: the field closes and the list keeps the
 * text (design D3, step 2). */
export function acceptAudiobooksFilter(): void {
	libraryUi.update((ui) => ({ ...ui, filterOpen: false }));
}

/** Select the book (row 9.1): the select command the core stores, whose frame
 * paints the Hero. A select repaints the Narrow arrangement's Hero overlay and
 * closes an open menu, because the facts they paint belong to the previous
 * book, as a Music pick's do. */
export function selectAudiobooksRow(row: AudiobooksListRow): void {
	const audiobooks = libraryAudiobooksList();
	if (audiobooks === null || row.kind === 'heading') {
		return;
	}
	void sendLibraryCommand({
		command: 'absSelect',
		libIdx: audiobooks.libIdx,
		row: row.row.id,
	});
	libraryUi.update((ui) => ({
		...ui,
		heroDismissed: false,
		itemMenuOpen: false,
		rowMenu: null,
	}));
}

/** Press a book row (row 9.1): the press selects the book the Hero shows. A
 * heading names no book, so its press changes nothing. */
export function pressAudiobooksRow(row: AudiobooksListRow): void {
	selectAudiobooksRow(row);
}

/** Play a book (row 9.1): Enter on the cursor book, and a double-click's play.
 * The Owner replaces the queue with the book at its saved position, through
 * the shared play path's admission. The book id is the row's stable
 * `library_item_id`. */
export function playAudiobooksRow(bookId: string): void {
	const audiobooks = libraryAudiobooksList();
	if (audiobooks === null) {
		return;
	}
	void sendLibraryCommand({ command: 'absPlay', libIdx: audiobooks.libIdx, itemId: bookId });
}

/** Add a book to the queue's tail (row 9.1): `a` on the cursor book. Books
 * have no play-next, so `n` is consumed and does nothing here. */
export function queueAudiobooksRow(bookId: string): void {
	const audiobooks = libraryAudiobooksList();
	if (audiobooks === null) {
		return;
	}
	void sendLibraryCommand({ command: 'absQueue', libIdx: audiobooks.libIdx, itemId: bookId });
}

/** Move the book list's cursor one movement along (row 9.1): Up and Down move
 * a row, Page Up and Page Down move the rows in view, and Home and End take
 * the list's ends, stepping over the letter headings. The scroll offset
 * follows, so the cursor row stays fully visible. */
export function moveAudiobooksListCursor(move: ListMove): void {
	const list = bookRows();
	const current = get(audiobooksUi);
	const scroll = get(audiobooksScroll);
	const view = listWindow(list, scroll.scrollTop, scroll.viewportHeight);
	const cursor = moveAudiobooksCursor(list, current.cursor, move, view.end - view.start);
	if (cursor === current.cursor) {
		return;
	}
	audiobooksUi.update((ui) => ({ ...ui, cursor }));
	followAudiobooksCursor(list, cursor);
}

/** Jump to a letter of the rail (row 9.1, design D7): the cursor moves to the
 * first book under the letter and that book scrolls into view. A letter
 * without books changes nothing. */
export function jumpAudiobooksToLetter(letter: string): void {
	const list = bookRows();
	const jump = jumpToLetter(audiobooksLetterRail(list), list, letter, get(audiobooksScroll));
	if (jump === null) {
		return;
	}
	audiobooksUi.update((ui) => ({ ...ui, cursor: jump.cursor }));
	audiobooksScroll.update((scroll) => ({ ...scroll, scrollTop: jump.scrollTop }));
}

/** Report the list's measured viewport height (row 9.1): the rows in view
 * follow it, and the scroll offset stays inside the narrower range. */
export function measureAudiobooksList(viewportHeight: number): void {
	audiobooksScroll.update((scroll) => ({
		...scroll,
		viewportHeight,
		scrollTop: clampScrollTop(bookRows(), scroll.scrollTop, viewportHeight),
	}));
}

/** Report the list's native scroll offset (row 9.1): a user scroll moves the
 * rows in view, and the offset stays inside the list's range. */
export function scrollAudiobooksList(scrollTop: number): void {
	audiobooksScroll.update((scroll) => {
		const top = clampScrollTop(bookRows(), Math.max(scrollTop, 0), scroll.viewportHeight);
		return top === scroll.scrollTop ? scroll : { ...scroll, scrollTop: top };
	});
}
