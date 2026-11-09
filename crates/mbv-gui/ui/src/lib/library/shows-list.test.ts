// add-gui-library-panel row 7.1: the Shows list's row model (design D12, the
// USER-APPROVED GUI Shows design) — the All series mode's letter headings over
// its series rows, the Latest and Upcoming modes' flat rows with their date
// gutter and no heading, the cursor's movement over the item rows only, and the
// A–Z rail. The model is plain data, so the geometry is asserted without a
// layout.

import { expect, test } from 'vitest';
import type { GuiShowsMode, GuiShowsPill, GuiShowsRow } from '$lib/bridge/types';
import {
	activeShowsMode,
	buildShowsRows,
	moveShowsCursor,
	SHOWS_FLAT_ROW_HEIGHT,
	SHOWS_HEADING_ROW_HEIGHT,
	SHOWS_SERIES_ROW_HEIGHT,
	showsLetterRail,
} from './shows-list';

/** One series row of the All series mode: its rail letter is the heading it
 * paints under. */
function series(id: string, title: string, sortLetter: string): GuiShowsRow {
	return { id, title, subtitle: null, date: null, sortLetter };
}

/** One flat row: the Latest mode's newest episode, or Upcoming's premiere. */
function flat(id: string, title: string, date: string): GuiShowsRow {
	return { id, title, subtitle: 'S3E4 · Sabrosito', date, sortLetter: null };
}

/** The pills the frame carries, with `mode` selected. */
function pills(mode: GuiShowsMode): GuiShowsPill[] {
	return [
		{ mode: 'latest', name: 'Latest', selected: mode === 'latest', collection: false },
		{ mode: 'upcoming', name: 'Upcoming', selected: mode === 'upcoming', collection: false },
		{ mode: 'all', name: 'All series', selected: mode === 'all', collection: false },
	];
}

/** The All series fixture: two `A` series, one `B` series, and one with no
 * letter, which paints under `#`. */
const SERIES: GuiShowsRow[] = [
	series('s1', 'Atlanta', 'A'),
	series('s2', 'The Bear', 'A'),
	series('s3', 'Better Call Saul', 'B'),
	series('s4', '1899', '#'),
];

const EPISODES: GuiShowsRow[] = [flat('e1', 'Atlanta', '12 May'), flat('e2', 'The Bear', '9 May')];

test('the All series mode paints a letter heading over each run of series rows', () => {
	const painted = buildShowsRows(SERIES, 'all');

	expect(painted.map((row) => row.kind)).toEqual([
		'heading',
		'series',
		'series',
		'heading',
		'series',
		'heading',
		'series',
	]);
	expect(painted.map((row) => row.height)).toEqual([
		SHOWS_HEADING_ROW_HEIGHT,
		SHOWS_SERIES_ROW_HEIGHT,
		SHOWS_SERIES_ROW_HEIGHT,
		SHOWS_HEADING_ROW_HEIGHT,
		SHOWS_SERIES_ROW_HEIGHT,
		SHOWS_HEADING_ROW_HEIGHT,
		SHOWS_SERIES_ROW_HEIGHT,
	]);
	expect(painted[0].top).toBe(0);
	expect(painted[1].top).toBe(SHOWS_HEADING_ROW_HEIGHT);
	expect(painted.map((row) => (row.kind === 'heading' ? row.letter : null))).toEqual([
		'A',
		null,
		null,
		'B',
		null,
		'#',
		null,
	]);
});

test('the flat modes paint no heading and keep each row its date', () => {
	const latest = buildShowsRows(EPISODES, 'latest');
	const upcoming = buildShowsRows([flat('p1', 'The Bear', '20 May')], 'upcoming');

	expect(latest.map((row) => row.kind)).toEqual(['flat', 'flat']);
	expect(latest.map((row) => row.height)).toEqual([
		SHOWS_FLAT_ROW_HEIGHT,
		SHOWS_FLAT_ROW_HEIGHT,
	]);
	expect(latest.map((row) => (row.kind === 'flat' ? row.row.date : null))).toEqual([
		'12 May',
		'9 May',
	]);
	expect(upcoming.map((row) => row.kind)).toEqual(['flat']);
});

test('the active mode is the pill the core marked selected', () => {
	const shows = {
		libIdx: 0,
		pills: pills('upcoming'),
		collectionNames: ['Upcoming'],
		loading: false,
		workspaceLoading: false,
		rows: EPISODES,
		selected: null,
		hero: null,
	};

	expect(activeShowsMode(shows)).toBe('upcoming');
	// A frame that marks no pill falls back to the rows' own shape, so the
	// list always has one mode.
	expect(activeShowsMode({ ...shows, pills: [] })).toBe('latest');
});

test('movement steps over the letter headings', () => {
	const painted = buildShowsRows(SERIES, 'all');

	expect(moveShowsCursor(painted, 's2', 'down')).toBe('s3');
	expect(moveShowsCursor(painted, 's3', 'down')).toBe('s4');
	expect(moveShowsCursor(painted, null, 'down')).toBe('s1');
	expect(moveShowsCursor(painted, 's1', 'end')).toBe('s4');
});

test('the rail holds the first series row under each letter and disables the rest', () => {
	const rail = showsLetterRail(buildShowsRows(SERIES, 'all'));

	expect(rail.find((letter) => letter.letter === 'A')?.rowId).toBe('s1');
	expect(rail.find((letter) => letter.letter === 'B')?.rowId).toBe('s3');
	expect(rail.find((letter) => letter.letter === 'C')?.rowId).toBeNull();
});
