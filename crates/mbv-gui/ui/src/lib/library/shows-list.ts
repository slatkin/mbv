// The Shows list's row model (add-gui-library-panel design D14): the Music
// tree over the TUI's TV node kinds — series → season → episode — with the
// 24 px letter headings above the first series of each letter, the rows in
// view, the cursor's movement, and the A–Z letter rail. The Latest and
// Upcoming modes keep their flat rows with the date gutter. The list paints
// fixed-height rows over native scroll and only the rows in view, as the
// Music list does, and the cursor moves over the nodes and the leaves only —
// a heading is structural, the cursor never rests on it. The model is plain
// data over fixed heights, so the geometry is asserted without a layout,
// which jsdom does not have.

import type {
	GuiShowsEpisodeRow,
	GuiShowsFrame,
	GuiShowsMode,
	GuiShowsRow,
	GuiShowsSeries,
} from '$lib/bridge/types';
import type { RailLetter } from './list';
import {
	LETTER_ROW_HEIGHT,
	NODE_ROW_HEIGHT,
	RAIL_LETTERS,
	TRACK_ROW_HEIGHT,
	filterMatch,
	moveCursor,
} from './list';
import type { ListMove } from './list';

/** The letter heading row's height in logical pixels: the Music list's letter
 * heading, which the All series list reuses. */
export const SHOWS_HEADING_ROW_HEIGHT = LETTER_ROW_HEIGHT;
/** The series and season row height in logical pixels: the Music list's node
 * row. */
export const SHOWS_SERIES_ROW_HEIGHT = NODE_ROW_HEIGHT;
export const SHOWS_SEASON_ROW_HEIGHT = NODE_ROW_HEIGHT;
/** The episode leaf's height in logical pixels: the Music track row. */
export const SHOWS_EPISODE_ROW_HEIGHT = TRACK_ROW_HEIGHT;
/** The flat episode or premiere row's height in logical pixels: the room a
 * two-line title and the date gutter paint into, as a Home item row. */
export const SHOWS_FLAT_ROW_HEIGHT = 48;

/** The tree's open and closed node keys (design D14): the Music tree's rule.
 * `open` holds the nodes the user opened, and `closed` holds the nodes the
 * user closed while the filter was narrowing the rows, which override the
 * filter's auto-open. */
export interface ShowsTreeState {
	open: readonly string[];
	closed: readonly string[];
}

/** A painted heading row (structural: the cursor never rests on it). */
interface ShowsHeadingRow {
	kind: 'heading';
	id: string;
	top: number;
	height: number;
	letter: string;
}

/** A painted series node (design D14): the Music artist row's counterpart. */
export interface ShowsSeriesRow {
	kind: 'series';
	id: string;
	top: number;
	height: number;
	/** The series' stable Service id: the select command names it. */
	sourceId: string;
	title: string;
	letter: string;
	open: boolean;
	expandable: boolean;
}

/** A painted season node (design D14): the Music album row's counterpart. */
export interface ShowsSeasonRow {
	kind: 'season';
	id: string;
	top: number;
	height: number;
	/** The season's stable Service id: the select-season command names it. */
	sourceId: string;
	/** The series the season belongs to, so an open asks for the pair. */
	seriesId: string;
	title: string;
	letter: string;
	open: boolean;
	expandable: boolean;
	/** Whether the season's episode load has landed: an open season without
	 * one asks the core for them. */
	loaded: boolean;
}

/** A painted episode leaf (design D14): the Music track row's counterpart. */
export interface ShowsEpisodeRow {
	kind: 'episode';
	id: string;
	top: number;
	height: number;
	/** The episode's stable Service id: a play command names it. */
	sourceId: string;
	seriesId: string;
	title: string;
	number: number;
	duration: string;
	letter: string;
}

/** A painted flat row of the Latest and Upcoming modes (design D12, kept):
 * an episode or a premiere notice. */
export interface ShowsFlatRow {
	kind: 'flat';
	id: string;
	top: number;
	height: number;
	row: GuiShowsRow;
}

