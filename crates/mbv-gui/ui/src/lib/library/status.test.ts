// add-gui-library-panel row 4.8: the Library Status bar's location (design
// D10) — the destination, the selected collection names in pill order, and the
// picked item's artist or title, joined by the canvas's separator.

import { expect, test } from 'vitest';
import type { GuiLibraryTab, GuiMusicFrame, GuiMusicHero } from '$lib/bridge/types';
import { libraryLocation } from './status';

const TABS: GuiLibraryTab[] = [
	{ position: 1, label: 'Music' },
	{ position: 2, label: 'Audiobooks' },
];

/** The picked album's Hero facts (design D5). */
function hero(): GuiMusicHero {
	return {
		artist: 'Beastie Boys',
		collections: ['Hip-Hop'],
		albumCount: 1,
		artistTracks: null,
		album: {
			albumId: 'al1',
			name: 'Licensed To Ill',
			year: '1986',
			tracks: null,
			total: null,
		},
		menu: [],
	};
}

/** The Music content: the pills disagree with the carried names, so the line
 * proves it reads the core's one derivation (design D6). */
function music(picked: GuiMusicHero | null): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [
			{ groupId: 'g1', name: 'Jazz', selected: true, collection: true },
			{ groupId: 'g2', name: 'Hip-Hop', selected: false, collection: true },
		],
		collectionNames: ['Hip-Hop', 'Soul'],
		loading: false,
		failure: null,
		artists: [],
		hero: picked,
		art: null,
		playingTrackId: null,
		rowMenu: null,
	};
}

test('the location joins the destination, the selected collections, and the picked artist', () => {
	expect(libraryLocation(TABS, 1, music(hero()))).toBe(
		'Music › Hip-Hop › Soul › Beastie Boys',
	);
});

// A library that holds no group selects none, so the location names no
// collection and leaves the segment out.
test('a library with no group names no collection', () => {
	const empty = { ...music(null), collectionNames: [] };

	expect(libraryLocation(TABS, 1, empty)).not.toContain('All');
});

// An unpicked Hero carries no item, so the location stops at the collections
// the canvas joins.
test('an unpicked item leaves the location without its last segment', () => {
	expect(libraryLocation(TABS, 1, music(null))).toBe('Music › Hip-Hop › Soul');
});
