// add-gui-library-panel row 4.8: the Library Status bar's location (design
// D10) — the destination, the selected collection names in pill order, and the
// picked item's artist or title, joined by the canvas's separator.

import { expect, test } from 'vitest';
import type {
	GuiAudiobooksDetail,
	GuiAudiobooksList,
	GuiDestinationList,
	GuiEmbyDetail,
	GuiFeedsDetail,
	GuiFeedsList,
	GuiLibraryDetail,
	GuiLibraryTab,
	GuiMusicHero,
	GuiMusicList,
	GuiShowsDetail,
	GuiShowsList,
} from '$lib/bridge/types';
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

/** The Music list part: the pills disagree with the carried names, so the line
 * proves it reads the core's one derivation (design D6). */
function musicList(): GuiMusicList & { kind: 'music' } {
	return {
		kind: 'music',
		libIdx: 0,
		pills: [
			{ groupId: 'g1', name: 'Jazz', selected: true, collection: true },
			{ groupId: 'g2', name: 'Hip-Hop', selected: false, collection: true },
		],
		collectionNames: ['Hip-Hop', 'Soul'],
		loading: false,
		failure: null,
		artists: [],
	};
}

/** The Music detail part with `picked` as its Hero. */
function musicDetail(picked: GuiMusicHero | null): GuiLibraryDetail {
	return { kind: 'music', hero: picked, art: null, playingTrackId: null };
}

test('the location joins the destination, the selected collections, and the picked artist', () => {
	expect(libraryLocation(TABS, 1, musicList(), musicDetail(hero()))).toBe(
		'Music › Hip-Hop › Soul › Beastie Boys',
	);
});

// A library that holds no group selects none, so the location names no
// collection and leaves the segment out.
test('a library with no group names no collection', () => {
	const empty = { ...musicList(), collectionNames: [] };

	expect(libraryLocation(TABS, 1, empty, musicDetail(null))).not.toContain('All');
});

// An unpicked Hero carries no item, so the location stops at the collections
// the canvas joins.
test('an unpicked item leaves the location without its last segment', () => {
	expect(libraryLocation(TABS, 1, musicList(), musicDetail(null))).toBe(
		'Music › Hip-Hop › Soul',
	);
});

// add-gui-library-panel row 8.1: on a plain Emby library the location reads the
// destination tab's label and the selected row's title, and the plain mode's
// letter rail replaces the pill row, so no collection segment appears
// (design D10, D12).
test('the location reads the plain Emby destination and the selected row', () => {
	const emby: GuiDestinationList = {
		kind: 'emby',
		libIdx: 2,
		libraryName: 'Movies',
		pills: [],
		collectionNames: [],
		feedGroup: false,
		loading: false,
		failed: false,
		rows: [
			{ id: 'm1', title: 'Alien', subtitle: '1979', date: null, sortLetter: 'A', kind: 'media' },
			{ id: 'm2', title: 'Heat', subtitle: '1995', date: null, sortLetter: 'H', kind: 'media' },
		],
	};
	const detail: GuiEmbyDetail & { kind: 'emby' } = {
		kind: 'emby',
		selected: 'm2',
		hero: null,
	};

	expect(libraryLocation([{ position: 2, label: 'Movies' }], 2, emby, detail)).toBe(
		'Movies › Heat',
	);
});

// add-gui-library-panel row 8.1: a feed-group library names the selected feed
// pill between the destination and the row (design D10, D12).
test('the location reads the feed pill name in feed-group mode', () => {
	const emby: GuiDestinationList = {
		kind: 'emby',
		libIdx: 2,
		libraryName: 'Movies',
		pills: [
			{ group: { kind: 'all' }, name: 'All', selected: false, collection: false },
			{ group: { kind: 'folder', id: 'f1' }, name: 'Home videos', selected: true, collection: false },
		],
		collectionNames: ['Home videos'],
		feedGroup: true,
		loading: false,
		failed: false,
		rows: [
			{ id: 'v1', title: 'Holiday', subtitle: null, date: '2019', sortLetter: null, kind: 'media' },
		],
	};
	const detail: GuiEmbyDetail & { kind: 'emby' } = {
		kind: 'emby',
		selected: 'v1',
		hero: null,
	};

	expect(libraryLocation([{ position: 2, label: 'Home videos' }], 2, emby, detail)).toBe(
		'Home videos › Home videos › Holiday',
	);
});

