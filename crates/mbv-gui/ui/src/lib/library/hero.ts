// The Library Hero's facts and geometry (add-gui-library-panel design D5, D8):
// the picked album's lines as the canvas prints them, the artist's albums in
// the union that Shuffle artist plays, and the tracklist's cursor. The data is
// the core's, so this module only reads the carried Hero; keeping the lines
// and the cursor movement here lets the painters stay markup and the movement
// test without a layout.
//
// A line that joins values of several data types is built as typed segments
// (add-gui-settings-screen design D14): colour carries the type, so a count
// and a time on one line each carry their own kind, and the painter paints the
// kinds through `TypedText`.

import type {
	GuiMusicFrame,
	GuiMusicHero,
	GuiMusicTrack,
	GuiMetaRow,
	GuiMetaSegment,
	GuiMusicTrackTotal,
} from '$lib/bridge/types';
import type { ListMove } from './list';

/** The tracklist rows of the Hero's album (design D5): the picked album's
 * cached tracklist, empty until the album node's open loads it. */
export function heroTracks(hero: GuiMusicHero | null): GuiMusicTrack[] {
	return hero?.album.tracks ?? [];
}

/** One line's typed runs, in display order (design D14). */
export type Segments = GuiMetaSegment[];

/** A line with no typed part: it inherits the colour the painter gives it. */
export function plainSegments(text: string): Segments {
	return text === '' ? [] : [{ text, kind: 'plain' }];
}

/** A section label's line, which paints the label token (design D14). */
export function labelSegments(text: string): Segments {
	return [{ text, kind: 'label' }];
}

/** The segments of several meta rows on one line, joined by ` · ` as the
 * frames order them. */
export function joinMetaRows(rows: readonly GuiMetaRow[]): Segments {
	return rows.filter((row) => row.length > 0).flatMap((row, index) =>
		index === 0 ? row : [{ text: ' · ', kind: 'plain' as const }, ...row],
	);
}

/** The artist header's first line, `ARTIST · HIP-HOP` as the canvas prints it:
 * the artist label with the selected collection names (design D5). */
export function heroKicker(hero: GuiMusicHero): Segments {
	return labelSegments(
		hero.collections.length > 0
			? `artist · ${hero.collections.join(' + ').toUpperCase()}`
			: 'artist',
	);
}

/** `13 tracks · 44:25`: a track count with its total time, each in the
 * singular when the count is one. The count and the time carry their own type
 * (design D14). */
export function trackTotalFacts(total: GuiMusicTrackTotal): Segments {
	return [
		{ text: String(total.count), kind: 'count' },
		{ text: total.count === 1 ? ' track · ' : ' tracks · ', kind: 'plain' },
		{ text: total.duration, kind: 'time' },
	];
}

/** The artist header's third line (design D5): the artist's album count in the
 * union, and the track count and total time once every one of those albums
 * holds a cached tracklist. */
export function artistFacts(hero: GuiMusicHero): Segments {
	const albums: Segments = [
		{ text: String(hero.albumCount), kind: 'count' },
		{ text: hero.albumCount === 1 ? ' album' : ' albums', kind: 'plain' },
	];
	return hero.artistTracks === null
		? albums
		: [...albums, { text: ' · ', kind: 'plain' }, ...trackTotalFacts(hero.artistTracks)];
}

/** The album block's facts line, empty while the album's tracklist has not
 * landed (design D5). */
export function albumFacts(hero: GuiMusicHero): Segments {
	return hero.album.total === null ? [] : trackTotalFacts(hero.album.total);
}

/** The artist's albums in the union, which Shuffle artist plays (design D8).
 * The union artist is the one that owns the picked album, so a name the
 * Service repeats cannot pick the wrong artist; the picked album alone is the
 * fallback while the union holds no owner. */
export function artistUnionAlbums(music: GuiMusicFrame, hero: GuiMusicHero): string[] {
	const owner = music.artists.find((artist) =>
		artist.albums.some((album) => album.albumId === hero.album.albumId),
	);
	return (owner?.albums ?? [hero.album]).map((album) => album.albumId);
}

/** The tracklist's cursor resolved against the tracks that paint (design D3):
 * the stored track while the tracklist holds it, and the first track while it
 * does not — a tracklist whose cache lands after the Hero pane took focus still
 * paints a cursor, and Enter acts on it without waiting for a movement key. */
