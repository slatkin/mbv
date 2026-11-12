// The plain Emby-library list's row model (add-gui-library-panel design D12,
// row 8.1): the plain mode's letter headings over its name-sorted rows, the
// feed-group mode's flat rows with their date gutter, the rows in view, the
// cursor's movement, and the A–Z rail. The list paints fixed-height rows over
// native scroll and only the rows in view, as the Music and Shows lists do,
// and the cursor moves over the item rows only — a heading is structural, the
// cursor never rests on it. The model is plain data over fixed heights, so the
// geometry is asserted without a layout, which jsdom does not have.

import type { GuiEmbyFeedGroup, GuiEmbyRow } from '$lib/bridge/types';
import type { RailLetter } from './list';
import {
	LETTER_ROW_HEIGHT,
	buildFlatRows,
	filterMatch,
	letterRail,
	moveCursor,
} from './list';
import type { ListMove } from './list';
import { memoRows } from './row-memo';

/** The letter heading row's height in logical pixels (design D12): the Music
 * list's letter heading, which the plain list reuses. */
export const EMBY_HEADING_ROW_HEIGHT = LETTER_ROW_HEIGHT;
/** The item row's height in logical pixels: the room a row's title, its
 * secondary label, and the feed-group list's date gutter paint into. Both
 * modes paint at this one height, so a mode switch changes the rows and never
 * their geometry. */
export const EMBY_ROW_HEIGHT = 40;

/** One painted row of the plain Emby-library list (design D12, row 8.1). The
 * `id` is the row's stable key: the cursor's identity, the each-block's key,
 * and the id the scroll geometry reads. */
export type EmbyListRow =
	| { kind: 'heading'; id: string; top: number; height: number; letter: string }
	| EmbyItemRow;

/** A painted row the cursor may rest on and the keys act on (row 8.1): every
 * item row, a folder and a boxset included, because the folder-play rule plays
 * a collection row. A heading is structural. */
export interface EmbyItemRow {
	kind: 'item';
	id: string;
	top: number;
	height: number;
	row: GuiEmbyRow;
}

/** The rows the cursor may rest on: every item row, never a heading (row
 * 8.1). */
export function embySelectableRows(rows: readonly EmbyListRow[]): EmbyItemRow[] {
	return rows.filter((row): row is EmbyItemRow => row.kind === 'item');
}

/**
 * The painted rows of the list (design D12, row 8.1): in plain mode the
 * name-sorted rows, each under the letter heading of its rail letter — the
 * Music list's heading rule, one heading per run of the same letter — and in
 * feed-group mode the flat rows in the server's date order with no heading at
 * all, because that list carries no rail. The filter narrows the rows to the
 * titles that contain its text, without case, and a heading paints only above
 * a kept row (design D7's filter rule, the rail's keyboard path). Every row
 * carries its top edge and height, so the list paints a window of rows over
 * native scroll.
 */
export function buildEmbyRows(
	rows: readonly GuiEmbyRow[],
	feedGroup: boolean,
	filter = '',
): EmbyListRow[] {
	return feedGroup
		? buildFlatRows(rows, EMBY_ROW_HEIGHT, filter, 'item')
		: headingEmbyRows(rows.filter((row) => filterMatch(row.title, filter)));
}

/** The plain mode's rows: each row under the letter heading of its rail
 * letter, one heading per run of the same letter. */
function headingEmbyRows(rows: readonly GuiEmbyRow[]): EmbyListRow[] {
	const built: EmbyListRow[] = [];
	let top = 0;
	let heading: string | null = null;
	for (const row of rows) {
		const letter = row.sortLetter ?? '#';
		if (letter !== heading) {
			heading = letter;
			built.push({
				kind: 'heading',
				id: `emby-letter:${letter}`,
				letter,
				top,
				height: EMBY_HEADING_ROW_HEIGHT,
			});
			top += EMBY_HEADING_ROW_HEIGHT;
		}
		built.push({ kind: 'item', id: row.id, row, top, height: EMBY_ROW_HEIGHT });
		top += EMBY_ROW_HEIGHT;
	}
	return built;
}

/** The rail letter a painted row belongs to, or `null` on a heading and on the
 * letterless feed-group rows. */
export function embyRowLetter(row: EmbyListRow): string | null {
	if (row.kind === 'heading') {
		return row.letter;
	}
	return row.row.sortLetter;
}

/**
 * The cursor key a movement leaves (row 8.1): the next, the previous, or a
 * page away item row, clamped to the list's ends, stepping over the letter
 * headings. With no cursor on an item row, Up and End start from the last item
 * row and every other movement from the first, so one key press always lands
 * the cursor. `page` is the rows a page spans: the rows in view.
 */
export function moveEmbyCursor(
	rows: readonly EmbyListRow[],
	cursor: string | null,
	move: ListMove,
	page = 1,
): string | null {
	return moveCursor(embySelectableRows(rows), cursor, move, page);
}

/** The A–Z rail over the painted rows (row 8.1, design D7): each letter with
 * the first row under it, so a press moves the cursor there. A letter no
 * painted row carries is disabled, as the Music rail's is. The feed-group mode
 * paints no rail, so its rows carry no letter and the rail stays disabled. */
export function embyLetterRail(rows: readonly EmbyListRow[]): RailLetter[] {
	return letterRail(rows, (row) => (row.kind === 'item' ? row.row.sortLetter : null));
}

/** The stable key a feed-group pill is focused and pressed by (row 8.1): the
 * group's own identity, so the pill row and the keys agree on it without a row
 * position. */
export function embyGroupKey(group: GuiEmbyFeedGroup): string {
	return group.kind === 'all' ? 'all' : group.kind === 'latest' ? 'latest' : `folder:${group.id}`;
}

/** The Emby rows, memoized on the carried rows, the feed-group flag, and the
 * filter (design D9). */
export const embyRowsOf = memoRows(buildEmbyRows);
