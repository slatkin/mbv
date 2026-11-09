// add-gui-library-panel row 8.1: the plain Emby-library keys (design D12) — the
// list's cursor moves, Enter and the double-click's play command play every row
// (a collection row too), `n` and `a` play next and queue from every pane, the
// feed-group pills walk their groups, `/` is not taken, and the pane walk skips
// the pill row a plain library does not paint and the Hero a frame without one
// does not. The router under test is the panel's one routing site,
// `routeLibraryKey`, so these assertions cover the Emby dispatch it hands to
// `./emby-keys`. The Tauri IPC is mocked; the asserted payloads are the serde
// wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {
	GuiEmbyFrame,
	GuiEmbyHero,
	GuiEmbyRow,
	GuiLibraryContent,
	GuiPanel,
} from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import { embyCursorRow, embyUi, initialEmbyUi } from './emby';
import { carryLibraryContent, initialLibraryUi, libraryUi } from './state';
import type { LibraryPane } from './state';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockClear();
	libraryUi.set(initialLibraryUi());
	embyUi.set(initialEmbyUi());
	carryLibraryContent(null);
});

/** A slice frame's routing facts (design D3) with the Library panel shown. */
function frame(panel: GuiPanel = 'library'): LibraryKeyFrame {
	return {
		focus: { surface: panel, panel },
		overlay: null,
		status: { prefixArmed: false, toast: null },
		summary: { filterActive: false },
		library: { revision: 1, content: null },
	};
}

/** Dispatch one keydown through the installed forwarder, its router reading the
 * frame the press is routed with. */
function press(key: string, options: KeyboardEventInit = {}, held = frame()): KeyboardEvent {
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

/** The Library's own keys: the movements, Right and Left's open and close,
 * Enter, and the item keys `n` and `a` (design D3, step 4). */
const LIBRARY_KEYS = [
	'Enter',
	'ArrowUp',
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'PageUp',
	'PageDown',
	'Home',
	'End',
	'n',
	'a',
];

/** The router's answer for one key under the current carried content. */
function outcome(key: string): string {
	return routeLibraryKey(new KeyboardEvent('keydown', { key }), frame());
}

/** The plain mode's name-sorted rows: two `A` movies and one `B` movie. */
const MOVIES: GuiEmbyRow[] = [
	{ id: 'm1', title: 'Alien', subtitle: '1979', date: null, sortLetter: 'A', kind: 'media' },
	{ id: 'm2', title: 'Amélie', subtitle: '2001', date: null, sortLetter: 'A', kind: 'media' },
	{ id: 'm3', title: 'Brazil', subtitle: '1985', date: null, sortLetter: 'B', kind: 'media' },
];

/** The feed-group mode's date-ordered rows, with a folder as the first. */
const FEED: GuiEmbyRow[] = [
	{ id: 'f1', title: 'Family', subtitle: null, date: '1998', sortLetter: null, kind: 'collection' },
	{ id: 'f2', title: 'Fargo', subtitle: null, date: '1996', sortLetter: null, kind: 'media' },
];

/** The selected row's Hero facts: a plain Emby-library Hero with no
 * Workspace. */
const EMBY_HERO: GuiEmbyHero = {
	title: 'Alien',
	metaRows: ['1979 · 1 h 57 m'],
	durationRow: 0,
	progressRow: null,
	overview: null,
	artShape: 'landscape',
	art: null,
};

/** The carried content while an Emby library is active (row 8.1): no Music,
 * Home, or Shows content, the mode's rows, and the Hero the frame paints. */
function embyContent(
	feedGroup: boolean,
	rows: GuiEmbyRow[],
	hero: GuiEmbyHero | null = null,
): GuiLibraryContent {
	const emby: GuiEmbyFrame = {
		libIdx: 3,
		pills: feedGroup
			? [
					{ group: { kind: 'all' }, name: 'All', selected: true, collection: false },
					{
						group: { kind: 'folder', id: 'folder-1' },
						name: 'Family',
						selected: false,
						collection: false,
					},
				]
			: [],
		collectionNames: feedGroup ? ['All'] : [],
		feedGroup,
		loading: false,
		failed: false,
		rows,
		selected: rows[0]?.id ?? null,
		hero,
	};
	return {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 3, label: 'Movies' },
		],
		currentTab: 3,
		music: null,
		home: null,
		shows: null,
		emby,
		audiobooks: null,
		podcasts: null,
	};
}

/** Carry the Emby content and focus the pane the key acts on. */
function carryEmby(
	content: GuiLibraryContent,
	pane: LibraryPane,
	cursor: string | null = null,
): void {
	carryLibraryContent({ revision: 1, content });
	libraryUi.update((ui) => ({ ...ui, pane }));
	embyUi.set({ ...initialEmbyUi(), cursor });
}

