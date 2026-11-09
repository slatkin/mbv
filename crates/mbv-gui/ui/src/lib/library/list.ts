// The Library list's row model (add-gui-library-panel design D7): the canvas
// tree of artists, albums, and tracks, the 24 px letter headings above the
// first artist of each letter, the rows in view, the cursor's movement, and
// the A–Z letter rail. The list paints fixed-height rows over native scroll,
// and the window content owns the cursor, the open nodes, the scroll, and the
// filter (design D1), so this module is plain data: `state.ts` applies it and
// the components paint it. Keeping it apart from both keeps the geometry
// testable without a layout, which jsdom does not have.

import type { GuiMusicArtist } from '$lib/bridge/types';

/** The canvas's letter heading height in logical pixels (design D7). */
export const LETTER_ROW_HEIGHT = 24;
/** The canvas's artist and album row height in logical pixels. */
export const NODE_ROW_HEIGHT = 30;
/** The canvas's track row height in logical pixels. */
export const TRACK_ROW_HEIGHT = 28;

/** The letters the rail spans (design D7): A to Z, as the canvas paints it. A
 * `#` heading for a Service's non-alphabetic names stays off the rail. The
 * Shows list's rail spans the same letters (design D12). */
export const RAIL_LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';

/** The fields every painted row carries: its stable id (the cursor's identity
 * and the each-block's key), its top edge from the list's first row, its
 * painted height, and the sort letter of the artist it belongs to — the rail
 * marks the letter of the row under the cursor. */
interface RowBase {
	id: string;
	top: number;
	height: number;
	letter: string;
}

/** One painted row of the canvas tree (design D7). A letter heading is
 * structural: the cursor never rests on it. */
export type LibraryRow =
	| (RowBase & { kind: 'letter' })
	| (RowBase & {
			kind: 'artist';
			name: string;
			/** The Service id the detail command names, or `null` for an artist
			 * the Service does not resolve. */
			itemId: string | null;
			open: boolean;
		})
	| (RowBase & {
			kind: 'album';
			albumId: string;
			name: string;
			/** The release year, empty when the Service holds none. */
			year: string;
			open: boolean;
			/** Whether the carried content holds the album's tracklist: an album
			 * without one asks the core to load it when it opens (design D5). */
			tracksLoaded: boolean;
		})
	| (RowBase & {
			kind: 'track';
			/** The album the track belongs to: a picked track picks its album
			 * (design D5). */
			albumId: string;
			trackId: string;
			name: string;
			/** The number the row paints. */
			number: number;
			/** The runtime, `M:SS` or `H:MM:SS`; empty when unknown. */
			duration: string;
		});

/** The fields the scroll and window geometry read (add-gui-library-panel
 * design D7, D12): the stable id, the top edge from the list's first row, and
 * the painted height. The Library tree's rows and the Home list's rows both
 * satisfy it, so one window implementation serves both lists. */
export interface GeometryRow {
	readonly id: string;
	readonly top: number;
	readonly height: number;
}

/** A row the cursor rests on and the tree's open and close acts on: the
 * letter headings and the track rows are neither. */
export type LibraryNodeRow = Extract<LibraryRow, { kind: 'artist' } | { kind: 'album' }>;

/** An artist row's id: the artist's stable identity, prefixed so an artist and
 * an album of the same Service id cannot collide. */
export function artistRowId(artist: GuiMusicArtist): string {
	return `artist:${artist.artistKey.kind}:${artist.artistKey.id}`;
}

/** An album row's id. */
export function albumRowId(albumId: string): string {
	return `album:${albumId}`;
}

/** A track row's id. */
export function trackRowId(trackId: string): string {
	return `track:${trackId}`;
}

/** The input the row model builds from: the carried union, the open nodes, and
 * the filter text. */
export interface LibraryTree {
	artists: readonly GuiMusicArtist[];
	/** The open node ids (artist and album row ids). */
	open: readonly string[];
	/** The node ids the user closed while the filter was narrowing the rows:
	 * they override the filter's auto-open, so a press still closes a node
	 * under a filter (design D7's open and close). */
	closed: readonly string[];
	/** The filter text: the list paints only the rows whose name contains it,
	 * without case (design D7). */
	filter: string;
}

