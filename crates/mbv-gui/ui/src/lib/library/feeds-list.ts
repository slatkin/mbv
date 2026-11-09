// The Feeds list's row model (add-gui-library-panel design D12, the
// USER-APPROVED GUI Podcasts and Feeds design, row 11.1): the active pill's
// flat entry rows — no letter headings, because the list is sorted by date,
// and so no letter rail. The list paints fixed-height rows over native scroll
// and only the rows in view, as the Shows and Home lists do, and the model is
// plain data over fixed heights, so the geometry is asserted without a
// layout, which jsdom does not have.

import type { GuiFeedsRow } from '$lib/bridge/types';
import type { ListWindow } from './list';
import { listWindow, rowsHeight } from './list';

/** The entry row's height in logical pixels (design D12): the room a
 * two-line entry title and the date gutter paint into, as a Shows flat row. */
export const FEEDS_ROW_HEIGHT = 48;

/** One painted row of the Feeds list (design D12). The list is one flat
 * entry list in core order — newest first with undated entries last — so
 * every painted row is an item row: the `id` is the row's stable identity as
 * one opaque string, the cursor's identity, the each-block's key, and the id
 * the scroll geometry reads. There is no heading and no rail to step over. */
export interface FeedsItemRow {
	id: string;
	top: number;
	height: number;
	row: GuiFeedsRow;
}

/** The painted rows of the Feeds list: every row the model paints is an item
 * row. */
export type FeedsListRow = FeedsItemRow;

/** The rows in view over the Feeds list (design D12): the shared list
 * window's own answer for the Feeds row heights. */
export function feedsListWindow(
	rows: readonly FeedsListRow[],
	scrollTop: number,
	viewportHeight: number,
): ListWindow {
	return listWindow(rows, scrollTop, viewportHeight);
}

/** The painted height of the Feeds list (design D12): the shared height's
 * own answer for the Feeds row heights. */
export function feedsRowsHeight(rows: readonly FeedsListRow[]): number {
	return rowsHeight(rows);
}

/**
 * The painted rows of the Feeds list (design D12): the frame's entry rows at
 * one height, with no heading and no letter, in the core's order — the
 * Latest pill's merged newest-first order, or a subscription's own entries.
 * Every row carries its top edge and height, so the list paints a window of
 * rows over native scroll.
 */
export function buildFeedsRows(rows: readonly GuiFeedsRow[]): FeedsListRow[] {
	return rows.map((row, at) => ({
		id: row.id,
		row,
		top: at * FEEDS_ROW_HEIGHT,
		height: FEEDS_ROW_HEIGHT,
	}));
}

/**
 * The cursor key a movement leaves (design D12): the next, the previous, or a
 * page away entry row, clamped to the list's ends. With no cursor on a row,
 * Up and End start from the last row and every other movement from the
 * first, so one key press always lands the cursor. `page` is the rows a page
 * spans: the rows in view.
 */
export function moveFeedsCursor(
	rows: readonly FeedsListRow[],
	cursor: string | null,
	move: 'up' | 'down' | 'pageUp' | 'pageDown' | 'home' | 'end',
	page = 1,
): string | null {
	if (rows.length === 0) {
		return null;
	}
	const last = rows.length - 1;
	const at = rows.findIndex((row) => row.id === cursor);
	if (at < 0) {
		return (move === 'up' || move === 'end' ? rows[last] : rows[0]).id;
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
	return rows[Math.min(Math.max(to, 0), last)].id;
}
