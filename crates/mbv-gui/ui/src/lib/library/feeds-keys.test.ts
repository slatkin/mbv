// add-gui-library-panel row 11.1: the Feeds keys (design D12) — the list's
// cursor moves, Enter plays the entry it rests on, `a` queues it, `n` is
// consumed and does nothing, the pill row walks its views, and the pane walk
// skips the Hero a frame without one does not paint. The router under test is
// the panel's one routing site, `routeLibraryKey`, so these assertions cover
// the Feeds dispatch it hands to `./feeds-keys`. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type { GuiFeedsFrame, GuiLibraryContent, GuiPanel } from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import { feedsPillKey, feedsCursorRow, feedsUi, initialFeedsUi } from './feeds';
import { carryLibraryContent, initialLibraryUi, libraryUi } from './state';
import type { LibraryPane } from './state';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockClear();
	libraryUi.set(initialLibraryUi());
	feedsUi.set(initialFeedsUi());
	carryLibraryContent(null);
});

/** A slice frame's routing facts (design D3) with the Library panel shown. */
function frame(panel: GuiPanel = 'library'): LibraryKeyFrame {
	return {
		focus: { surface: panel, panel },
		overlay: null,
		status: { prefixArmed: false, toast: null },
		summary: { filterActive: false },
		settings: null,
		library: { revision: 1, search: null, content: null },
	};
}

/** Dispatch one keydown through the installed forwarder, its router reading
 * the frame the press is routed with. */
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

/** The entry rows the Latest pill merges, newest first. */
const ENTRIES = [
	{
		id: 's1::e1',
		title: 'Entry A',
		subtitle: 'Sub One',
		date: '12 May',
		state: 'ordinary' as const,
	},
	{
		id: 's2::e2',
		title: 'Entry B',
		subtitle: 'Sub Two',
		date: '9 May',
		state: 'played' as const,
	},
];

/** The Latest pill and two subscription pills, with the frame carrying
 * `selected` (design D12). */
function pills(selected: 'latest' | 'subscription'): GuiFeedsFrame['pills'] {
	return [
		{
			pill: { kind: 'latest' },
			name: 'Latest',
			selected: selected === 'latest',
			collection: false,
		},
		{
			pill: { kind: 'subscription', id: 'https://sub.one' },
			name: 'Sub One',
			selected: selected === 'subscription',
			collection: false,
		},
		{
			pill: { kind: 'subscription', id: 'https://sub.two' },
			name: 'Sub Two',
			selected: false,
			collection: false,
		},
	];
}

/** The entry Hero a selected entry paints (design D12, row 11.1). */
const HERO: NonNullable<GuiFeedsFrame['hero']> = {
	title: 'Entry A',
	metaRows: ['Sub One', '12:30'],
	durationRow: 1,
	progressRow: null,
	artShape: 'square',
};

/** The carried content while the Feeds destination is active (design D12):
 * no other destination's content, the pill row, and `hero` the caller
 * picks. */
function feedsContent(
	pillSelection: 'latest' | 'subscription',
	rows: typeof ENTRIES,
	hero: NonNullable<GuiFeedsFrame['hero']> | null = null,
	selected: string | null = rows[0]?.id ?? null,
): GuiLibraryContent {
	return {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 2, label: 'Feeds' },
		],
		currentTab: 2,
		music: null,
		home: null,
		emby: null,
		audiobooks: null,
		podcasts: null,
		feeds: {
			pills: pills(pillSelection),
			loading: false,
			failed: false,
			rows,
			selected,
			hero,
		},
		rowMenu: null,
		shows: null,
	};
}

/** Carry the Feeds content and focus the pane the key acts on. */
function carryFeeds(
	content: GuiLibraryContent,
	pane: LibraryPane,
	cursor: string | null = null,
): void {
	carryLibraryContent({ revision: 1, search: null, content });
	libraryUi.update((ui) => ({ ...ui, pane }));
	feedsUi.set({ ...initialFeedsUi(), cursor });
}

test('Enter on the cursor row plays the entry', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's2::e2');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsPlay', itemId: 's2::e2' },
	});
});

test('a movement key moves the cursor and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's1::e1');

	press('ArrowDown');

	expect(feedsCursorRow()?.id).toBe('s2::e2');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('a on the cursor row queues the entry', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's1::e1');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsQueue', itemId: 's1::e1' },
	});
});

