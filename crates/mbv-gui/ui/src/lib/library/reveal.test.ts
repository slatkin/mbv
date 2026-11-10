// add-gui-library-panel: a restored pick reveals its album row in the list. The
// reveal waits for the list's first measurement, then scrolls the row into
// view. The carried content is mock data; no IPC runs (the album has tracks).

import { beforeEach, expect, test } from 'vitest';
import { get } from 'svelte/store';
import type {
	GuiLibraryContent,
	GuiMusicArtist,
	GuiMusicFrame,
	GuiMusicHero,
} from '$lib/bridge/types';
import {
	carryLibraryContent,
	initialLibraryUi,
	libraryUi,
	measureLibraryList,
	revealPickedAlbum,
} from './state';

beforeEach(() => {
	libraryUi.set(initialLibraryUi());
	carryLibraryContent(null);
});

/** Twenty artists with no albums above the target, so its row starts below a
 * short viewport. */
const FILLER: GuiMusicArtist[] = Array.from({ length: 20 }, (_, index) => ({
	artistKey: { kind: 'service', id: `filler-${index}` },
	name: `Artist ${index}`,
	sortLetter: 'A',
	albums: [],
}));

/** The target artist: one album with its tracks cached, so no fetch runs. */
const BRAND: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a2' },
	name: 'Brand Nubian',
	sortLetter: 'B',
	albums: [{ albumId: 'al3', name: 'One For All', year: '1990', tracks: [], total: null }],
};

const HERO: GuiMusicHero = {
	artist: 'Brand Nubian',
	collections: ['Rock'],
	albumCount: 1,
	artistTracks: { count: 0, duration: '0:00' },
	album: { albumId: 'al3', name: 'One For All', year: '1990', tracks: [], total: null },
	menu: [],
};

function music(): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [{ groupId: 'g1', name: 'Rock', selected: true, collection: true }],
		collectionNames: ['Rock'],
		loading: false,
		failure: null,
		artists: [...FILLER, BRAND],
		hero: HERO,
		art: null,
		playingTrackId: null,
		rowMenu: null,
	};
}

function content(): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: music(),
		home: null,
		shows: null,
		emby: null,
		audiobooks: null,
		podcasts: null,
		feeds: null,
	};
}

test('a reveal scrolls the picked album row into view once the list is measured', () => {
	carryLibraryContent({ revision: 1, search: null, content: content() });

	revealPickedAlbum(music());
	measureLibraryList(40);

	expect(get(libraryUi).scrollTop).toBeGreaterThan(0);
});
