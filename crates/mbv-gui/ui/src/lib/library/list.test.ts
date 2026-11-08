// add-gui-library-panel row 4.5 and 4.6: the Library list's row model (design
// D7) — the canvas tree of artists, albums, and tracks under its 24 px letter
// headings at the canvas's row heights, the filter that narrows the rows
// without case, the rows in view, the cursor's movement over them, and the A–Z
// rail that disables a letter no painted row carries. The model is plain data
// over fixed row heights, so the geometry is asserted without a layout.

import { expect, test } from 'vitest';
import type { GuiMusicArtist, GuiMusicTrack } from '$lib/bridge/types';
import {
	buildLibraryRows,
	letterRail,
	listWindow,
	moveListCursor,
	scrollTopToShowCursor,
} from './list';

/** The track Brand Nubian's album holds. */
const ALL_FOR_ONE: GuiMusicTrack = {
	trackId: 't3',
	name: 'All For One',
	number: 1,
	duration: '4:00',
};

/** The A Tribe Called Quest artist the fixture carries: closed, with one
 * album whose tracks the content does not hold. */
const TRIBE: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a3' },
	name: 'A Tribe Called Quest',
	sortLetter: 'A',
	albums: [{ albumId: 'al4', name: 'The Low End Theory', year: '1991', tracks: null, total: null }],
};

/** The Beastie Boys artist the fixture carries: two albums, the first with a
 * cached tracklist. */
const BEASTIE: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a1' },
	name: 'Beastie Boys',
	sortLetter: 'B',
	albums: [
		{
			albumId: 'al1',
			name: 'Licensed To Ill',
			year: '1986',
			tracks: [
				{ trackId: 't1', name: 'Rhymin & Stealin', number: 1, duration: '3:27' },
				{ trackId: 't2', name: 'Paul Revere', number: 9, duration: '3:40' },
			],
			total: { count: 2, duration: '7:07' },
		},
		{ albumId: 'al2', name: 'Paul’s Boutique', year: '1989', tracks: null, total: null },
	],
};

/** The Brand Nubian artist the fixture carries: one album whose tracklist the
 * content holds. */
const BRAND: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a2' },
	name: 'Brand Nubian',
	sortLetter: 'B',
	albums: [
		{
			albumId: 'al3',
			name: 'One For All',
			year: '1990',
			tracks: [ALL_FOR_ONE],
			total: { count: 1, duration: '4:00' },
		},
	],
};

/** The carried union the fixture builds: the artists in sort-name order, with
 * Beastie Boys and its first album open. */
function rows() {
	return buildLibraryRows({
		artists: [TRIBE, BEASTIE, BRAND],
		open: ['artist:service:a1', 'album:al1'],
		filter: '',
	});
}

test('the canvas tree paints letter headings and the canvas row heights', () => {
	const painted = rows();

	expect(painted.map((row) => row.kind)).toEqual([
		'letter',
		'artist',
		'letter',
		'artist',
		'album',
		'track',
		'track',
		'album',
		'artist',
	]);
	expect(painted.map((row) => row.height)).toEqual([24, 30, 24, 30, 30, 28, 28, 30, 30]);
	expect(painted.map((row) => row.top)).toEqual([0, 24, 54, 78, 108, 138, 166, 194, 224]);
});

test('the filter narrows the rows to the names that contain its text', () => {
	const painted = buildLibraryRows({
		artists: [TRIBE, BEASTIE, BRAND],
		open: [],
		filter: 'ALL FOR',
	});

	expect(painted.map((row) => row.id)).toEqual([
		'letter:B',
		'artist:service:a2',
		'album:al3',
		'track:t3',
	]);
});

test('the rail disables the letters no painted row carries', () => {
	const rail = letterRail(rows());

	expect(rail.length).toBe(26);
	expect(rail.find((letter) => letter.letter === 'A')?.rowId).toBe('artist:service:a3');
	expect(rail.find((letter) => letter.letter === 'B')?.rowId).toBe('artist:service:a1');
	expect(rail.find((letter) => letter.letter === 'C')?.rowId).toBeNull();
});

test('the window paints only the rows in view', () => {
	expect(listWindow(rows(), 54, 60)).toEqual({ start: 2, end: 5 });
});

test('the scroll offset that shows a cursor below the viewport puts its bottom edge there', () => {
	expect(scrollTopToShowCursor(rows(), 'artist:service:a2', 0, 60)).toBe(194);
});

test('movement steps over the letter headings', () => {
	expect(moveListCursor(rows(), 'artist:service:a3', 'down')).toBe('artist:service:a1');
	expect(moveListCursor(rows(), 'artist:service:a3', 'end')).toBe('artist:service:a2');
});