// A feed entry has no play-next (row 11.1): `n` is consumed and sends
// nothing, so it neither queues nor reaches the core's playback chords.
test('n on the cursor row sends nothing and is handled', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's1::e1');

	const event = press('n');

	expect(invokeMock).not.toHaveBeenCalled();
	expect(event.defaultPrevented).toBe(true);
});

test('Right on the pill row focuses the first pill and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'pills');

	press('ArrowRight');

	const focused = get(feedsUi).focusedPill;
	expect(focused !== null && feedsPillKey(focused)).toBe('latest');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space on a focused subscription pill asks for its selection', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'pills');
	feedsUi.update((ui) => ({ ...ui, focusedPill: { kind: 'subscription', id: 'https://sub.one' } }));

	press(' ');

	expect(get(feedsUi).pillSelect).toEqual({
		kind: 'subscription',
		id: 'https://sub.one',
	});
	expect(invokeMock).not.toHaveBeenCalled();
});

// Review of row 10.1, finding P2: the pill walk finds the focused pill by
// its key, not its object, because every carried frame is a fresh
// deserialization. An object-identity find would lose the focus across
// frames and the walk would restart from a row end.
test('the pill walk keeps its place when the frame re-carries equal pills', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'pills');
	feedsUi.update((ui) => ({ ...ui, focusedPill: { kind: 'subscription', id: 'https://sub.one' } }));

	// A new frame: structurally equal pills, all fresh objects.
	carryLibraryContent({ revision: 2, search: null, content: feedsContent('subscription', ENTRIES) });

	press('ArrowRight');

	expect(get(feedsUi).focusedPill).toEqual({
		kind: 'subscription',
		id: 'https://sub.two',
	});
});

test('Enter on a focused pill asks for its selection', () => {
	carryFeeds(feedsContent('subscription', ENTRIES), 'pills');

	press('ArrowRight');
	press('Enter');

	expect(get(feedsUi).pillSelect).toEqual({ kind: 'latest' });
	expect(invokeMock).not.toHaveBeenCalled();
});

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
test('/ opens the Feeds filter and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's1::e1');

	press('/');

	expect(get(libraryUi).filterOpen).toBe(true);
	expect(invokeMock).not.toHaveBeenCalled();
});

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Feeds list's own resolve, not another
// destination's.
test('Esc closes the Feeds filter and drops its text', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's1::e1');
	feedsUi.set({ ...initialFeedsUi(), filter: 'e1' });
	libraryUi.set({ ...initialLibraryUi(), filterOpen: true });

	press('Escape');

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(get(feedsUi).filter).toBe('');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('a in the Hero pane queues the frame\'s selected entry', () => {
	carryFeeds(feedsContent('latest', ENTRIES, HERO), 'hero');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsQueue', itemId: 's1::e1' },
	});
});

test('a movement key in the Hero pane changes nothing and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES, HERO), 'hero');

	press('ArrowDown');

	expect(invokeMock).not.toHaveBeenCalled();
});

// Review of row 7.1's P1 lesson, row 11.1: the Feeds panes keep the
// Library's keys from the core's router (design D3, step 4). A pane that does
// not act on a key still consumes it, so `n` and `a` never reach the core's
// playback chords, which are not gated on Panel focus.
test('a movement key on the pills pane does not reach the core', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'pills');

	press('ArrowUp');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('a on the pills pane queues the list cursor entry and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'pills', 's2::e2');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsQueue', itemId: 's2::e2' },
	});
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

test('n on the pills pane is consumed and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'pills', 's2::e2');

	press('n');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('n on the hero-less Hero pane is consumed and sends no key', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'hero', 's1::e1');

	press('n');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space with no focused pill keeps its core effect', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list', 's1::e1');

	press(' ');

	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: ' ' }),
	});
});

// The pane walk skips the Hero a frame without one does not paint (design
// D12, row 11.1): the list is the walk's last pane there, so Tab past it
// takes Queue Panel focus.
test('Tab past the hero-less list takes Queue Panel focus', () => {
	carryFeeds(feedsContent('latest', ENTRIES), 'list');

	press('Tab');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab walks from the list into the Hero a frame carries', () => {
	carryFeeds(feedsContent('latest', ENTRIES, HERO), 'list');

	press('Tab');

	expect(get(libraryUi).pane).toBe('hero');
	expect(invokeMock).not.toHaveBeenCalled();
});
