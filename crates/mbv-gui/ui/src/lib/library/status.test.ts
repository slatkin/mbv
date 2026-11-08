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

/** The Music content: two pills with the second selected, and an optional
 * picked album. */
function music(picked: GuiMusicHero | null): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [
			{ groupId: 'g1', name: 'Jazz', selected: false, collection: true },
			{ groupId: 'g2', name: 'Hip-Hop', selected: true, collection: true },
			{ groupId: 'g3', name: 'Soul', selected: true, collection: true },
		],
		collectionNames: ['Hip-Hop', 'Soul'],
		loading: false,
		failure: null,
		artists: [],
		hero: picked,
		art: null,
	};
}

test('the location joins the destination, the selected collections, and the picked artist', () => {
	expect(libraryLocation(TABS, 1, music(hero()))).toBe(
		'Music › Hip-Hop › Soul › Beastie Boys',
	);
});

// An unpicked Hero carries no item, so the location stops at the collections
// the canvas joins.
test('an unpicked item leaves the location without its last segment', () => {
	expect(libraryLocation(TABS, 1, music(null))).toBe('Music › Hip-Hop › Soul');
});