// add-gui-library-panel row 9.1: on an Audiobooks library the location reads
// the destination tab's label and the selected book's title, joined by the
// canvas's separator (design D10, D12).
test('the location reads the Audiobooks destination and the selected book', () => {
	const audiobooks: GuiAudiobooksList & { kind: 'audiobooks' } = {
		kind: 'audiobooks',
		libIdx: 1,
		loading: false,
		failed: false,
		rows: [
			{ id: 'b1', title: 'Dune', subtitle: 'Frank Herbert', duration: '15:12:00', sortLetter: 'D' },
			{ id: 'b2', title: 'Hyperion', subtitle: 'Dan Simmons', duration: '9:18:00', sortLetter: 'H' },
		],
	};
	const detail: GuiAudiobooksDetail & { kind: 'audiobooks' } = {
		kind: 'audiobooks',
		workspaceLoading: false,
		selected: 'b2',
		playingBookId: null,
		hero: null,
	};

	expect(libraryLocation(TABS, 2, audiobooks, detail)).toBe('Audiobooks › Hyperion');
});

// A frame whose selection names no carried book fills no last segment, so the
// location stops at the destination (design D10, D12).
test('an Audiobooks selection with no carried book leaves the location at the destination', () => {
	const audiobooks: GuiAudiobooksList & { kind: 'audiobooks' } = {
		kind: 'audiobooks',
		libIdx: 1,
		loading: false,
		failed: false,
		rows: [],
	};
	const detail: GuiAudiobooksDetail & { kind: 'audiobooks' } = {
		kind: 'audiobooks',
		workspaceLoading: false,
		selected: null,
		playingBookId: null,
		hero: null,
	};

	expect(libraryLocation(TABS, 2, audiobooks, detail)).toBe('Audiobooks');
});

// add-gui-library-panel row 11.1: on a Feeds destination the location reads
// the destination tab's label, the selected pill's name, and the selected
// entry's title, joined by the canvas's separator (design D10, D12).
test('the location reads the Feeds destination, the selected pill, and the selected entry', () => {
	const feeds: GuiFeedsList & { kind: 'feeds' } = {
		kind: 'feeds',
		loading: false,
		failed: false,
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: false, collection: false },
			{
				pill: { kind: 'subscription', id: 'sub1' },
				name: 'Bits and Bites',
				selected: true,
				collection: false,
			},
		],
		rows: [
			{
				id: 'sub1:guid12',
				title: 'The Long Road Home',
				subtitle: 'Bits and Bites',
				date: '08 Oct',
				state: 'ordinary',
			},
		],
	};
	const detail: GuiFeedsDetail & { kind: 'feeds' } = {
		kind: 'feeds',
		selected: 'sub1:guid12',
		hero: null,
	};

	expect(libraryLocation([{ position: 5, label: 'Feeds' }], 5, feeds, detail)).toBe(
		'Feeds › Bits and Bites › The Long Road Home',
	);
});

// add-gui-library-panel row 6.2: on Home the location reads `Home`, then the
// selected row's Hero title (design D10, D12).
test('the location reads Home and the Home Hero title', () => {
	expect(
		libraryLocation(
			TABS,
			null,
			{ kind: 'home', loading: false, rows: [] },
			{
				kind: 'home',
				selected: { kind: 'continue', id: 'e1' },
				hero: {
					title: 'Better Call Saul',
					metaRows: [[{ text: '1 h 47 m left', kind: 'time' }]],
					durationRow: 0,
					progressRow: null,
					overview: null,
					artShape: 'square',
					art: null,
				},
			},
		),
	).toBe('Home › Better Call Saul');
});

// add-gui-library-panel row 7.1: on Shows the location reads the destination
// tab's label, the active view mode's name, and the selected row's title
// (design D10, D12).
test('the location reads the Shows destination, the view mode, and the selected row', () => {
	const shows: GuiShowsList & { kind: 'shows' } = {
		kind: 'shows',
		libIdx: 0,
		pills: [
			{ mode: 'latest', name: 'Latest', selected: false, collection: false },
			{ mode: 'upcoming', name: 'Upcoming', selected: false, collection: false },
			{ mode: 'all', name: 'All series', selected: true, collection: false },
		],
		collectionNames: ['All series'],
		loading: false,
		series: [],
		upcomingFailed: false,
		rows: [
			{ id: 's1', title: 'Atlanta', subtitle: null, date: null, sortLetter: 'A' },
			{ id: 's2', title: 'The Bear', subtitle: null, date: null, sortLetter: 'B' },
		],
	};
	const detail: GuiShowsDetail & { kind: 'shows' } = {
		kind: 'shows',
		workspaceLoading: false,
		selected: 's2',
		hero: null,
	};

	expect(libraryLocation([{ position: 3, label: 'Shows' }], 3, shows, detail)).toBe(
		'Shows › All series › The Bear',
	);
});