/**
 * The painted rows of the canvas tree (design D7): the artists of the union in
 * their carried order, each with its albums when the artist is open and its
 * tracks when the album is open, under the letter heading of the first artist
 * of each sort letter. Every row carries its top edge and height, so the list
 * can paint a window of rows over native scroll and the rail can scroll to a
 * letter.
 *
 * The filter narrows the rows to the names that contain the text, without
 * case: a row paints when its own name matches. An artist or album whose rows
 * all drop out stays when one of its descendants matches, so a match is never
 * orphaned, and a node with a matching child paints open so no match hides —
 * unless the user closed that node under the filter, which the closed set
 * keeps shut. Outside a filter, a node paints open when the window content has
 * it open, even while an album's tracklist is still loading.
 */
export function buildLibraryRows(tree: LibraryTree): LibraryRow[] {
	const needle = tree.filter.trim().toLowerCase();
	const narrowing = needle !== '';
	/** Whether a name contains the filter's text; an empty filter matches
	 * every name. */
	const matches = (name: string): boolean =>
		!narrowing || name.toLowerCase().includes(needle);
	const rows: LibraryRow[] = [];
	let top = 0;
	let heading: string | null = null;

	function push<Row extends Omit<LibraryRow, 'top' | 'height'>>(
		row: Row,
		height: number,
	): void {
		rows.push({ ...row, top, height } as LibraryRow);
		top += height;
	}

	for (const artist of tree.artists) {
		const artistId = artistRowId(artist);
		const letter = artist.sortLetter;
		const albums = artist.albums.map((album) => {
			const tracks = (album.tracks ?? []).filter((track) => matches(track.name));
			return { album, tracks, kept: matches(album.name) || tracks.length > 0 };
		});
		const keptAlbums = albums.filter((candidate) => candidate.kept);
		if (!matches(artist.name) && keptAlbums.length === 0) {
			continue;
		}
		if (letter !== heading) {
			heading = letter;
			push({ id: `letter:${letter}`, kind: 'letter', letter }, LETTER_ROW_HEIGHT);
		}
		const artistOpen = narrowing
			? keptAlbums.length > 0 && !tree.closed.includes(artistId)
			: tree.open.includes(artistId);
		push(
			{
				id: artistId,
				kind: 'artist',
				letter,
				name: artist.name,
				itemId: artist.artistKey.kind === 'service' ? artist.artistKey.id : null,
				open: artistOpen,
			},
			NODE_ROW_HEIGHT,
		);
		if (!artistOpen) {
			continue;
		}
		for (const { album, tracks, kept } of keptAlbums) {
			if (!kept) {
				continue;
			}
			const albumId = albumRowId(album.albumId);
			const albumOpen = narrowing
				? tracks.length > 0 && !tree.closed.includes(albumId)
				: tree.open.includes(albumId);
			push(
				{
					id: albumId,
					kind: 'album',
					letter,
					albumId: album.albumId,
					name: album.name,
					year: album.year,
					open: albumOpen,
					tracksLoaded: album.tracks !== null,
				},
				NODE_ROW_HEIGHT,
			);
			if (!albumOpen) {
				continue;
			}
			for (const track of tracks) {
				push(
					{
						id: trackRowId(track.trackId),
						kind: 'track',
						letter,
						albumId: album.albumId,
						trackId: track.trackId,
						name: track.name,
						number: track.number,
						duration: track.duration,
					},
					TRACK_ROW_HEIGHT,
				);
			}
		}
	}
	return rows;
}

/** The rows the cursor may rest on: every row but the letter headings. */
export function selectableRows(rows: readonly LibraryRow[]): LibraryRow[] {
	return rows.filter((row) => row.kind !== 'letter');
}

/** The painted height of the whole list: the bottom edge of its last row. */
export function rowsHeight(rows: readonly GeometryRow[]): number {
	const last = rows[rows.length - 1];
	return last === undefined ? 0 : last.top + last.height;
}

