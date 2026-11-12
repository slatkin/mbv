// add-gui-library-panel row 7.1: the Shows keys (design D12) — the list's
// cursor moves, Enter selects a series row and plays a flat row, `n` and `a`
// act on a flat row only, the pill row walks its modes, `/` is not taken, and
// the pane walk skips the Hero the Upcoming mode does not paint. The router
// under test is the panel's one routing site, `routeLibraryKey`, so these
// assertions cover the Shows dispatch it hands to `./shows-keys`. The Tauri IPC
// is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {

	GuiPanel,
	GuiShowsHero,
	GuiShowsMode,
	GuiShowsPill,
	GuiShowsRow,
	GuiShowsSeries,
} from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import { initialShowsUi, showsCursorRow, showsUi } from './shows';
import { libraryList } from '$lib/bridge/sections';
import { carryLibrary } from '$lib/bridge/test-library';
import type { LibrarySections } from '$lib/bridge/test-library';
import { initialLibraryUi, libraryUi } from './state';
import type { LibraryPane } from './state';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockClear();
	libraryUi.set(initialLibraryUi());
	showsUi.set(initialShowsUi());
	carryLibrary(null);
});

/** A slice frame's routing facts (design D3) with the Library panel shown. */
function frame(panel: GuiPanel = 'library'): LibraryKeyFrame {
	return {
		focus: { surface: panel, panel },
		overlay: null,
		status: { prefixArmed: false, toast: null },
		summary: { filterActive: false },
		settings: null,
		libraryList: get(libraryList),
		search: null,
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

/** The series rows the All series mode's list moves through. */
const SHOWS_SERIES: GuiShowsRow[] = [
	{ id: 's1', title: 'Atlanta', subtitle: null, date: null, sortLetter: 'A' },
	{ id: 's2', title: 'The Bear', subtitle: null, date: null, sortLetter: 'B' },
];

/** The All series mode's tree (design D14): one series with a season whose
 * episodes have landed, and one with none. */
const SHOWS_TREE: GuiShowsSeries[] = [
	{
		id: 's1',
		title: 'Atlanta',
		sortLetter: 'A',
		expandable: true,
		seasons: [
			{
				id: 'se1',
				name: 'Season 1',
				episodes: [
					{ episodeId: 'ep1', number: 1, name: 'The Big Bang', duration: '24:00', state: 'ordinary' },
				],
			},
		],
	},
	{ id: 's2', title: 'The Bear', sortLetter: 'B', expandable: true, seasons: [] },
];

/** The flat newest-episode rows the Latest mode paints. */
const SHOWS_EPISODES: GuiShowsRow[] = [
	{ id: 'e1', title: 'Atlanta', subtitle: 'S3E4 · Sabrosito', date: '12 May', sortLetter: null },
	{ id: 'e2', title: 'The Bear', subtitle: 'S2E1 · Beef', date: '9 May', sortLetter: null },
];

/** The three view modes' pills, with `mode` selected (design D12). */
function showsPills(mode: GuiShowsMode): GuiShowsPill[] {
	return [
		{ mode: 'latest', name: 'Latest', selected: mode === 'latest', collection: false },
		{ mode: 'upcoming', name: 'Upcoming', selected: mode === 'upcoming', collection: false },
		{ mode: 'all', name: 'All series', selected: mode === 'all', collection: false },
	];
}

/** The carried content while Shows is active (design D12): no Music and no
 * Home content, the mode's rows, and the Hero the mode paints. */
function showsContent(
	mode: GuiShowsMode,
	rows: GuiShowsRow[],
	hero: GuiShowsHero | null = null,
): LibrarySections {
	return {
		list: {
			tabs: [
				{ position: 1, label: 'Music' },
				{ position: 2, label: 'Shows' },
			],
			currentTab: 2,
			destination: {
				kind: 'shows',
				libIdx: 0,
				pills: showsPills(mode),
				collectionNames: [mode],
				loading: false,
				series: mode === 'all' ? SHOWS_TREE : [],
				upcomingFailed: false,
				rows,
			},
		},
		detail: {
			kind: 'shows',
			workspaceLoading: false,
			selected: rows[0]?.id ?? null,
			hero,
		},
	};
}

/** The All series mode's Hero: a series whose Workspace the Hero pane walks. */
const SHOWS_HERO: GuiShowsHero = {
	title: 'Atlanta',
	metaRows: [[{ text: '4 seasons', kind: 'count' }]],
	durationRow: null,
	progressRow: null,
	overview: null,
	artShape: 'landscape',
	art: null,
	workspace: {
		seasons: [{ seasonId: 'se1', name: 'Season 1', selected: true }],
		selectedSeason: 'se1',
		episodes: [
			{ episodeId: 'ep1', number: 1, name: 'The Big Bang', duration: '24:00', state: 'ordinary' },
			{
				episodeId: 'ep2',
				number: 2,
				name: 'Streets on Fire',
				duration: '23:00',
				state: 'ordinary',
			},
		],
	},
};

/** Carry the Shows content and focus the pane the key acts on. */
function carryShows(
	content: LibrarySections,
	pane: LibraryPane,
	cursor: string | null = null,
): void {
	carryLibrary(content);
	libraryUi.update((ui) => ({ ...ui, pane }));
	showsUi.set({ ...initialShowsUi(), cursor });
}

test('Enter on a series row selects it for the Hero', () => {
	carryShows(showsContent('all', SHOWS_SERIES), 'list', 'shows-series:s1');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsSelect', libIdx: 0, row: 's1' },
	});
});

