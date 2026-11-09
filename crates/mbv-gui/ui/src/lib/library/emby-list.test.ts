// add-gui-library-panel row 8.1: the plain Emby-library list's row model (design
// D12) — the plain mode's letter headings over its name-sorted rows with the
// A–Z rail, and the feed-group mode's flat date-ordered rows with no heading and
// no rail. The model is plain data, so the geometry is asserted without a
// layout.

import { expect, test } from 'vitest';
import type { GuiEmbyRow } from '$lib/bridge/types';
import {
	buildEmbyRows,
	EMBY_HEADING_ROW_HEIGHT,
	EMBY_ROW_HEIGHT,
	embyGroupKey,
	embyLetterRail,
	moveEmbyCursor,
} from './emby-list';

/** One plain-mode row: the name-sorted library's row under its rail letter. */
function plain(id: string, title: string, sortLetter: string): GuiEmbyRow {
	return {
		id,
		title,
		subtitle: '1999',
		date: null,
		sortLetter,
		kind: 'media',
	};
}

/** One feed-group row: the date-ordered list's row with its year in the
 * gutter. */
function feed(id: string, title: string, date: string, kind: 'collection' | 'media' = 'media') {
	return { id, title, subtitle: null, date, sortLetter: null, kind } satisfies GuiEmbyRow;
}

/** The plain fixture: two `A` movies, one `B` movie, and one with no letter,
 * which paints under `#`. */
const MOVIES: GuiEmbyRow[] = [
	plain('m1', 'Alien', 'A'),
	plain('m2', 'Amélie', 'A'),
	plain('m3', 'Brazil', 'B'),
	plain('m4', '2001', '#'),
];

test('the plain mode paints a letter heading over each run of rows', () => {
	const painted = buildEmbyRows(MOVIES, false);

	expect(painted.map((row) => row.kind)).toEqual([
		'heading',
		'item',
		'item',
		'heading',
		'item',
		'heading',
		'item',
	]);
	expect(painted[0].top).toBe(0);
	expect(painted[1].top).toBe(EMBY_HEADING_ROW_HEIGHT);
	expect(painted.map((row) => row.height)).toContain(EMBY_ROW_HEIGHT);
});

test('the feed-group mode paints flat rows with their date and no heading', () => {
	const painted = buildEmbyRows([feed('f1', 'Family', '1998'), feed('f2', 'Fargo', '1996')], true);

	expect(painted.map((row) => row.kind)).toEqual(['item', 'item']);
	expect(painted.map((row) => (row.kind === 'item' ? row.row.date : null))).toEqual([
		'1998',
		'1996',
	]);
	expect(painted[1].top).toBe(EMBY_ROW_HEIGHT);
});

test('movement steps over the letter headings and every row is selectable', () => {
	const painted = buildEmbyRows(MOVIES, false);

	expect(moveEmbyCursor(painted, 'm2', 'down')).toBe('m3');
	expect(moveEmbyCursor(painted, null, 'down')).toBe('m1');
	expect(moveEmbyCursor(painted, 'm1', 'end')).toBe('m4');
});

test('the rail holds the first row under each letter and disables the rest', () => {
	const rail = embyLetterRail(buildEmbyRows(MOVIES, false));

	expect(rail.find((letter) => letter.letter === 'A')?.rowId).toBe('m1');
	expect(rail.find((letter) => letter.letter === 'B')?.rowId).toBe('m3');
	expect(rail.find((letter) => letter.letter === 'C')?.rowId).toBeNull();
});

test('the feed-group rows carry no letter, so every rail letter is disabled', () => {
	const rail = embyLetterRail(buildEmbyRows([feed('f1', 'Family', '1998')], true));

	expect(rail.every((letter) => letter.rowId === null)).toBe(true);
});

test('a pill keys on its group, never on a row position', () => {
	expect(embyGroupKey({ kind: 'all' })).toBe('all');
	expect(embyGroupKey({ kind: 'folder', id: 'folder-1' })).toBe('folder:folder-1');
});