export function resolveTrackCursor(
	tracks: readonly GuiMusicTrack[],
	cursor: string | null,
): string | null {
	if (tracks.some((track) => track.trackId === cursor)) {
		return cursor;
	}
	return tracks[0]?.trackId ?? null;
}

/** The rows a Page Up or Page Down spans in the tracklist: the tracklist is a
 * short list inside the Hero, so a fixed page keeps the cursor a screenful
 * away without measuring the Hero. */
const TRACK_PAGE = 10;

/** The tracklist's cursor row after one movement (design D3): row by row for
 * Up and Down, a page for Page Up and Page Down, and the tracklist's ends for
 * Home and End, clamped to the tracklist. The caller resolves the cursor
 * against the painted tracks first (`resolveTrackCursor`), so a cursor the
 * tracklist does not hold starts the movement from the first row. */
export function moveTrackCursor(
	tracks: readonly GuiMusicTrack[],
	cursor: string | null,
	move: ListMove,
): string | null {
	if (tracks.length === 0) {
		return null;
	}
	const ids = tracks.map((track) => track.trackId);
	const last = ids.length - 1;
	const at = Math.max(ids.indexOf(cursor ?? ''), 0);
	const to = {
		up: at - 1,
		down: at + 1,
		pageUp: at - TRACK_PAGE,
		pageDown: at + TRACK_PAGE,
		home: 0,
		end: last,
	}[move];
	return ids[Math.min(Math.max(to, 0), last)];
}

/** One meta row of a single-row Hero, with the role the frame paints it in. */
export interface HeroMetaRow {
	segments: Segments;
	role: 'meta' | 'duration' | 'progress';
}

/** The joined text of one Hero meta row: the line's text without its types, for
 * the places that compare or join a row as one string. */
export function metaRowText(row: GuiMetaRow): string {
	return row.map((segment) => segment.text).join('');
}

/** The meta rows of a Hero that shows one selected row (add-gui-library-panel
 * design D15): each row's typed segments with its duration or progress role, as
 * the core's `durationRow` and `progressRow` indices mark them. */
export function heroMetaRows(facts: {
	metaRows: GuiMetaRow[];
	durationRow: number | null;
	progressRow: number | null;
}): HeroMetaRow[] {
	return facts.metaRows.map((row, index) => ({
		segments: row,
		role:
			index === facts.durationRow
				? 'duration'
				: index === facts.progressRow
					? 'progress'
					: 'meta',
	}));
}

/** The attribute name of one part of a destination's Hero (add-gui-library-panel
 * design D15): `data-library-hero-<name>` for Music (`attr` empty) and
 * `data-library-<attr>-hero-<name>` for the rest. The one rule the frame, the
 * artwork arm, and the action buttons share, so a destination's hooks stay
 * stable. */
export function heroAttr(attr: string, name: string): string {
	return attr === '' ? `data-library-hero-${name}` : `data-library-${attr}-hero-${name}`;
}

/** The facts every Hero carries, as the core's frames declare them. */
export interface HeroLayoutFacts {
	title: string;
	/** The largest unit's name (show, feed, series), when the item has one. */
	parent?: string | null;
	metaRows: GuiMetaRow[];
	durationRow: number | null;
	progressRow: number | null;
	overview?: string | null;
}

/** The frame's header and block for one Hero (add-gui-library-panel design D15,
 * invariant 30): the header's big title is the largest unit — the parent when
 * the item has one, else the item itself — over the plain facts on one line;
 * the block beside the art holds the selected item's name (only when a parent
 * took the header), its duration and progress rows, and its overview. */
export function heroLayout(
	facts: HeroLayoutFacts,
	kicker: string,
): {
	header: { kicker: Segments; title: string; facts: Segments };
	block: {
		label: string;
		name?: string;
		meta: HeroMetaRow[];
		overview: string | null;
	};
} {
	const rows = heroMetaRows(facts);
	const parent = (facts.parent ?? '') === '' ? null : (facts.parent ?? null);
	return {
		header: {
			kicker: labelSegments(kicker),
			title: parent ?? facts.title,
			facts: joinMetaRows(rows.filter((row) => row.role === 'meta').map((row) => row.segments)),
		},
		block: {
			label: facts.title,
			name: parent === null ? undefined : facts.title,
			meta: rows.filter((row) => row.role !== 'meta'),
			overview: facts.overview ?? null,
		},
	};
}
