// add-gui-library-panel row 4.2: a press inside the Library panel sends the
// take-focus command (design D4), so the core sets the focus stack and Panel
// focus together before the press acts on the row. Row 4.7 mounts the Hero in
// the arrangement's two places — the Wide pane beside the list and the Narrow
// overlay over it — with the same painters, and the overlay closes on a press
// on its backdrop (design D3). The Tauri IPC is mocked; the asserted payload is
// the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { resetSectionsForTest, status } from '$lib/bridge/sections';
import { DEFAULT_DRAGGED_WIDTHS, LIBRARY_LIST_DEFAULT_WIDTH_PX, draggedWidths } from '$lib/bridge/resize';
import type {
	GuiAudiobooksDetail,
	GuiAudiobooksList,
	GuiEmbyDetail,
	GuiEmbyList,
	GuiFeedsDetail,
	GuiFeedsList,
	GuiMusicHero,
	GuiMusicList,
	GuiShowsDetail,
	GuiShowsList,
	GuiStatusBar,
} from '$lib/bridge/types';
import { carryLibrary } from '$lib/bridge/test-library';
import type { LibrarySections } from '$lib/bridge/test-library';
import { initialLibraryUi, libraryUi } from '$lib/library/state';
import LibraryPanel from './LibraryPanel.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	resetSectionsForTest();
	libraryUi.set(initialLibraryUi());
	// The widths are session state (design D4): a drag held by one test would
	// outlive it.
	draggedWidths.set(DEFAULT_DRAGGED_WIDTHS);
});

/** The frame's Status bar facts: nothing armed and no live toast. */
const idle: GuiStatusBar = { prefixArmed: false, toast: null };

/** The carried Music list part with no album picked. */
function music(): GuiMusicList {
	return {
		libIdx: 0,
		pills: [],
		collectionNames: [],
		loading: false,
		failure: null,
		artists: [],
	};
}

/** The carried Music sections, with `picked` as the Hero's album. */
function musicSections(picked: GuiMusicHero | null = null): LibrarySections {
	return {
		list: {
			tabs: [{ position: 1, label: 'Music' }],
			currentTab: 1,
			destination: { kind: 'music', ...music() },
		},
		detail: { kind: 'music', hero: picked, art: null, playingTrackId: null },
	};
}

/** The picked album's Hero facts: the album block a pick fills (design D5). */
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

test('a press inside the panel sends the take-focus command', () => {
	carryLibrary(musicSections());
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});
	const panel = container.querySelector('[data-library-panel]');

	expect(panel).toBeTruthy();
	fireEvent.pointerDown(panel as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
});

test('the panel paints the top bar and the pill row above the list region', () => {
	carryLibrary(musicSections());
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});
	const panel = container.querySelector('[data-library-panel]');
	const regions = [...(panel?.querySelectorAll('[data-library-region]') ?? [])].map((el) =>
		el.getAttribute('data-library-region'),
	);

	expect(regions.slice(0, 3)).toEqual(['topBar', 'pills', 'hero']);
	expect(regions).toContain('list');
});

// Review of row 4.5, finding P1: a pick fills `music.hero`, so the Narrow
// Hero's place mounted over the whole list — filter, rows, and rail — with an
// opaque fill that no key dismissed. Row 4.7 paints the Hero into it and gives
// it the dismissal the review required: a press on the backdrop closes it.
test('the Narrow overlay paints the Hero and closes on a press on its backdrop', async () => {
	carryLibrary(musicSections(hero()));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'narrow',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 800,
	});
	const overlay = container.querySelector('[data-library-region="heroOverlay"]');

	expect(overlay?.querySelector('[data-library-hero]')).toBeTruthy();
	expect((overlay as HTMLElement).style.background).not.toBe('');

	await fireEvent.click(
		container.querySelector('[data-library-hero-backdrop]') as HTMLElement,
	);

	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

