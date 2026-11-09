// The Audiobooks list's row model (add-gui-library-panel design D12, row
// 9.1): the catalog's book rows under the letter headings of their author sort
// key, the rows in view, the cursor's movement, and the A–Z letter rail. There
// is no pill row and no second mode: the destination is one flat book list, and
// the letter rail replaces the TUI's surname buckets as it does for Shows and
// the plain Emby libraries. The list paints fixed-height rows over native
// scroll and only the rows in view, as the Music and Emby lists do, and the
// cursor moves over the item rows only — a heading is structural, the cursor
// never rests on it. The model is plain data over fixed heights, so the
// geometry is asserted without a layout, which jsdom does not have.

import type { GuiAudiobooksRow } from '$lib/bridge/types';
import type { ListWindow, RailLetter } from './list';
import { LETTER_ROW_HEIGHT, RAIL_LETTERS, listWindow, rowsHeight } from './list';

/** The letter heading row's height in logical pixels (design D12, row 9.1):
 * the Music list's letter heading, which the book list reuses. */
export const AUIB_HEADING_ROW_HEIGHT = LETTER_ROW_HEIGHT;
/** The book row's height in logical pixels: the room a row's title, its
 * author line, and the duration gutter paint into. */
export const AUIB_ROW_HEIGHT = 40;

/** One painted row of the Audiobooks list (design D12, row 9.1). The `id` is
 * the row's stable key: the cursor's identity, the each-block's key, and the
 * id the scroll geometry reads. A heading is structural; an item row is a
 * book. */
export type AudiobooksListRow =
	| { kind: 'heading'; id: string; top: number; height: number; letter: string }
	| AudiobooksItemRow;

/** A painted row the cursor may rest on and the keys act on (row 9.1): a
 * book. A heading names no book. */
export interface AudiobooksItemRow {
	kind: 'item';
	id: string;
	top: number;
	height: number;
	row: GuiAudiobooksRow;
}

/** The rows in view over the book list (row 9.1): the shared list window's
 * own answer for the Audiobooks row heights. */
export function audiobooksListWindow(
	rows: readonly AudiobooksListRow[],
	scrollTop: number,
	viewportHeight: number,
): ListWindow {
	return listWindow(rows, scrollTop, viewportHeight);
}

/** The painted height of the book list (row 9.1): the shared height's own
 * answer for the Audiobooks row heights. */
export function audiobooksRowsHeight(rows: readonly AudiobooksListRow[]): number {
	return rowsHeight(rows);
}

/**
 * The painted rows of the book list (design D12, row 9.1): the catalog's book
 * rows in the TUI's sort — author sort key, then title — each under the letter
 * heading of its rail letter, one heading per run of the same letter. Every
 * row carries its top edge and height, so the list paints a window of rows
 * over native scroll.
 */
export function buildAudiobooksRows(rows: readonly GuiAudiobooksRow[]): AudiobooksListRow[] {
	const built: AudiobooksListRow[] = [];
	let top = 0;
	let heading: string | null = null;
	for (const row of rows) {
		const letter = row.sortLetter;
		if (letter !== heading) {
			heading = letter;
			built.push({
				kind: 'heading',
				id: `abs-letter:${letter}`,
				letter,
				top,
				height: AUIB_HEADING_ROW_HEIGHT,
			});
			top += AUIB_HEADING_ROW_HEIGHT;
		}
		built.push({ kind: 'item', id: row.id, row, top, height: AUIB_ROW_HEIGHT });
		top += AUIB_ROW_HEIGHT;
	}
	return built;
}

/** The rows the cursor may rest on: every item row, never a heading (row
 * 9.1). */
export function audiobooksSelectableRows(rows: readonly AudiobooksListRow[]): AudiobooksItemRow[] {
	return rows.filter((row): row is AudiobooksItemRow => row.kind === 'item');
}

/** The rail letter a painted row belongs to, or `null` on a heading (row
 * 9.1). A book row always carries its sort letter. */
export function audiobooksRowLetter(row: AudiobooksListRow): string | null {
	if (row.kind === 'heading') {
		return row.letter;
	}
	return row.row.sortLetter;
}

/**
 * The cursor key a movement leaves (row 9.1): the next, the previous, or a
 * page away book row, clamped to the list's ends, stepping over the letter
 * headings. With no cursor on an item row, Up and End start from the last book
 * and every other movement from the first, so one key press always lands the
 * cursor. `page` is the rows a page spans: the rows in view.
 */
export function moveAudiobooksCursor(
	rows: readonly AudiobooksListRow[],
	cursor: string | null,
	move: 'up' | 'down' | 'pageUp' | 'pageDown' | 'home' | 'end',
	page = 1,
): string | null {
	const items = audiobooksSelectableRows(rows);
	if (items.length === 0) {
		return null;
	}
	const last = items.length - 1;
	const at = items.findIndex((row) => row.id === cursor);
	if (at < 0) {
		return (move === 'up' || move === 'end' ? items[last] : items[0]).id;
	}
	const step = Math.max(page, 1);
	const to = {
		up: at - 1,
		down: at + 1,
		pageUp: at - step,
		pageDown: at + step,
		home: 0,
		end: last,
	}[move];
	return items[Math.min(Math.max(to, 0), last)].id;
}

/** The A–Z rail over the painted rows (row 9.1, design D7): each letter with
 * the first book under it, so a press moves the cursor there. A letter no
 * painted row carries is disabled, as the Music rail's is. */
export function audiobooksLetterRail(rows: readonly AudiobooksListRow[]): RailLetter[] {
	return [...RAIL_LETTERS].map((letter) => ({
		letter,
		rowId:
			rows.find((row) => row.kind === 'item' && row.row.sortLetter === letter)?.id ??
			null,
	}));
}