/** One painted row of the Shows list (design D14). The `id` is the row's
 * stable key: the cursor's identity and the each-block's key. */
export type ShowsListRow = ShowsHeadingRow | ShowsSeriesRow | ShowsSeasonRow | ShowsEpisodeRow | ShowsFlatRow;

/** A painted row the cursor may rest on and the keys act on: every row but a
 * heading. */
export type ShowsItemRow = Exclude<ShowsListRow, ShowsHeadingRow>;

/** A painted node: the rows the open and close acts apply to. */
export type ShowsNodeRow = ShowsSeriesRow | ShowsSeasonRow;

/** The row key of a series node. */
export function seriesRowId(seriesId: string): string {
	return `shows-series:${seriesId}`;
}

/** The row key of a season node: the series is part of the key, so two
 * series that share a season id cannot collide. */
export function seasonRowId(seriesId: string, seasonId: string): string {
	return `shows-season:${seriesId}:${seasonId}`;
}

/** The row key of an episode leaf. */
export function episodeRowId(episodeId: string): string {
	return `shows-episode:${episodeId}`;
}

/** The active view mode: the pill the core marked selected. A frame that
 * marks none falls back to the rows' own shape — the tree-carrying frame is
 * the All series list — so the list always has one mode. */
export function activeShowsMode(shows: GuiShowsFrame): GuiShowsMode {
	const selected = shows.pills.find((pill) => pill.selected);
	if (selected !== undefined) {
		return selected.mode;
	}
	return shows.series.length > 0 || shows.rows.some((row) => row.sortLetter !== null)
		? 'all'
		: 'latest';
}

/** Whether the mode paints the series tree rather than a flat episode or
 * premiere list. */
export function showsSeriesMode(mode: GuiShowsMode): boolean {
	return mode === 'all';
}

/**
 * The painted rows of the Shows list (design D14): in the All series mode the
 * Music tree over the TUI's node kinds — each series node under the letter
 * heading of its rail letter, its seasons when it is open, and each season's
 * episodes when that season is open — and in the Latest and Upcoming modes the
 * flat rows with no heading at all.
 *
 * The filter narrows the rows to the titles that contain its text, without
 * case: a row paints when its own title matches, and a node whose descendants
 * match stays so a match is never orphaned. Under a filter a node paints open
 * unless the user closed it (the Music tree's `closed` rule). Every row
 * carries its top edge and height, so the list paints a window of rows over
 * native scroll.
 */
export function buildShowsRows(
	shows: GuiShowsFrame,
	mode: GuiShowsMode,
	filter = '',
	tree: ShowsTreeState = { open: [], closed: [] },
): ShowsListRow[] {
	return showsSeriesMode(mode)
		? seriesShowsRows(shows.series, filter, tree)
		: flatShowsRows(shows.rows.filter((row) => filterMatch(row.title, filter)));
}

/** A painted row without its geometry: the shape the row builder fills before
 * the running top edge is known. */
type ShowsRowContent = ShowsListRow extends infer Row
	? Row extends ShowsListRow
		? Omit<Row, 'top' | 'height'>
		: never
	: never;