/** A cursor movement the design's keys name (design D3): row by row for Up and
 * Down, one viewport for Page Up and Page Down, and the list's ends for Home
 * and End. */
export type ListMove = 'up' | 'down' | 'pageUp' | 'pageDown' | 'home' | 'end';

/**
 * The cursor row a movement leaves (design D7): the next, the previous, or a
 * page away selectable row, clamped to the list's ends. With no cursor on a
 * selectable row, Up and End start from the last row and every other movement
 * from the first, so one key press always lands the cursor. `page` is the rows
 * a page spans: the rows in view.
 */
export function moveListCursor(
	rows: readonly LibraryRow[],
	cursor: string | null,
	move: ListMove,
	page = 1,
): string | null {
	const nodes = selectableRows(rows);
	if (nodes.length === 0) {
		return null;
	}
	const last = nodes.length - 1;
	const at = nodes.findIndex((row) => row.id === cursor);
	if (at < 0) {
		return (move === 'up' || move === 'end' ? nodes[last] : nodes[0]).id;
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
	return nodes[Math.min(Math.max(to, 0), last)].id;
}

/** The half-open range of row indexes the viewport paints (design D7): the rows
 * that intersect `[scrollTop, scrollTop + viewportHeight)`. An unmeasured
 * viewport paints nothing, so a long list never paints in full while it waits
 * for its first measurement. */
export interface ListWindow {
	start: number;
	/** Exclusive. */
	end: number;
}

/** The rows in view: only these paint, so a long list stays fast. */
export function listWindow(
	rows: readonly GeometryRow[],
	scrollTop: number,
	viewportHeight: number,
): ListWindow {
	if (viewportHeight <= 0) {
		return { start: 0, end: 0 };
	}
	const top = Math.max(scrollTop, 0);
	const bottom = top + viewportHeight;
	const start = rows.findIndex((row) => row.top + row.height > top);
	if (start < 0) {
		return { start: rows.length, end: rows.length };
	}
	let end = start;
	while (end < rows.length && rows[end].top < bottom) {
		end += 1;
	}
	return { start, end };
}

/** A scroll offset held inside the list's scrollable range. */
export function clampScrollTop(
	rows: readonly GeometryRow[],
	scrollTop: number,
	viewportHeight: number,
): number {
	const max = Math.max(rowsHeight(rows) - Math.max(viewportHeight, 0), 0);
	return Math.min(Math.max(scrollTop, 0), max);
}

/**
 * The scroll offset that keeps the cursor row fully visible (design D7, the
 * spec's "the list scrolls so that the cursor row is fully visible"): the row's
 * top when it sits above the viewport, its bottom edge when it sits below, and
 * an unchanged, clamped offset while it already shows. A cursor with no row
 * moves nothing.
 */
export function scrollTopToShowCursor(
	rows: readonly GeometryRow[],
	cursor: string | null,
	scrollTop: number,
	viewportHeight: number,
): number {
	const row = rows.find((candidate) => candidate.id === cursor);
	const top = Math.max(scrollTop, 0);
	if (row === undefined || viewportHeight <= 0) {
		return clampScrollTop(rows, top, viewportHeight);
	}
	if (row.top < top) {
		return clampScrollTop(rows, row.top, viewportHeight);
	}
	if (row.top + row.height > top + viewportHeight) {
		return clampScrollTop(rows, row.top + row.height - viewportHeight, viewportHeight);
	}
	return clampScrollTop(rows, top, viewportHeight);
}

/** One letter of the rail (design D7): the letter and the first artist row
 * under it, or `null` while no painted row carries the letter — a disabled
 * letter. */
export interface RailLetter {
	letter: string;
	rowId: string | null;
}

/** The A–Z rail over the painted rows (design D7): each letter with the first
 * artist row under it, so a press scrolls to that row and moves the cursor
 * there. A letter no visible row carries is disabled. */
export function letterRail(rows: readonly LibraryRow[]): RailLetter[] {
	return [...RAIL_LETTERS].map((letter) => ({
		letter,
		rowId:
			rows.find((row) => row.kind === 'artist' && row.letter === letter)?.id ?? null,
	}));
}
