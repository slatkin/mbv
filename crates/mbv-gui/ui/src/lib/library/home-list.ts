// The Home list's row model (add-gui-library-panel design D12, the
// USER-APPROVED GUI Home design): the flat list's section headings, the
// spacer between two non-empty sections, and the item rows with their stable
// identity. The list paints fixed-height rows over native scroll and only
// the rows in view, as the Music list does, and the cursor moves over the
// item rows only — headings and spacers are structural, the cursor never
// rests on them. The model is plain data over fixed heights, so the geometry
// is asserted without a layout, which jsdom does not have.

import type { GuiHomeRow, GuiHomeRowId, GuiHomeRowKind, GuiHomeRowState, GuiLibraryArt } from '$lib/bridge/types';
import type { ListWindow } from './list';
import { listWindow, rowsHeight } from './list';

/** The rows in view over the flat Home list (design D12): the shared list
 * window's own answer for the Home row heights. */
export function homeListWindow(
	rows: readonly HomeListRow[],
	scrollTop: number,
	viewportHeight: number,
): ListWindow {
	return listWindow(rows, scrollTop, viewportHeight);
}

/** The painted height of the flat Home list (design D12): the shared
 * height's own answer for the Home row heights. */
export function homeRowsHeight(rows: readonly HomeListRow[]): number {
	return rowsHeight(rows);
}

/** The section heading row's height in logical pixels: the letter heading
 * height's look, scaled to the section name. */
export const HOME_HEADING_ROW_HEIGHT = 28;
/** The item row's height in logical pixels: the room a 40 px art thumb and
 * the two-line name paint into. */
export const HOME_ITEM_ROW_HEIGHT = 48;
/** The spacer row's height between two non-empty sections (design D12). */
export const HOME_SPACER_ROW_HEIGHT = 20;

/** One painted row of the flat Home list (design D12). The `id` is the row's
 * stable key: the cursor's identity, the each-block's key, and the id the
 * scroll geometry read. */
export type HomeListRow =
	| { kind: 'heading'; id: string; top: number; height: number; text: string }
	| { kind: 'spacer'; id: string; top: number; height: number }
	| {
			kind: 'item';
			id: string;
			top: number;
			height: number;
			/** The row's stable identity (design D12): the opaque
			 * `GuiHomeRowId` the core resolves, carried back by the select, play,
			 * and enqueue commands. */
			rowId: GuiHomeRowId;
			title: string;
			subtitle: string | null;
			date: string | null;
			itemKind: GuiHomeRowKind;
			state: GuiHomeRowState;
			art: GuiLibraryArt | null;
	  };

/** The row's stable key (design D12): the `GuiHomeRowId` as one string, so a
 * cursor and an each-block can carry it. A `Continue` row keys on its Emby
 * item id, a `Recent` row on its queue-item content id's provider and value. */
export function homeRowKey(id: GuiHomeRowId): string {
	if (id.kind === 'continue') {
		return `home-continue:${id.id}`;
	}
	return `home-recent:${id.id.provider}:${homeContentValue(id.id.value)}`;
}

/** The content id's value part as a key piece: a scalar value as itself, and
 * a struct value as its JSON form. */
function homeContentValue(value: string | object): string {
	return typeof value === 'string' ? value : JSON.stringify(value);
}

/**
 * The painted rows of the flat Home list (design D12): the carried rows
 * sequence — a heading only above its rows, an empty section absent, and the
 * spacer only between two non-empty sections — with each row's top edge and
 * height, so the list paints a window of rows over native scroll.
 */
export function buildHomeRows(rows: readonly GuiHomeRow[]): HomeListRow[] {
	const built: HomeListRow[] = [];
	let top = 0;

	function push<Row extends Omit<HomeListRow, 'top' | 'height'>>(row: Row, height: number): void {
		built.push({ ...row, top, height } as HomeListRow);
		top += height;
	}

	for (const row of rows) {
		if (row.row === 'heading') {
			push({ kind: 'heading', id: `home-heading:${row.text}`, text: row.text }, HOME_HEADING_ROW_HEIGHT);
		} else if (row.row === 'spacer') {
			push({ kind: 'spacer', id: `home-spacer:${top}` }, HOME_SPACER_ROW_HEIGHT);
		} else {
			push(
				{
					kind: 'item',
					id: homeRowKey(row.id),
					rowId: row.id,
					title: row.title,
					subtitle: row.subtitle,
					date: row.date,
					itemKind: row.kind,
					state: row.state,
					art: row.art,
				},
				HOME_ITEM_ROW_HEIGHT,
			);
		}
	}
	return built;
}

/** The rows the cursor may rest on: the item rows only (design D12). */
export function homeSelectableRows(rows: readonly HomeListRow[]): HomeListRow[] {
	return rows.filter((row) => row.kind === 'item');
}

/**
 * The cursor key a movement leaves (design D12): the next, the previous, or a
 * page away item row, clamped to the list's ends, stepping over the headings
 * and spacers. With no cursor on an item row, Up and End start from the last
 * item row and every other movement from the first, so one key press always
 * lands the cursor. `page` is the rows a page spans: the rows in view.
 */
export function moveHomeCursor(
	rows: readonly HomeListRow[],
	cursor: string | null,
	move: 'up' | 'down' | 'pageUp' | 'pageDown' | 'home' | 'end',
	page = 1,
): string | null {
	const items = homeSelectableRows(rows);
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
