// The Podcasts list's row model (add-gui-library-panel design D12, the
// USER-APPROVED GUI Podcasts design, row 10.1): the active pill's flat
// episode rows — no letter headings, because the list is sorted by date, and
// so no letter rail. The list paints fixed-height rows over native scroll and
// only the rows in view, as the Shows and Home lists do, and the model is
// plain data over fixed heights, so the geometry is asserted without a
// layout, which jsdom does not have.

import type { GuiPodcastsRow } from '$lib/bridge/types';
import type { ListWindow } from './list';
import { listWindow, rowsHeight } from './list';

/** The episode row's height in logical pixels (design D12): the room a
 * two-line episode title and the date gutter paint into, as a Shows flat
 * row. */
export const PODCASTS_ROW_HEIGHT = 48;

/** One painted row of the Podcasts list (design D12). The list is one flat
 * episode list in core order — newest first with undated episodes last — so
 * every painted row is an item row: the `id` is the row's stable identity as
 * one opaque string, the cursor's identity, the each-block's key, and the id
 * the scroll geometry reads. There is no heading and no rail to step over. */
export interface PodcastsItemRow {
	id: string;
	top: number;
	height: number;
	row: GuiPodcastsRow;
}

/** The painted rows of the Podcasts list: every row the model paints is an
 * item row. */
export type PodcastsListRow = PodcastsItemRow;

/** The rows in view over the Podcasts list (design D12): the shared list
 * window's own answer for the Podcasts row heights. */
export function podcastsListWindow(
	rows: readonly PodcastsListRow[],
	scrollTop: number,
	viewportHeight: number,
): ListWindow {
	return listWindow(rows, scrollTop, viewportHeight);
}

/** The painted height of the Podcasts list (design D12): the shared height's
 * own answer for the Podcasts row heights. */
export function podcastsRowsHeight(rows: readonly PodcastsListRow[]): number {
	return rowsHeight(rows);
}

/**
 * The painted rows of the Podcasts list (design D12): the frame's episode
 * rows at one height, with no heading and no letter, in the core's order —
 * the latest pill's merged newest-first order, or a show's own episodes.
 * Every row carries its top edge and height, so the list paints a window of
 * rows over native scroll.
 */
export function buildPodcastsRows(rows: readonly GuiPodcastsRow[]): PodcastsListRow[] {
	return rows.map((row, at) => ({
		id: row.id,
		row,
		top: at * PODCASTS_ROW_HEIGHT,
		height: PODCASTS_ROW_HEIGHT,
	}));
}

/**
 * The cursor key a movement leaves (design D12): the next, the previous, or a
 * page away episode row, clamped to the list's ends. With no cursor on a
 * row, Up and End start from the last row and every other movement from the
 * first, so one key press always lands the cursor. `page` is the rows a page
 * spans: the rows in view.
 */
export function movePodcastsCursor(
	rows: readonly PodcastsListRow[],
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
