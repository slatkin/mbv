// add-gui-library-panel task 4.1: the Library's key router runs before the
// input forwarder (design D3). The window keys and the Library keys stay in the
// front end, an open core overlay or an armed prefix keeps the keyboard with
// the core, and an open text field types. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/keys.rs and
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {
	GuiLibraryContent,
	GuiLibraryTab,
	GuiMusicArtist,
	GuiMusicHero,
	GuiMusicPill,
	GuiPanel,
} from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import {
	carryLibraryContent,
	initialLibraryUi,
	libraryUi,
	measureLibraryList,
	openLibraryFilter,
	reportLibraryArrangement,
	setLibraryFilter,
} from './state';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockClear();
	libraryUi.set(initialLibraryUi());
	carryLibraryContent(null);
});

/** A slice frame's routing facts (design D3) with the Library panel shown. */
function frame(
	panel: GuiPanel = 'library',
	rest: Partial<LibraryKeyFrame> = {},
): LibraryKeyFrame {
	return {
		focus: { surface: panel, panel },
		overlay: null,
		status: { prefixArmed: false, toast: null },
		summary: { filterActive: false },
		library: { revision: 1, content: null },
		...rest,
	};
}

const pills: GuiMusicPill[] = [
	{ groupId: 'g1', name: 'Rock', selected: true, collection: true },
	{ groupId: 'g2', name: 'Jazz', selected: false, collection: true },
];

/** The Beastie Boys artist: one album with a cached tracklist, one without. */
const BEASTIE: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a1' },
	name: 'Beastie Boys',
	sortLetter: 'B',
	albums: [
		{
			albumId: 'al1',
			name: 'Licensed To Ill',
			year: '1986',
			tracks: [{ trackId: 't1', name: 'Rhymin & Stealin', number: 1, duration: '3:27' }],
			total: { count: 1, duration: '3:27' },
		},
		{ albumId: 'al2', name: 'Paul’s Boutique', year: '1989', tracks: null, total: null },
	],
};

/** The Brand Nubian artist: one album, closed under the artist. */
const BRAND: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a2' },
	name: 'Brand Nubian',
	sortLetter: 'B',
	albums: [{ albumId: 'al3', name: 'One For All', year: '1990', tracks: null, total: null }],
};

/** The carried Music content: two group pills, one selected, the union the
 * list keys move through, and the Hero's facts while an album is picked. */
function content(
	artists: GuiMusicArtist[] = [],
	hero: GuiMusicHero | null = null,
): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: {
			libIdx: 0,
			pills,
			collectionNames: ['Rock'],
			loading: false,
			failure: null,
			artists,
			hero,
			art: null,
			playingTrackId: null,
			rowMenu: null,
		},
	};
}

/** Carry the content the panel paints, so the window holds the Music content
 * its presence fact needs (design D2, D10). */
function carryPanel(): void {
	carryLibraryContent({ revision: 1, content: content() });
}

/** Carry the content with the given destination tabs (design D2): the tabs the
 * digits open.
 */
function carryTabs(tabs: GuiLibraryTab[]): void {
	carryLibraryContent({ revision: 1, content: { ...content(), tabs } });
}

/** The picked album's Hero facts: the tracklist the Hero pane's keys move
 * through (design D3, D8). */
const HERO: GuiMusicHero = {
	artist: 'Beastie Boys',
	collections: ['Rock'],
	albumCount: 1,
	artistTracks: { count: 2, duration: '7:02' },
	album: {
		albumId: 'al1',
		name: 'Licensed To Ill',
		year: '1986',
		tracks: [
			{ trackId: 't1', name: 'Rhymin & Stealin', number: 1, duration: '4:08' },
			{ trackId: 't2', name: 'The New Style', number: 2, duration: '4:35' },
		],
		total: { count: 2, duration: '8:43' },
	},
	menu: [],
};

/** Carry the union the list keys move through, with the list pane focused. */
function carryList(open: string[] = []): void {
	carryLibraryContent({ revision: 1, content: content([BEASTIE, BRAND]) });
	libraryUi.update((ui) => ({ ...ui, pane: 'list', openNodes: open }));
	measureLibraryList(40);
}

/** Dispatch one keydown through the installed forwarder, its router reading
 * the frame the press is routed with. */
function press(
	key: string,
	options: KeyboardEventInit = {},
	held = frame(),
): KeyboardEvent {
	const detach = installInput(window, (event) => routeLibraryKey(event, held));
	const event = new KeyboardEvent('keydown', {
		...options,
		key,
		cancelable: true,
	});
	window.dispatchEvent(event);
	detach();
	return event;
}