test('Enter on the cursor row plays it', () => {
	carryEmby(embyContent(false, MOVIES), 'list', 'm2');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlay', libIdx: 3, itemId: 'm2' },
	});
});

// A folder or boxset row plays through the core's folder-play rule, so Enter on
// it sends the same play command as any other row (row 8.1).
test('Enter on a collection row plays it', () => {
	carryEmby(embyContent(true, FEED), 'list', 'f1');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlay', libIdx: 3, itemId: 'f1' },
	});
});

test('n and a play the cursor row next and queue it', () => {
	carryEmby(embyContent(false, MOVIES), 'list', 'm1');

	press('n');
	expect(invokeMock).toHaveBeenLastCalledWith('library', {
		command: { command: 'embyPlayNext', libIdx: 3, itemId: 'm1' },
	});

	press('a');
	expect(invokeMock).toHaveBeenLastCalledWith('library', {
		command: { command: 'embyQueue', libIdx: 3, itemId: 'm1' },
	});
});

test('a movement key moves the Emby cursor and sends no key', () => {
	carryEmby(embyContent(false, MOVIES), 'list', 'm1');

	press('ArrowDown');

	expect(embyCursorRow()?.id).toBe('m2');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Right on the feed-group pills focuses the first pill and sends no key', () => {
	carryEmby(embyContent(true, FEED), 'pills');

	press('ArrowRight');

	expect(get(embyUi).focusedPill).toBe('all');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space on a focused feed-group pill asks for its group and sends no key', () => {
	carryEmby(embyContent(true, FEED), 'pills');
	embyUi.update((ui) => ({ ...ui, focusedPill: 'folder:folder-1' }));

	press(' ');

	expect(get(embyUi).pillSelect).toEqual({ kind: 'folder', id: 'folder-1' });
	expect(invokeMock).not.toHaveBeenCalled();
});

test('the Emby list takes no filter key, so / reaches the core', () => {
	carryEmby(embyContent(false, MOVIES), 'list', 'm1');

	press('/');

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: '/' }),
	});
});

// The pane walk skips the panes the Emby frame does not paint (row 8.1): a
// plain library paints no pill row, so the list is the walk's first pane, and a
// frame without a Hero leaves the list the last one.
test('Shift+Tab before the plain list takes Queue Panel focus', () => {
	carryEmby(embyContent(false, MOVIES), 'list');

	press('Tab', { shiftKey: true });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab past the plain list without a Hero takes Queue Panel focus', () => {
	carryEmby(embyContent(false, MOVIES), 'list');

	press('Tab');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab walks from the plain list into the Emby Hero', () => {
	carryEmby(embyContent(false, MOVIES, EMBY_HERO), 'list');

	press('Tab');

	expect(get(libraryUi).pane).toBe('hero');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Tab from the Queue panel focuses the plain list, not an absent pill row', () => {
	carryEmby(embyContent(false, MOVIES), 'list');

	press('Tab', {}, frame('queue'));

	expect(get(libraryUi).pane).toBe('list');
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
});

// Review of row 7.1's lesson, applied to Emby from the start: no Emby pane
// leaks a Library key to the core's router — the feed-group pill row, the
// Hero, and a hero-less Hero pane included. A pane that does not act on a key
// still consumes it, so `a` and `n` never reach the core's playback chords,
// which are not gated on Panel focus.
test('the feed-group pills and a hero-less Hero pane consume every Library key', () => {
	carryEmby(embyContent(true, FEED), 'pills');
	const pills = LIBRARY_KEYS.map(outcome);
	libraryUi.update((ui) => ({ ...ui, pane: 'hero' }));
	const hero = LIBRARY_KEYS.map(outcome);

	expect(pills).toEqual(LIBRARY_KEYS.map(() => 'handled'));
	expect(hero).toEqual(LIBRARY_KEYS.map(() => 'handled'));
});

test('a on the pills pane queues the list cursor row and sends no key', () => {
	carryEmby(embyContent(true, FEED), 'pills', 'f2');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyQueue', libIdx: 3, itemId: 'f2' },
	});
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

test('n on the Emby Hero pane plays the selected row next', () => {
	carryEmby(embyContent(false, MOVIES, EMBY_HERO), 'hero', 'm2');

	press('n');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlayNext', libIdx: 3, itemId: 'm1' },
	});
});

test('a movement key on the Emby Hero pane does not reach the core', () => {
	carryEmby(embyContent(false, MOVIES, EMBY_HERO), 'hero');

	press('ArrowDown');

	expect(invokeMock).not.toHaveBeenCalled();
});
