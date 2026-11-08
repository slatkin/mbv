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
	GuiMusicArtist,
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

/** The carried Music content: two group pills, one selected, and the union the
 * list keys move through. */
function content(artists: GuiMusicArtist[] = []): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: {
			libIdx: 0,
			pills,
			loading: false,
			failure: null,
			artists,
			hero: null,
			art: null,
		},
	};
}

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
	press('/');

	expect(get(libraryUi).filterOpen).toBe(true);
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Queue Panel focus sends / to the core', () => {
	press('/', {}, frame('queue'));

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: '/' }),
	});
});

test('an armed prefix sends a Library key to the core', () => {
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

test('Space on a focused pill asks for its toggle and sends no key', () => {
	carryLibraryContent({ revision: 1, content: content() });
	libraryUi.update((ui) => ({ ...ui, focusedPill: 'g2' }));

	press(' ');

	expect(get(libraryUi).pillToggle).toEqual({ groupId: 'g2', solo: false });
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space with no focused pill is sent to the core', () => {
	carryLibraryContent({ revision: 1, content: content() });

	press(' ');

	expect(get(libraryUi).pillToggle).toBeNull();
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

test('Tab with Queue Panel focus takes Library focus and sends no key', () => {
	press('Tab', {}, frame('queue'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
	expect(get(libraryUi).pane).toBe('pills');
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

test('a digit opens the destination tab at its position', () => {
	press('3');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openDestination', position: 3 },
	});
});

test('the open filter takes a typed key without preventing its default', () => {
	libraryUi.update((ui) => ({ ...ui, filterOpen: true }));

	const event = press('r');

	expect(event.defaultPrevented).toBe(false);
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Esc closes the filter, clears its text, and sends no key', () => {
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

test('Enter picks the item under the cursor', () => {
	carryList(['artist:service:a1']);
	libraryUi.update((ui) => ({ ...ui, cursor: 'album:al2' }));

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showDetail', itemId: 'al2' },
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