test('Library Panel focus: / opens the filter and sends no key', () => {
	carryPanel();

	press('/');

	expect(get(libraryUi).filterOpen).toBe(true);
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Queue Panel focus sends / to the core', () => {
	carryPanel();

	press('/', {}, frame('queue'));

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: '/' }),
	});
});

test('an armed prefix sends a Library key to the core', () => {
	carryPanel();

	press(
		'n',
		{},
		frame('library', { status: { prefixArmed: true, toast: null } }),
	);

	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: 'n' }),
	});
});

test('an open core overlay sends a Library key to the core', () => {
	carryPanel();

	press(
		'/',
		{},
		frame('library', {
			overlay: {
				kind: 'rowMenu',
				anchorSlot: 0,
				cursor: 0,
				entries: [],
			},
		}),
	);

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: '/' }),
	});
});

// The panel's presence is one fact for the window and the router (design D2,
// D10): a frame that carries no Library section takes no Library key, even
// while the window still holds the content of an earlier frame.
test('a frame without the Library section leaves every Library key to the core', () => {
	carryPanel();

	press('/', {}, frame('library', { library: null }));

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(invokeMock).not.toHaveBeenCalledWith('library', expect.anything());
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: '/' }),
	});
});

test('Space on a focused pill asks for its toggle and sends no key', () => {
	carryLibraryContent({ revision: 1, content: content() });
	libraryUi.update((ui) => ({ ...ui, focusedPill: 'g2' }));

	press(' ');

	expect(get(libraryUi).pillSelect).toEqual({ groupId: 'g2' });
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space with no focused pill is sent to the core', () => {
	carryLibraryContent({ revision: 1, content: content() });

	press(' ');

	expect(get(libraryUi).pillSelect).toBeNull();
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: ' ' }),
	});
});

test('Right with no focused pill focuses the first pill and sends no key', () => {
	carryLibraryContent({ revision: 1, content: content() });

	press('ArrowRight');

	expect(get(libraryUi).focusedPill).toBe('g1');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Enter on a focused pill asks for its toggle and sends no key', () => {
	carryPanel();
	libraryUi.update((ui) => ({ ...ui, focusedPill: 'g2' }));

	press('Enter');

	expect(get(libraryUi).pillSelect).toEqual({ groupId: 'g2' });
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Tab with Queue Panel focus takes Library focus and sends no key', () => {
	carryPanel();

	press('Tab', {}, frame('queue'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
	expect(get(libraryUi).pane).toBe('pills');
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

// The walks cross the panel boundary at their ends (design D3, D4): Tab past
// the Hero and Shift+Tab before the pill row give the Queue panel Panel focus,
// and only Queue Panel focus asks for the Library panel's focus.
test('Tab past the last Library pane takes Queue Panel focus', () => {
	carryPanel();
	libraryUi.update((ui) => ({ ...ui, pane: 'hero' }));

	press('Tab');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

test('Shift+Tab before the first Library pane takes Queue Panel focus', () => {
	carryPanel();

	press('Tab', { shiftKey: true });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

// A digit names a carried tab, never a raw position (design D3, step 3): the
// presented destinations are the tabs the top bar paints.
test('a digit opens the carried tab at its place', () => {
	carryTabs([
		{ position: 1, label: 'Music' },
		{ position: 4, label: 'Audiobooks' },
	]);

	press('2');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openDestination', position: 4 },
	});
});

test('a digit past the carried tabs opens nothing', () => {
	carryTabs([{ position: 1, label: 'Music' }]);

	press('2');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('the open filter takes a typed key without preventing its default', () => {
	carryPanel();
	libraryUi.update((ui) => ({ ...ui, filterOpen: true }));

	const event = press('r');

	expect(event.defaultPrevented).toBe(false);
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Esc closes the filter, clears its text, and sends no key', () => {
	carryPanel();
	libraryUi.update((ui) => ({
		...ui,
		filterOpen: true,
		filter: 'roc',
	}));

	press('Escape');

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(get(libraryUi).filter).toBe('');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('a cursor move below the rows in view scrolls its row into view', () => {
	carryList();
	libraryUi.update((ui) => ({ ...ui, cursor: 'artist:service:a1' }));

	press('ArrowDown');

	expect(get(libraryUi).cursor).toBe('artist:service:a2');
	expect(get(libraryUi).scrollTop).toBe(44);
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Right opens the cursor album and asks for its tracks', () => {
	carryList(['artist:service:a1']);
	libraryUi.update((ui) => ({ ...ui, cursor: 'album:al2' }));

	press('ArrowRight');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openAlbum', albumId: 'al2' },
	});
	expect(get(libraryUi).openNodes).toContain('album:al2');
});

// The Library is mouse-driven for picking and keyboard-driven for playback
// (`design.md` D7): Enter plays the item under the cursor and never picks it,
// so a mouse press stays the only way to pick an album for the Hero.
test('Enter in the list plays the item under the cursor', () => {
	carryList(['artist:service:a1']);
	libraryUi.update((ui) => ({ ...ui, cursor: 'album:al2' }));

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 'al2' },
	});
	expect(invokeMock).not.toHaveBeenCalledWith('library', {
		command: { command: 'showDetail', itemId: 'al2' },
	});
});

// A track row plays its own track, not the album a pick would show.
test('Enter in the list plays the track under the cursor', () => {
	carryList(['artist:service:a1', 'album:al1']);
	libraryUi.update((ui) => ({ ...ui, cursor: 'track:t1' }));

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 't1' },
	});
});

// Review of row 4.5, finding P2: the filter opens a node that holds a match,
// so the cursor can rest on a track row that only the narrowing paints. Esc
// clears the filter, and the row goes: the cursor must move to a row that does
// paint, or Enter and Right would reach no row at all.
test('Esc that clears the filter moves a stranded cursor to a painted row', () => {
	carryList();
	setLibraryFilter('rhymin');
	press('ArrowDown');
	press('ArrowDown');
	expect(get(libraryUi).cursor).toBe('track:t1');

	openLibraryFilter();
	press('Escape');

	expect(get(libraryUi).cursor).toBe('artist:service:a1');
});

// Row 4.7: the focused pane's cursor decides what `n`, `a`, Enter, and the
// movement keys act on (design D3, step 4), and Esc closes the front end's own
// menu and overlay.
test('n with the list cursor on an album sends play next for that album', () => {
	carryList(['artist:service:a1']);
	libraryUi.update((ui) => ({ ...ui, cursor: 'album:al1' }));

	press('n');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'playNext', itemId: 'al1' },
	});
});

