// add-gui-library-panel row 4.8: the Library Status bar's location (design
// D10) — the destination, the selected collection names in pill order, and the
// picked item's artist or title, joined by the canvas's separator.

import { expect, test } from 'vitest';
import type { GuiLibraryTab, GuiMusicFrame, GuiMusicHero, GuiShowsFrame } from '$lib/bridge/types';
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

// add-gui-library-panel row 6.2: on Home the location reads `Home`, then the
// selected row's Hero title (design D10, D12).
test('the location reads Home and the Home Hero title', () => {
	expect(
		libraryLocation(TABS, null, null, {
			rows: [],
			selected: { kind: 'continue', id: 'e1' },
			hero: {
				title: 'Better Call Saul',
				metaRows: ['1 h 47 m left'],
				durationRow: 0,
				progressRow: null,
				overview: null,
				artShape: 'square',
				art: null,
			},
		}),
	).toBe('Home › Better Call Saul');
});

// add-gui-library-panel row 7.1: on Shows the location reads the destination
// tab's label, the active view mode's name, and the selected row's title
// (design D10, D12).
test('the location reads the Shows destination, the view mode, and the selected row', () => {
	const shows: GuiShowsFrame = {
		libIdx: 0,
		pills: [
			{ mode: 'latest', name: 'Latest', selected: false, collection: false },
			{ mode: 'upcoming', name: 'Upcoming', selected: false, collection: false },
			{ mode: 'all', name: 'All series', selected: true, collection: false },
		],
		collectionNames: ['All series'],
		loading: false,
		workspaceLoading: false,
		rows: [
			{ id: 's1', title: 'Atlanta', subtitle: null, date: null, sortLetter: 'A' },
			{ id: 's2', title: 'The Bear', subtitle: null, date: null, sortLetter: 'B' },
		],
		selected: 's2',
		hero: null,
	};

	expect(
		libraryLocation([{ position: 3, label: 'Shows' }], 3, null, null, shows),
	).toBe('Shows › All series › The Bear');
});