test('the Wide Hero pane paints the Hero beside the list', () => {
	carryLibrary(musicSections(hero()));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(
		container.querySelector('[data-library-region="hero"] [data-library-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

// add-gui-library-panel row 6.2: while the Home content is present (design
// D12, the destinations are exclusive), the panel mounts the Home painters in
// place of the Music ones, and the pill row and the letter rail do not paint:
// Home paints no pills, no filter, and no rail.
test('the Home content mounts the Home list and Hero and no pills or rail', () => {
	carryLibrary({
		list: {
			tabs: [{ position: 1, label: 'Music' }],
			currentTab: null,
			destination: {
				kind: 'home',
				loading: false,
				rows: [
					{ row: 'heading', text: 'Continue watching' },
					{
						row: 'item',
						id: { kind: 'continue', id: 'e1' },
						title: 'Better Call Saul',
						subtitle: null,
						date: null,
						kind: 'media',
						state: 'ordinary',
						art: null,
					},
				],
			},
		},
		detail: {
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
	});
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-library-region="pills"]')).toBeNull();
	expect(container.querySelector('[data-library-region="rail"]')).toBeNull();
	expect(container.querySelector('[data-library-home-list]')).toBeTruthy();
	expect(
		container.querySelector('[data-library-region="hero"] [data-library-home-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-list]')).toBeNull();
});

// The Narrow Hero overlay works for the Home Hero through the same painters
// the Wide pane uses (design D4, D12), and closes on a press on its backdrop.
test('the Narrow overlay paints the Home Hero and closes on a press on its backdrop', async () => {
	carryLibrary({
		list: {
			tabs: [{ position: 1, label: 'Music' }],
			currentTab: null,
			destination: { kind: 'home', loading: false, rows: [] },
		},
		detail: {
			kind: 'home',
			selected: { kind: 'continue', id: 'e1' },
			hero: {
				title: 'Better Call Saul',
				metaRows: [],
				durationRow: null,
				progressRow: null,
				overview: null,
				artShape: 'square',
				art: null,
			},
		},
	});
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'narrow',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 800,
	});
	const overlay = container.querySelector('[data-library-region="heroOverlay"]');

	expect(overlay?.querySelector('[data-library-home-hero]')).toBeTruthy();

	await fireEvent.click(
		container.querySelector('[data-library-hero-backdrop]') as HTMLElement,
	);

	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

// add-gui-library-panel row 7.1: while the Shows content is present (design
// D12, the destinations are exclusive), the panel mounts the Shows painters in
// place of the Music and Home ones — its mode pills, its list, and the series
// Hero with its Workspace.
test('the Shows content mounts the mode pills, the Shows list, and the series Hero', () => {
	carryLibrary(showsSections('all'));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-library-region="pills"]')).toBeTruthy();
	expect(container.querySelector('[data-library-shows-list]')).toBeTruthy();
	expect(
		container.querySelector('[data-library-region="hero"] [data-library-shows-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-list]')).toBeNull();
	expect(container.querySelector('[data-library-home-list]')).toBeNull();
});

// The Upcoming mode carries no Hero (design D12): the panel paints its flat
// list and no Hero in the Wide pane or the Narrow overlay.
test('the Upcoming Shows content paints no Hero', () => {
	carryLibrary(showsSections('upcoming'));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'narrow',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 800,
	});

	expect(container.querySelector('[data-library-shows-list]')).toBeTruthy();
	expect(container.querySelector('[data-library-shows-hero]')).toBeNull();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

/** The carried Shows list part of `mode`: the three mode pills and one row. */
function showsList(mode: 'latest' | 'upcoming' | 'all'): GuiShowsList {
	return {
		libIdx: 0,
		pills: [
			{ mode: 'latest', name: 'Latest', selected: mode === 'latest', collection: false },
			{ mode: 'upcoming', name: 'Upcoming', selected: mode === 'upcoming', collection: false },
			{ mode: 'all', name: 'All series', selected: mode === 'all', collection: false },
		],
		collectionNames: [mode],
		loading: false,
		series: [],
		upcomingFailed: false,
		rows: [{ id: 's1', title: 'Atlanta', subtitle: null, date: null, sortLetter: 'A' }],
	};
}

/** The carried Shows detail part of `mode`: the series Hero, which the
 * Upcoming mode does not paint (design D12). */
function showsDetail(mode: 'latest' | 'upcoming' | 'all'): GuiShowsDetail {
	return {
		workspaceLoading: false,
		selected: 's1',
		hero:
			mode === 'upcoming'
				? null
				: {
						title: 'Atlanta',
						metaRows: [[{ text: '4 seasons', kind: 'count' }]],
						durationRow: null,
						progressRow: null,
						overview: null,
						artShape: 'landscape',
						art: null,
						workspace: null,
					},
	};
}

/** The Shows sections of `mode`, under the Shows tab. */
function showsSections(mode: 'latest' | 'upcoming' | 'all'): LibrarySections {
	return {
		list: {
			tabs: [{ position: 3, label: 'Shows' }],
			currentTab: 3,
			destination: { kind: 'shows', ...showsList(mode) },
		},
		detail: { kind: 'shows', ...showsDetail(mode) },
	};
}

/** The plain Emby-library list part: the plain mode's list, or the feed-group
 * mode's pills and date-ordered rows. */
function emby(feedGroup: boolean): GuiEmbyList {
	return {
		libIdx: 3,
		libraryName: 'Movies',
		pills: feedGroup
			? [{ group: { kind: 'all' }, name: 'All', selected: true, collection: false }]
			: [],
		collectionNames: feedGroup ? ['All'] : [],
		feedGroup,
		loading: false,
		failed: false,
		rows: [
			{
				id: 'm1',
				title: 'Alien',
				subtitle: '1979',
				date: feedGroup ? '1979' : null,
				sortLetter: feedGroup ? null : 'A',
				kind: 'media',
			},
		],
	};
}

/** The plain Emby-library detail part: the row selected, with a Hero. */
function embyDetail(): GuiEmbyDetail {
	return {
		selected: 'm1',
		hero: {
			kind: 'movie',
			title: 'Alien',
			metaRows: [[{ text: '1979', kind: 'date' }]],
			durationRow: 0,
			progressRow: null,
			overview: null,
			artShape: 'landscape',
			art: null,
			credits: [],
			imdbUrl: null,
		},
	};
}

/** The Emby sections under the tab `label` at position 3. */
function embySections(feedGroup: boolean, label: string): LibrarySections {
	return {
		list: {
			tabs: [{ position: 3, label }],
			currentTab: 3,
			destination: { kind: 'emby', ...emby(feedGroup) },
		},
		detail: { kind: 'emby', ...embyDetail() },
	};
}

// add-gui-library-panel row 8.1: while the plain Emby-library content is present
// (design D12, the destinations are exclusive), the panel mounts the Emby
// painters in place of the Music, Home, and Shows ones — its list with the plain
// mode's rail, its Hero with no Workspace, and no pill row.
test('the plain Emby content mounts the Emby list, rail, and Hero and none of the other destinations', () => {
	carryLibrary(embySections(false, 'Movies'));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-library-emby-list]')).toBeTruthy();
	expect(container.querySelector('[data-library-region="rail"]')).toBeTruthy();
	expect(
		container.querySelector('[data-library-region="hero"] [data-library-emby-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-region="pills"]')).toBeNull();
	expect(container.querySelector('[data-library-list]')).toBeNull();
	expect(container.querySelector('[data-library-home-list]')).toBeNull();
	expect(container.querySelector('[data-library-shows-list]')).toBeNull();
});

test('the feed-group Emby content mounts its pill row and no rail', () => {
	carryLibrary(embySections(true, 'Home videos'));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-library-emby-pill]')).toBeTruthy();
	expect(container.querySelector('[data-library-emby-list]')).toBeTruthy();
	expect(container.querySelector('[data-library-region="rail"]')).toBeNull();
});

/** The carried Audiobooks list part: the catalog's book list. */
function audiobooksList(): GuiAudiobooksList {
	return {
		libIdx: 4,
		loading: false,
		failed: false,
		rows: [
			{
				id: 'b1',
				title: 'Aftermath',
				subtitle: 'Bill Wyman',
				duration: '4:32',
				sortLetter: 'A',
			},
		],
	};
}

/** The carried Audiobooks detail part: the selected book, with `hero` its Hero
 * the frame paints (row 9.1). */
function audiobooksDetail(hero: GuiAudiobooksDetail['hero']): GuiAudiobooksDetail {
	return {
		workspaceLoading: false,
		selected: 'b1',
		playingBookId: null,
		hero,
	};
}

/** The Audiobooks sections under the Audiobooks tab. */
function audiobooksSections(hero: GuiAudiobooksDetail['hero']): LibrarySections {
	return {
		list: {
			tabs: [{ position: 4, label: 'Audiobooks' }],
			currentTab: 4,
			destination: { kind: 'audiobooks', ...audiobooksList() },
		},
		detail: { kind: 'audiobooks', ...audiobooksDetail(hero) },
	};
}

// add-gui-library-panel row 9.1: while the Audiobooks content is present
// (design D12, the destinations are exclusive), the panel mounts the book list
// with its rail and the book Hero in place of the Music, Home, Shows, and Emby
// painters, and no pill row rides on the destination.
test('the Audiobooks content mounts the book list, rail, and Hero and none of the other destinations', () => {
	carryLibrary(
		audiobooksSections({
			title: 'Aftermath',
			metaRows: [[{ text: 'Bill Wyman', kind: 'plain' }], [{ text: '4:32', kind: 'time' }]],
			durationRow: 0,
			progressRow: null,
			overview: null,
			artShape: 'portrait',
			art: null,
			workspace: null,
		}),
	);
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-library-abs-list]')).toBeTruthy();
	expect(container.querySelector('[data-library-region="rail"]')).toBeTruthy();
	expect(
		container.querySelector('[data-library-region="hero"] [data-library-abs-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-region="pills"]')).toBeNull();
	expect(container.querySelector('[data-library-list]')).toBeNull();
	expect(container.querySelector('[data-library-home-list]')).toBeNull();
	expect(container.querySelector('[data-library-shows-list]')).toBeNull();
	expect(container.querySelector('[data-library-emby-list]')).toBeNull();
});

