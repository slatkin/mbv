// The Library list's row derivation under scrolling (task 1.3 of
// `gui-change-driven-sections`, design D12): every scroll step through
// `scrollLibraryList` rebuilds the canvas rows from the carried content and
// clamps the offset to them, so the step's cost is the derivation's cost. The
// fixture holds 5,000 closed artists, one row each, under 26 letter headings.
//
// Run with `pnpm vitest bench --run` in `crates/mbv-gui/ui`. A bench is not a
// test: it asserts nothing and runs outside `make ui-test`.

import { test } from 'vitest';
import type {
	GuiLibraryDetail,
	GuiLibraryList,
	GuiMusicArtist,
	GuiMusicDetail,
	GuiMusicList,
} from '$lib/bridge/types';
import { NODE_ROW_HEIGHT, RAIL_LETTERS } from './list';
import { libraryDetail, libraryList } from '$lib/bridge/sections';
import { initialLibraryUi, libraryUi, measureLibraryList, scrollLibraryList } from './state';

/** Artists in the fixture; each closed artist is one row under its letter. */
const ARTISTS = 5_000;
/** Scroll steps per bench iteration. */
const SCROLL_STEPS = 100;
/** Each step moves 50 rows, so the steps walk the whole 5,000-row list. */
const SCROLL_STRIDE = NODE_ROW_HEIGHT * 50;
/** The list's measured viewport height, in px. */
const VIEWPORT_HEIGHT = 600;

/** The artist at `index`. Artists are ordered by letter, so the list paints
 * one letter heading per letter of the alphabet. */
function artist(index: number): GuiMusicArtist {
	const letter = RAIL_LETTERS[Math.floor((index * RAIL_LETTERS.length) / ARTISTS)];
	return {
		artistKey: { kind: 'service', id: `artist-${index}` },
		name: `Artist ${index}`,
		sortLetter: letter,
		albums: [{ albumId: `album-${index}`, name: `Album ${index}`, year: '2000', tracks: null, total: null }],
	};
}

/** The carried Library list and detail of one grouped Music library over the
 * fixture's artists, with no Hero and no album picked. */
function musicSections(): { list: GuiLibraryList; detail: GuiLibraryDetail } {
	const music: GuiMusicList = {
		libIdx: 0,
		pills: [],
		collectionNames: ['Bench'],
		loading: false,
		failure: null,
		artists: Array.from({ length: ARTISTS }, (_, index) => artist(index)),
	};
	const detail: GuiMusicDetail = { hero: null, art: null, playingTrackId: null };
	return {
		list: { tabs: [], currentTab: 0, destination: { kind: 'music', ...music } },
		detail: { kind: 'music', ...detail },
	};
}

// The content and the viewport are the list's inputs, not the scroll: they
// are set once, when the module loads, so the bench times only the steps.
const sections = musicSections();
libraryList.set(sections.list);
libraryDetail.set(sections.detail);
libraryUi.set(initialLibraryUi());
measureLibraryList(VIEWPORT_HEIGHT);

/** One scroll pass: each step moves the offset down the list, through
 * `scrollLibraryList`. */
function scrollThroughList(): void {
	for (let step = 0; step < SCROLL_STEPS; step += 1) {
		scrollLibraryList(step * SCROLL_STRIDE);
	}
}

test('LibraryList row derivation under scrolling', async ({ bench }) => {
	await bench('100 scroll steps over 5,000 rows', () => {
		scrollThroughList();
	}).run();
});
