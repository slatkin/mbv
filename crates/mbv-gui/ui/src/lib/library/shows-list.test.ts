// add-gui-library-panel design D14: the Shows list's row model — the Music
// tree over the TUI's TV node kinds (series → season → episode), the letter
// headings over the first series of each letter, the Latest and Upcoming
// modes' flat rows with their date gutter and no heading, the cursor's
// movement over the nodes and leaves only, and the A–Z rail. The model is
// plain data, so the geometry is asserted without a layout.

import { expect, test } from 'vitest';
import type {
	GuiShowsEpisodeRow,
	GuiShowsFrame,
	GuiShowsMode,
	GuiShowsPill,
	GuiShowsRow,
	GuiShowsSeries,
} from '$lib/bridge/types';
import {
	activeShowsMode,
	buildShowsRows,
	episodeRowId,
	moveShowsCursor,
	seasonRowId,
	seriesRowId,
	SHOWS_EPISODE_ROW_HEIGHT,
	SHOWS_FLAT_ROW_HEIGHT,
	SHOWS_HEADING_ROW_HEIGHT,
	SHOWS_SEASON_ROW_HEIGHT,
	SHOWS_SERIES_ROW_HEIGHT,
	showsLetterRail,
} from './shows-list';

/** One episode row of a season, as the frame carries it. */
function episode(id: string, name: string): GuiShowsEpisodeRow {
	return { episodeId: id, number: 1, name, duration: '24:00', state: 'ordinary' };
}

/** One series node: its rail letter is the heading it paints under. */
function series(
	id: string,
	title: string,
	sortLetter: string,
	seasons: GuiShowsSeries['seasons'] = [],
	expandable = true,
): GuiShowsSeries {
	return { id, title, sortLetter, expandable, seasons };
}

/** One flat row: the Latest mode's newest episode, or Upcoming's premiere. */
function flat(id: string, title: string, date: string): GuiShowsRow {
	return { id, title, subtitle: 'S3E4 · Sabrosito', date, sortLetter: null };
}

/** The pills the frame carries, with `mode` selected. */
function pills(mode: GuiShowsMode): GuiShowsPill[] {
	return [
		{ mode: 'latest', name: 'New', selected: mode === 'latest', collection: false },
		{ mode: 'upcoming', name: 'Upcoming', selected: mode === 'upcoming', collection: false },
		{ mode: 'all', name: 'All', selected: mode === 'all', collection: false },
	];
}

/** The All series fixture: two `A` series (the first with a season and its
 * episodes), one `B` series, and one with no letter, which paints under `#`. */
const TREE: GuiShowsSeries[] = [
	series('s1', 'Atlanta', 'A', [
		{ id: 'se1', name: 'Season 1', episodes: [episode('ep1', 'The Big Bang')] },
		{ id: 'se2', name: 'Season 2', episodes: null },
	]),
	series('s2', 'The Bear', 'A'),
	series('s3', 'Better Call Saul', 'B'),
	series('s4', '1899', '#'),
];

const EPISODES: GuiShowsRow[] = [flat('e1', 'Atlanta', '12 May'), flat('e2', 'The Bear', '9 May')];

/** A frame over `tree` and `rows`, in `mode`. */
function showsFrame(
	mode: GuiShowsMode,
	tree: GuiShowsSeries[] = TREE,
	rows: GuiShowsRow[] = EPISODES,
): GuiShowsFrame {
	return {
		libIdx: 0,
		pills: pills(mode),
		collectionNames: [mode],
		loading: false,
		workspaceLoading: false,
		series: tree,
		upcomingFailed: false,
		rows,
		selected: null,
		hero: null,
	};
}

test('the All series mode paints the tree at the Music heights', () => {
	const painted = buildShowsRows(showsFrame('all'), 'all', '', {
		open: [seriesRowId('s1'), seasonRowId('s1', 'se1')],
		closed: [],
	});

	expect(painted.map((row) => row.kind)).toEqual([
		'heading',
		'series',
		'season',
		'episode',
		'season',
		'series',
		'heading',
		'series',
		'heading',
		'series',
	]);
	expect(painted.map((row) => row.height)).toEqual([
		SHOWS_HEADING_ROW_HEIGHT,
		SHOWS_SERIES_ROW_HEIGHT,
		SHOWS_SEASON_ROW_HEIGHT,
		SHOWS_EPISODE_ROW_HEIGHT,
		SHOWS_SEASON_ROW_HEIGHT,
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
		null,
		null,
		null,
		'B',
		null,
		'#',
		null,
	]);
});

test('a closed series paints no season and a closed season no episode', () => {
	const closed = buildShowsRows(showsFrame('all'), 'all', '', {
		open: [],
		closed: [],
	});

	expect(closed.map((row) => row.kind)).toEqual([
		'heading',
		'series',
		'series',
		'heading',
		'series',
		'heading',
		'series',
	]);
});

test('the flat modes paint no heading and keep each row its date', () => {
	const latest = buildShowsRows(showsFrame('latest'), 'latest');
	const upcoming = buildShowsRows(
		showsFrame('upcoming', [], [flat('p1', 'The Bear', '20 May')]),
		'upcoming',
	);

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

test('the filter keeps a series whose season matches and opens it', () => {
	const painted = buildShowsRows(showsFrame('all'), 'all', 'season 2', {
		open: [],
		closed: [],
	});

	expect(painted.map((row) => row.kind)).toEqual(['heading', 'series', 'season']);
	expect(painted[2].kind === 'season' ? painted[2].title : null).toBe('Season 2');
});

test('the active mode is the pill the core marked selected', () => {
	const shows = showsFrame('upcoming', []);

	expect(activeShowsMode(shows)).toBe('upcoming');
	// A frame that marks no pill falls back to the rows' own shape, so the
	// list always has one mode.
	expect(activeShowsMode({ ...shows, pills: [] })).toBe('latest');
	expect(activeShowsMode({ ...showsFrame('all'), pills: [] })).toBe('all');
});

test('movement steps over the letter headings', () => {
	const painted = buildShowsRows(showsFrame('all'), 'all', '', {
		open: [seriesRowId('s1')],
		closed: [],
	});

	expect(moveShowsCursor(painted, seriesRowId('s1'), 'down')).toBe(
		seasonRowId('s1', 'se1'),
	);
	expect(moveShowsCursor(painted, seasonRowId('s1', 'se2'), 'down')).toBe(
		seriesRowId('s2'),
	);
	expect(moveShowsCursor(painted, null, 'down')).toBe(seriesRowId('s1'));
	expect(moveShowsCursor(painted, seriesRowId('s1'), 'end')).toBe(seriesRowId('s4'));
});

test('the rail holds the first series row under each letter and disables the rest', () => {
	const rail = showsLetterRail(buildShowsRows(showsFrame('all'), 'all'));

	expect(rail.find((letter) => letter.letter === 'A')?.rowId).toBe(seriesRowId('s1'));
	expect(rail.find((letter) => letter.letter === 'B')?.rowId).toBe(seriesRowId('s3'));
	expect(rail.find((letter) => letter.letter === 'C')?.rowId).toBeNull();
	expect(episodeRowId('ep1')).toBe('shows-episode:ep1');
});