// A frame without a Hero paints no Hero (row 9.1), and the Narrow overlay
// stays closed while the selection carries none.
test('the Hero-less Audiobooks content paints its list and no Hero', () => {
	carryLibrary(audiobooksSections(null));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'narrow',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 800,
	});

	expect(container.querySelector('[data-library-abs-list]')).toBeTruthy();
	expect(container.querySelector('[data-library-abs-hero]')).toBeNull();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

// The Narrow overlay works for the Audiobooks Hero through the same painters
// the Wide pane uses (design D4), and — copied from the row-7.1 lesson — a
// stale Hero paints through an overlay the frame's own selection carries.
test('the Narrow overlay paints the Audiobooks Hero and closes on a press on its backdrop', async () => {
	carryLibrary(
		audiobooksSections({
			title: 'Aftermath',
			metaRows: [[{ text: 'Bill Wyman', kind: 'plain' }], [{ text: '4:32', kind: 'time' }]],
			durationRow: 0,
			progressRow: null,
			overview: null,
			artShape: 'portrait',
			art: null,
			workspace: { rows: [{ chapter: 1, title: 'Act one', duration: '2:16', startTicks: 0, endTicks: 136 * 1_000_000 }] },
		}),
	);
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'narrow',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 800,
	});
	const overlay = container.querySelector('[data-library-region="heroOverlay"]');

	expect(overlay?.querySelector('[data-library-abs-hero]')).toBeTruthy();

	await fireEvent.click(
		container.querySelector('[data-library-hero-backdrop]') as HTMLElement,
	);

	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