/** The All series mode's tree rows. */
function seriesShowsRows(
	series: readonly GuiShowsSeries[],
	filter: string,
	tree: ShowsTreeState,
): ShowsListRow[] {
	const needle = filter.trim().toLowerCase();
	const narrowing = needle !== '';
	const matches = (title: string): boolean =>
		!narrowing || title.toLowerCase().includes(needle);
	const rows: ShowsListRow[] = [];
	let top = 0;
	let heading: string | null = null;

	function push<Row extends ShowsRowContent>(row: Row, height: number): void {
		rows.push({ ...row, top, height } as ShowsListRow);
		top += height;
	}

	for (const node of series) {
		const letter = node.sortLetter === '' ? '#' : node.sortLetter;
		const nodeId = seriesRowId(node.id);
		const seasons = node.seasons.map((season) => {
			const episodes = season.episodes ?? [];
			const keptEpisodes = episodes.filter((episode) => matches(episode.name));
			return {
				season,
				keptEpisodes,
				kept: matches(season.name) || keptEpisodes.length > 0,
			};
		});
		const keptSeasons = seasons.filter((candidate) => candidate.kept);
		if (!matches(node.title) && keptSeasons.length === 0) {
			continue;
		}
		if (letter !== heading) {
			heading = letter;
			push({ kind: 'heading', id: `shows-letter:${letter}`, letter }, SHOWS_HEADING_ROW_HEIGHT);
		}
		const seriesOpen = narrowing
			? keptSeasons.length > 0 && !tree.closed.includes(nodeId)
			: tree.open.includes(nodeId);
		push(
			{
				kind: 'series',
				id: nodeId,
				sourceId: node.id,
				title: node.title,
				letter,
				open: seriesOpen,
				expandable: node.expandable,
			},
			SHOWS_SERIES_ROW_HEIGHT,
		);
		if (!seriesOpen) {
			continue;
		}
		for (const { season, keptEpisodes, kept } of keptSeasons) {
			if (!kept) {
				continue;
			}
			const seasonId = seasonRowId(node.id, season.id);
			const open = narrowing
				? keptEpisodes.length > 0 && !tree.closed.includes(seasonId)
				: tree.open.includes(seasonId);
			push(
				{
					kind: 'season',
					id: seasonId,
					sourceId: season.id,
					seriesId: node.id,
					title: season.name,
					letter,
					open,
					expandable: season.episodes === null || keptEpisodes.length > 0,
					loaded: season.episodes !== null,
				},
				SHOWS_SEASON_ROW_HEIGHT,
			);
			if (!open) {
				continue;
			}
			for (const episode of keptEpisodes) {
				push(episodeLeaf(node, episode, letter), SHOWS_EPISODE_ROW_HEIGHT);
			}
		}
	}
	return rows;
}

/** One episode leaf of a season (design D14). */
function episodeLeaf(
	series: GuiShowsSeries,
	episode: GuiShowsEpisodeRow,
	letter: string,
): ShowsRowContent {
	return {
		kind: 'episode',
		id: episodeRowId(episode.episodeId),
		sourceId: episode.episodeId,
		seriesId: series.id,
		title: episode.name,
		number: episode.number,
		duration: episode.duration,
		letter,
	};
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

/** The rows the cursor may rest on: every row but a heading. */
export function showsSelectableRows(rows: readonly ShowsListRow[]): ShowsItemRow[] {
	return rows.filter((row): row is ShowsItemRow => row.kind !== 'heading');
}

/** Whether a row is a node the open and close acts apply to. */
export function isShowsNodeRow(row: ShowsItemRow | null): row is ShowsNodeRow {
	return row !== null && (row.kind === 'series' || row.kind === 'season');
}

/** The rail letter a painted row belongs to, or `null` on a heading and on
 * the letterless flat rows. */
export function showsRowLetter(row: ShowsListRow): string | null {
	switch (row.kind) {
		case 'heading':
			return row.letter;
		case 'flat':
			return row.row.sortLetter;
		default:
			return row.letter;
	}
}

/**
 * The cursor key a movement leaves (design D14): the next, the previous, or a
 * page away item row, clamped to the list's ends, stepping over the letter
 * headings. With no cursor on an item row, Up and End start from the last item
 * row and every other movement from the first, so one key press always lands
 * the cursor. `page` is the rows a page spans: the rows in view.
 */
export function moveShowsCursor(
	rows: readonly ShowsListRow[],
	cursor: string | null,
	move: ListMove,
	page = 1,
): string | null {
	return moveCursor(showsSelectableRows(rows), cursor, move, page);
}

/** The A–Z rail over the painted rows (design D7, D14): each letter with the
 * first series row under it, so a press moves the cursor there. A letter no
 * painted row carries is disabled, as the Music rail's is. */
export function showsLetterRail(rows: readonly ShowsListRow[]): RailLetter[] {
	return [...RAIL_LETTERS].map((letter) => ({
		letter,
		rowId:
			rows.find((row) => row.kind === 'series' && row.letter === letter)?.id ?? null,
	}));
}