test('a with the Hero pane focused sends add to queue for the tracklist cursor track', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	libraryUi.update((ui) => ({ ...ui, pane: 'hero', trackCursor: 't1' }));

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'addToQueue', itemId: 't1' },
	});
});

test('Down with the Hero pane focused moves the tracklist cursor', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	libraryUi.update((ui) => ({ ...ui, pane: 'hero', trackCursor: 't1' }));

	press('ArrowDown');

	expect(get(libraryUi).trackCursor).toBe('t2');
	expect(invokeMock).not.toHaveBeenCalled();
});

// Review of row 4.7, finding P2-a: the pane walk reached the Hero in Narrow
// while its overlay was dismissed, so the movement keys, `n`/`a`, and Enter
// acted on a pane that painted nothing. Taking the Hero's pane paints it again.
test('Tab onto the Hero in Narrow paints its overlay again', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	reportLibraryArrangement('narrow');
	libraryUi.update((ui) => ({ ...ui, pane: 'list', heroDismissed: true }));

	press('Tab');

	expect(get(libraryUi).pane).toBe('hero');
	expect(get(libraryUi).heroDismissed).toBe(false);
});

// The other half of the same invariant: closing the overlay while the Hero pane
// held the focus moves the focus back to the painted list.
test('Esc with the Hero pane focused returns the pane focus to the list', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	reportLibraryArrangement('narrow');
	libraryUi.update((ui) => ({ ...ui, pane: 'hero' }));

	press('Escape');

	expect(get(libraryUi).heroDismissed).toBe(true);
	expect(get(libraryUi).pane).toBe('list');
});

// Review of row 4.7, finding P2-b: the cursor was seeded only when a pane took
// the focus, so a tracklist whose cache landed afterwards painted no cursor and
// Enter did nothing. The cursor now derives from the tracks that paint.
test('Enter in the Hero pane plays the track the painted tracklist shows', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	libraryUi.update((ui) => ({ ...ui, pane: 'hero', trackCursor: null }));

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 't1' },
	});
});

test('Esc closes the open item menu and sends no key', () => {
	carryPanel();
	libraryUi.update((ui) => ({ ...ui, itemMenuOpen: true }));

	press('Escape');

	expect(get(libraryUi).itemMenuOpen).toBe(false);
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Esc closes the Narrow Hero overlay and sends no key', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	reportLibraryArrangement('narrow');

	press('Escape');

	expect(get(libraryUi).heroDismissed).toBe(true);
	expect(invokeMock).not.toHaveBeenCalled();
});

// The overlay paints only in Narrow (design D4): the Esc that would close it
// elsewhere still belongs to the core's router.
test('Esc with no overlay or menu open is the core\u2019s key', () => {
	carryLibraryContent({ revision: 1, content: content([BEASTIE], HERO) });
	reportLibraryArrangement('wide');

	press('Escape');

	expect(get(libraryUi).heroDismissed).toBe(false);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: 'Escape' }),
	});
});