/** The carried Feeds list part: the Latest and one subscription pill, and one
 * merged entry row. */
function feedsList(): GuiFeedsList {
	return {
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false },
			{
				pill: { kind: 'subscription', id: 'https://sub.one' },
				name: 'Sub One',
				selected: false,
				collection: false,
			},
		],
		loading: false,
		failed: false,
		rows: [
			{
				id: 's1::e1',
				title: 'Entry A',
				subtitle: 'Sub One',
				date: '12 May',
				state: 'ordinary',
			},
		],
	};
}

/** The Feeds sections under the Feeds tab, with `hero` the selected entry's
 * Hero the frame paints (row 11.1). */
function feedsSections(hero: GuiFeedsDetail['hero']): LibrarySections {
	return {
		list: {
			tabs: [{ position: 5, label: 'Feeds' }],
			currentTab: 5,
			destination: { kind: 'feeds', ...feedsList() },
		},
		detail: { kind: 'feeds', selected: 's1::e1', hero },
	};
}

// add-gui-library-panel row 11.1: while the Feeds content is present (design
// D12, the destinations are exclusive), the panel mounts the view pills, the
// date-gutter list, and the entry Hero, and none of the other destinations'
// painters.
test('the Feeds content mounts the Feeds pills, list, and Hero and none of the other destinations', () => {
	carryLibrary(
		feedsSections({
			title: 'Entry A',
			metaRows: [[{ text: 'Sub One', kind: 'plain' }], [{ text: '12:30', kind: 'time' }]],
			durationRow: 1,
			progressRow: null,
			artShape: 'square',
		}),
	);
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-library-region="pills"]')).toBeTruthy();
	expect(container.querySelector('[data-library-feeds-list]')).toBeTruthy();
	expect(
		container.querySelector('[data-library-region="hero"] [data-library-feeds-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-list]')).toBeNull();
	expect(container.querySelector('[data-library-podcasts-list]')).toBeNull();
	expect(container.querySelector('[data-library-home-list]')).toBeNull();
	expect(container.querySelector('[data-library-emby-list]')).toBeNull();
});