test('Enter on a flat row plays the episode', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'list', 'e2');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsPlay', libIdx: 0, itemId: 'e2' },
	});
});

// A series row is not playable (row 7.1): its `n` and `a` send nothing, so the
// keys never act on a row the core refuses.
test('n and a on a series row send nothing', () => {
	carryShows(showsContent('all', SHOWS_SERIES), 'list', 'shows-series:s1');

	press('n');
	press('a');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('n and a on a flat row play the episode next and queue it', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'list', 'e1');

	press('n');
	expect(invokeMock).toHaveBeenLastCalledWith('library', {
		command: { command: 'showsPlayNext', libIdx: 0, itemId: 'e1' },
	});

	press('a');
	expect(invokeMock).toHaveBeenLastCalledWith('library', {
		command: { command: 'showsQueue', libIdx: 0, itemId: 'e1' },
	});
});

test('a movement key moves the Shows cursor and sends no key', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'list', 'e1');

	press('ArrowDown');

	const row = showsCursorRow();
	expect(row?.kind === 'flat' ? row.row.id : null).toBe('e2');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Right on the Shows pill row focuses the first mode pill and sends no key', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'pills');

	press('ArrowRight');

	expect(get(showsUi).focusedPill).toBe('latest');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Space on a focused Shows pill asks for its mode and sends no key', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'pills');
	showsUi.update((ui) => ({ ...ui, focusedPill: 'upcoming' }));

	press(' ');

	expect(get(showsUi).pillSelect).toBe('upcoming');
	expect(invokeMock).not.toHaveBeenCalled();
});

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path, and the Queue filter never sees the key.
test('/ opens the Shows filter and sends no key', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'list', 'e1');

	press('/');

	expect(get(libraryUi).filterOpen).toBe(true);
	expect(invokeMock).not.toHaveBeenCalled();
});

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Shows list's own resolve, not another
// destination's.
test('Esc closes the Shows filter and drops its text', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'list', 'e1');
	showsUi.set({ ...initialShowsUi(), filter: 'e1' });
	libraryUi.set({ ...initialLibraryUi(), filterOpen: true });

	press('Escape');

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(get(showsUi).filter).toBe('');
	expect(invokeMock).not.toHaveBeenCalled();
});

// The pane walk skips the Hero the Upcoming mode does not paint (design D12):
// the list is the walk's last pane there, so Tab past it takes Queue Panel
// focus.
test('Tab past the Upcoming list takes Queue Panel focus', () => {
	carryShows(showsContent('upcoming', SHOWS_EPISODES), 'list');

	press('Tab');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab walks from the All series list into the series Hero', () => {
	carryShows(showsContent('all', SHOWS_SERIES, SHOWS_HERO), 'list');

	press('Tab');

	expect(get(libraryUi).pane).toBe('hero');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Tab from the Queue panel focuses the Shows pill row', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'list');

	press('Tab', {}, frame('queue'));

	expect(get(libraryUi).pane).toBe('pills');
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
});

// Review of row 7.1, finding P1: the Shows panes keep the Library's keys from
// the core's router (design D3, step 4). A pane that does not act on a key
// still consumes it, so `a` and `n` never reach the core's playback chords,
// which are not gated on Panel focus.
test('a movement key on the pills pane does not reach the core', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'pills');

	press('ArrowDown');

	expect(invokeMock).not.toHaveBeenCalled();
});

test('a on the pills pane queues the list cursor\u2019s flat row and sends no key', () => {
	carryShows(showsContent('latest', SHOWS_EPISODES), 'pills', 'e1');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsQueue', libIdx: 0, itemId: 'e1' },
	});
	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});

test('n on the hero-less Hero pane does not reach the core', () => {
	carryShows(showsContent('upcoming', SHOWS_EPISODES), 'hero', 'e2');

	press('n');

	expect(invokeMock).not.toHaveBeenCalledWith('key', expect.anything());
});
