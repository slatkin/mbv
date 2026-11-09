// The Shows list's row model (add-gui-library-panel design D12, the
// USER-APPROVED GUI Shows design): the All series mode's letter headings over
// its series rows, the Latest and Upcoming modes' flat rows with their date
// gutter, the rows in view, the cursor's movement, and the A–Z letter rail.
// The list paints fixed-height rows over native scroll and only the rows in
// view, as the Music list does, and the cursor moves over the item rows only —
// a heading is structural, the cursor never rests on it. The model is plain
// data over fixed heights, so the geometry is asserted without a layout, which
// jsdom does not have.

import type { GuiShowsFrame, GuiShowsMode, GuiShowsRow } from '$lib/bridge/types';
import type { ListWindow, RailLetter } from './list';
import {
	LETTER_ROW_HEIGHT,
	NODE_ROW_HEIGHT,
	RAIL_LETTERS,
	listWindow,
	rowsHeight,
} from './list';

/** The letter heading row's height in logical pixels (design D12): the Music
 * list's letter heading, which the All series list reuses. */
export const SHOWS_HEADING_ROW_HEIGHT = LETTER_ROW_HEIGHT;
/** The series row's height in logical pixels: the Music list's node row. */
export const SHOWS_SERIES_ROW_HEIGHT = NODE_ROW_HEIGHT;
/** The flat episode or premiere row's height in logical pixels: the room a
 * two-line title and the date gutter paint into, as a Home item row. */
export const SHOWS_FLAT_ROW_HEIGHT = 48;

/** One painted row of the Shows list (design D12). The `id` is the row's
 * stable key: the cursor's identity, the each-block's key, and the id the
 * scroll geometry reads. A `series` row is the All series mode's row, which is
 * not playable, and a `flat` row is an episode or a premiere. */
export type ShowsListRow =
	| { kind: 'heading'; id: string; top: number; height: number; letter: string }
	| ShowsItemRow;

/** A painted row the cursor may rest on and the keys act on (design D12): the
 * series rows and the flat episode or premiere rows. A heading is structural. */
export type ShowsItemRow =
	| { kind: 'series'; id: string; top: number; height: number; row: GuiShowsRow }
	| { kind: 'flat'; id: string; top: number; height: number; row: GuiShowsRow };

/** The active view mode (design D12): the pill the core marked selected. A
 * frame that marks none falls back to the rows' own shape — the letter-carrying
 * rows are the All series list — so the list always has one mode. */
export function activeShowsMode(shows: GuiShowsFrame): GuiShowsMode {
	const selected = shows.pills.find((pill) => pill.selected);
	if (selected !== undefined) {
		return selected.mode;
	}
	return shows.rows.some((row) => row.sortLetter !== null) ? 'all' : 'latest';
}

/** Whether the mode paints the series list rather than a flat episode or
 * premiere list (design D12). */
export function showsSeriesMode(mode: GuiShowsMode): boolean {
	return mode === 'all';
}

/** The rows in view over the Shows list (design D12): the shared list
 * window's own answer for the Shows row heights. */
export function showsListWindow(
	rows: readonly ShowsListRow[],
	scrollTop: number,
	viewportHeight: number,
): ListWindow {
	return listWindow(rows, scrollTop, viewportHeight);
}

/** The painted height of the Shows list (design D12): the shared height's own
 * answer for the Shows row heights. */
export function showsRowsHeight(rows: readonly ShowsListRow[]): number {
	return rowsHeight(rows);
}

/**
 * The painted rows of the Shows list (design D12): in the All series mode the
 * series rows in the server's sort order, each under the letter heading of
 * its rail letter — the Music list's heading rule, one heading per letter —
 * and in the Latest and Upcoming modes the flat rows with no heading at all.
 * Every row carries its top edge and height, so the list paints a window of
 * rows over native scroll.
 */
export function buildShowsRows(
	rows: readonly GuiShowsRow[],
	mode: GuiShowsMode,
): ShowsListRow[] {
	return showsSeriesMode(mode) ? seriesShowsRows(rows) : flatShowsRows(rows);
}

/** The All series mode's rows: each series under the letter heading of its
 * rail letter, one heading per run of the same letter. */
function seriesShowsRows(rows: readonly GuiShowsRow[]): ShowsListRow[] {
	const built: ShowsListRow[] = [];
	let top = 0;
	let heading: string | null = null;
	for (const row of rows) {
		const letter = row.sortLetter ?? '#';
		if (letter !== heading) {
			heading = letter;
			built.push({
				kind: 'heading',
				id: `shows-letter:${letter}`,
				letter,
				top,
				height: SHOWS_HEADING_ROW_HEIGHT,
			});
			top += SHOWS_HEADING_ROW_HEIGHT;
		}
		built.push({ kind: 'series', id: row.id, row, top, height: SHOWS_SERIES_ROW_HEIGHT });
		top += SHOWS_SERIES_ROW_HEIGHT;
	}
	return built;
}

/** The Latest and Upcoming modes' rows: the flat rows at one height, with no
 * heading and no letter (design D12). */
function flatShowsRows(rows: readonly GuiShowsRow[]): ShowsListRow[] {
	return rows.map((row, at) => ({
		kind: 'flat',
		id: row.id,
		row,
		top: at * SHOWS_FLAT_ROW_HEIGHT,
		height: SHOWS_FLAT_ROW_HEIGHT,
	}));
}

/** The rows the cursor may rest on: the series and flat rows, never a heading
 * (design D12). */
function showsSelectableRows(rows: readonly ShowsListRow[]): ShowsItemRow[] {
	return rows.filter((row): row is ShowsItemRow => row.kind !== 'heading');
}

/** The rail letter a painted row belongs to, or `null` on a heading and on the
 * letterless flat rows. */
export function showsRowLetter(row: ShowsListRow): string | null {
	if (row.kind === 'heading') {
		return row.letter;
	}
	return row.row.sortLetter;
}

/**
 * The cursor key a movement leaves (design D12): the next, the previous, or a
 * page away item row, clamped to the list's ends, stepping over the letter
 * headings. With no cursor on an item row, Up and End start from the last item
 * row and every other movement from the first, so one key press always lands
 * the cursor. `page` is the rows a page spans: the rows in view.
 */
export function moveShowsCursor(
	rows: readonly ShowsListRow[],
	cursor: string | null,
	move: 'up' | 'down' | 'pageUp' | 'pageDown' | 'home' | 'end',
	page = 1,
): string | null {
	const items = showsSelectableRows(rows);
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

/** The A–Z rail over the painted rows (design D12, D7): each letter with the
 * first series row under it, so a press moves the cursor there. A letter no
 * painted row carries is disabled, as the Music rail's is. */
export function showsLetterRail(rows: readonly ShowsListRow[]): RailLetter[] {
	return [...RAIL_LETTERS].map((letter) => ({
		letter,
		rowId:
			rows.find((row) => row.kind === 'series' && row.row.sortLetter === letter)?.id ?? null,
	}));
}
