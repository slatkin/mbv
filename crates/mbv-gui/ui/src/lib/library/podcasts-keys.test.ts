// add-gui-library-panel row 10.1: the Podcasts keys (design D12) — the list's
// cursor moves, Enter plays the episode it rests on, `a` queues it, `n` is
// consumed and does nothing, the pill row walks its views, and the pane walk
// skips the Hero a frame without one does not paint. The router under test is
// the panel's one routing site, `routeLibraryKey`, so these assertions cover
// the Podcasts dispatch it hands to `./podcasts-keys`. The Tauri IPC is
// mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {
	GuiLibraryContent,
	GuiPanel,
	GuiPodcastsFrame,
	GuiPodcastsPill,
} from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import {
	initialPodcastsUi,
	podcastPillKey,
	podcastsCursorRow,
	podcastsUi,
} from './podcasts';
import { carryLibraryContent, initialLibraryUi, libraryUi } from './state';
import type { LibraryPane } from './state';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockClear();
	libraryUi.set(initialLibraryUi());
	podcastsUi.set(initialPodcastsUi());
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

/** The episode rows the Latest pill merges, newest first. */
const EPISODES = [
	{ id: 'e1:ep1', title: 'Episode A', subtitle: 'Show One', date: '12 May', state: 'ordinary' as const },
	{ id: 'e2:ep2', title: 'Episode B', subtitle: 'Show Two', date: '9 May', state: 'played' as const },
];

/** The Latest pill and two show pills, with the frame carrying `selected`
 * (design D12). */
function pills(selected: 'latest' | 'show'): GuiPodcastsPill[] {
	return [
		{
			pill: { kind: 'latest' },
			name: 'Latest',
			selected: selected === 'latest',
			collection: false,
		},
		{
			pill: { kind: 'show', id: 'sh1' },
			name: 'Show One',
			selected: selected === 'show',
			collection: false,
		},
		{
			pill: { kind: 'show', id: 'sh2' },
			name: 'Show Two',
			selected: false,
			collection: false,
		},
	];
}

/** The episode Hero a selected episode paints (design D12, row 10.1). */
const HERO: NonNullable<GuiPodcastsFrame['hero']> = {
	title: 'Episode A',
	metaRows: ['Show One', '34:00'],
	durationRow: 1,
	progressRow: null,
	overview: null,
	artShape: 'square',
	art: null,
};

/** The carried content while Podcasts is active (design D12): no other
 * destination's content, the pill row, and `hero` the caller picks. */
function podcastsContent(
	pillSelection: 'latest' | 'show',
	rows: typeof EPISODES,
	hero: NonNullable<GuiPodcastsFrame['hero']> | null = null,
	selected: string | null = rows[0]?.id ?? null,
): GuiLibraryContent {
	return {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 2, label: 'Podcasts' },
		],
		currentTab: 2,
		music: null,
		home: null,
		emby: null,
		audiobooks: null,
		podcasts: {
			libIdx: 0,
			pills: pills(pillSelection),
			loading: false,
			failed: false,
			rows,
			selected,
			hero,
		},
		shows: null,
	};
}

/** Carry the Podcasts content and focus the pane the key acts on. */
function carryPodcasts(
	content: GuiLibraryContent,
	pane: LibraryPane,
	cursor: string | null = null,
): void {
	carryLibraryContent({ revision: 1, content });
	libraryUi.update((ui) => ({ ...ui, pane }));
	podcastsUi.set({ ...initialPodcastsUi(), cursor });
}

test('Enter on the cursor row plays the episode', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list', 'e2:ep2');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsPlay', libIdx: 0, itemId: 'e2:ep2' },
	});
});

test('a movement key moves the cursor and sends no key', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list', 'e1:ep1');

	press('ArrowDown');

	expect(podcastsCursorRow()?.row.id).toBe('e2:ep2');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('a on the cursor row queues the episode', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list', 'e1:ep1');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsQueue', libIdx: 0, itemId: 'e1:ep1' },
	});
});