// A frame without a Hero paints no Hero (row 11.1), and the Narrow overlay
// stays closed while the selection carries none.
test('the Hero-less Feeds content paints its list and no Hero', () => {
	carryLibrary(feedsSections(null));
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'narrow',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 800,
	});

	expect(container.querySelector('[data-library-feeds-list]')).toBeTruthy();
	expect(container.querySelector('[data-library-feeds-hero]')).toBeNull();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

// add-gui-library-panel design D8, D15: the right-click item menu is the
// Library's, not one destination's. The row menu rides its own section, so a
// Shows row's menu paints while Music carries no content — before D15 the panel
// read the menu off the Music frame, so every other destination's right-click
// opened nothing.
test('the item menu paints on a destination that is not Music', () => {
	libraryUi.update((ui) => ({
		...ui,
		rowMenu: { itemId: 's1', x: 40, y: 60 },
	}));
	carryLibrary({
		...showsSections('all'),
		rowMenu: {
			itemId: 's1',
			entries: [{ label: 'Play', keyHint: null, index: 0 }],
		},
	});
	status.set(idle);
	const { container } = render(LibraryPanel, {
		arrangement: 'wide',
		listWidth: LIBRARY_LIST_DEFAULT_WIDTH_PX,
		libraryColumn: 1200,
	});

	expect(container.querySelector('[data-row-menu]')).toBeTruthy();
	expect(container.querySelector('[data-menu-entry="0"]')?.textContent).toContain('Play');
});
