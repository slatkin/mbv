// gui-change-driven-sections design D9: a scroll step moves the Music list's
// scroll store and leaves the rows the list derives in place. The carried
// content is mock data; no IPC runs.

import { beforeEach, expect, test } from 'vitest';
import { get } from 'svelte/store';
import type { GuiMusicArtist, GuiMusicList } from '$lib/bridge/types';
import { carryLibrary } from '$lib/bridge/test-library';
import type { LibrarySections } from '$lib/bridge/test-library';
import type { LibraryTree } from './list';
import { initialListScroll, musicScroll } from './scroll';
import {
	initialLibraryUi,
	libraryRowsOf,
	libraryUi,
	measureLibraryList,
	scrollLibraryList,
} from './state';

beforeEach(() => {
	libraryUi.set(initialLibraryUi());
	musicScroll.set(initialListScroll());
	carryLibrary(null);
});

/** Twenty artists under one letter, each closed, so the list is taller than a
 * short viewport. */
const ARTISTS: GuiMusicArtist[] = Array.from({ length: 20 }, (_, index) => ({
	artistKey: { kind: 'service', id: `artist-${index}` },
	name: `Artist ${index}`,
	sortLetter: 'A',
	albums: [],
}));

function content(): LibrarySections {
	const music: GuiMusicList = {
		libIdx: 0,
		pills: [],
		collectionNames: ['Rock'],
		loading: false,
		failure: null,
		artists: ARTISTS,
	};
	return {
		list: {
			tabs: [{ position: 1, label: 'Music' }],
			currentTab: 1,
			destination: { kind: 'music', ...music },
		},
		detail: { kind: 'music', hero: null, art: null, playingTrackId: null },
	};
}

/** The tree the list paints under the current interaction state. */
function paintedTree(): LibraryTree {
	const ui = get(libraryUi);
	return { artists: ARTISTS, open: ui.openNodes, closed: ui.filterClosed, filter: ui.filter };
}

test('a scroll step moves the scroll store and keeps the derived rows', () => {
	carryLibrary(content());
	measureLibraryList(40);
	const rows = libraryRowsOf(paintedTree());

	scrollLibraryList(20);

	expect(get(musicScroll).scrollTop).toBeGreaterThan(0);
	expect(libraryRowsOf(paintedTree())).toBe(rows);
});