// A podcast episode has no play-next (row 10.1): `n` is consumed and sends
// nothing, so it neither queues nor reaches the core's playback chords.
test('n on the cursor row sends nothing and is handled', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list', 'e1:ep1');

	const event = press('n');

	expect(invokeMock).not.toHaveBeenCalled();
	expect(event.defaultPrevented).toBe(true);
});

test('Right on the pill row focuses the first pill and sends no key', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'pills');

	press('ArrowRight');

	const focused = get(podcastsUi).focusedPill;
	expect(focused !== null && podcastPillKey(focused)).toBe('latest');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space on a focused show pill asks for its selection', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'pills');
	podcastsUi.update((ui) => ({ ...ui, focusedPill: { kind: 'show', id: 'sh1' } }));

	press(' ');

	expect(get(podcastsUi).pillSelect).toEqual({ kind: 'show', id: 'sh1' });
	expect(invokeMock).not.toHaveBeenCalled();
});

// Review of row 10.1, finding P2: the pill walk finds the focused pill by
// its key, not its object, because every carried frame is a fresh
// deserialization. An object-identity find would lose the focus across
// frames and the walk would restart from a row end.
test('the pill walk keeps its place when the frame re-carries equal pills', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'pills');
	podcastsUi.update((ui) => ({ ...ui, focusedPill: { kind: 'show', id: 'sh1' } }));

	// A new frame: structurally equal pills, all fresh objects.
	carryLibraryContent({ revision: 2, content: podcastsContent('show', EPISODES) });

	press('ArrowRight');

	expect(get(podcastsUi).focusedPill).toEqual({ kind: 'show', id: 'sh2' });
});

test('Enter on a focused pill asks for its selection', () => {
	carryPodcasts(podcastsContent('show', EPISODES), 'pills');

	press('ArrowRight');
	press('Enter');

	expect(get(podcastsUi).pillSelect).toEqual({ kind: 'latest' });
	expect(invokeMock).not.toHaveBeenCalled();
});

test('/ keeps its core effect, so the filter field never opens', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list', 'e1:ep1');

	press('/');

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: '/' }),
	});
});

test('a in the Hero pane queues the frame\'s selected episode', () => {
	carryPodcasts(podcastsContent('latest', EPISODES, HERO), 'hero');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsQueue', libIdx: 0, itemId: 'e1:ep1' },
	});
});

test('a movement key in the Hero pane changes nothing and sends no key', () => {
	carryPodcasts(podcastsContent('latest', EPISODES, HERO), 'hero');

	press('ArrowDown');

	expect(invokeMock).not.toHaveBeenCalled();
});

// Review of row 7.1's P1 lesson, row 10.1: the Podcasts panes keep the
// Library's keys from the core's router (design D3, step 4). A pane that does
// not act on a key still consumes it, so `n` and `a` never reach the core's
// playback chords, which are not gated on Panel focus.
test('a movement key on the pills pane does not reach the core', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'pills');

	press('ArrowUp');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('a on the pills pane queues the list cursor episode and sends no key', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'pills', 'e2:ep2');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsQueue', libIdx: 0, itemId: 'e2:ep2' },
	});
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

test('n on the pills pane is consumed and sends no key', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'pills', 'e2:ep2');

	press('n');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('n on the hero-less Hero pane is consumed and sends no key', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'hero', 'e1:ep1');

	press('n');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space with no focused pill keeps its core effect', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list', 'e1:ep1');

	press(' ');

	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: expect.objectContaining({ key: ' ' }),
	});
});

// The pane walk skips the Hero a frame without one does not paint (design
// D12, row 10.1): the list is the walk's last pane there, so Tab past it
// takes Queue Panel focus.
test('Tab past the hero-less list takes Queue Panel focus', () => {
	carryPodcasts(podcastsContent('latest', EPISODES), 'list');

	press('Tab');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab walks from the list into the Hero a frame carries', () => {
	carryPodcasts(podcastsContent('latest', EPISODES, HERO), 'list');

	press('Tab');

	expect(get(libraryUi).pane).toBe('hero');
	expect(invokeMock).not.toHaveBeenCalled();
});
