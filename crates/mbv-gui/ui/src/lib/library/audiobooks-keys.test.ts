// add-gui-library-panel row 9.1: the Audiobooks keys (design D12) — the
// list's cursor moves, Enter plays the cursor book and `a` queues it, `n` is
// consumed and does nothing (books have no play-next), and the Hero pane's
// keys act through the chapter Workspace's own contract functions. The 7.1
// review's lesson holds: from the list pane and from a hero-less Hero pane,
// every Library key returns handled and none reaches the core. The workspace
// module is mocked here, so the Hero-pane assertions name the contract calls
// the router makes. The Tauri IPC is mocked for the list-pane commands; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
	GuiAudiobooksFrame,
	GuiAudiobooksRow,
	GuiLibraryContent,
	GuiPanel,
} from '$lib/bridge/types';
import { installInput } from '$lib/bridge/input';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import { audiobooksCursorRow, audiobooksUi, initialAudiobooksUi } from './audiobooks';
import { carryLibraryContent, initialLibraryUi, libraryUi } from './state';
import type { LibraryPane } from './state';
import { moveAbsChapterCursor, playAbsChapter, queueAbsBook } from './audiobooks-workspace';

vi.mock('./audiobooks-workspace', () => ({
	moveAbsChapterCursor: vi.fn(),
	playAbsChapter: vi.fn(),
	queueAbsBook: vi.fn(),
	resetAbsChapterCursor: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const moveAbsChapterCursorMock = vi.mocked(moveAbsChapterCursor);
const playAbsChapterMock = vi.mocked(playAbsChapter);
const queueAbsBookMock = vi.mocked(queueAbsBook);

beforeEach(() => {
	invokeMock.mockClear();
	moveAbsChapterCursorMock.mockClear();
	playAbsChapterMock.mockClear();
	queueAbsBookMock.mockClear();
	libraryUi.set(initialLibraryUi());
	audiobooksUi.set(initialAudiobooksUi());
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

/** The catalog's book rows: two books under `A` and one under `B`. */
const CATALOG: GuiAudiobooksRow[] = [
	{ id: 'b1', title: 'Aftermath', subtitle: 'Bill Wyman', duration: '4:32', sortLetter: 'A' },
	{ id: 'b2', title: 'Audition', subtitle: 'Ryu Murakami', duration: '2:14', sortLetter: 'A' },
	{ id: 'b3', title: 'Blindsight', subtitle: 'Peter Watts', duration: '10:02', sortLetter: 'B' },
];

/** The carried Audiobooks frame: the catalog's first book selected. */
const AUIB_FRAME: GuiAudiobooksFrame = {
	libIdx: 4,
	loading: false,
	failed: false,
	workspaceLoading: false,
	rows: CATALOG,
	selected: 'b1',
	hero: null,
};

/** The carried Audiobooks content with a Hero on `hero`, and no Hero on
 * `null`. */
function content(hero: GuiAudiobooksFrame['hero']): GuiLibraryContent {
	return {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 4, label: 'Audiobooks' },
		],
		currentTab: 4,
		music: null,
		home: null,
		shows: null,
		emby: null,
		audiobooks: { ...AUIB_FRAME, hero },
		podcasts: null,
		feeds: null,
		rowMenu: null,
	};
}

/** The selected book's Hero facts with its Workspace's chapter rows. */
const AUIB_HERO: NonNullable<GuiAudiobooksFrame['hero']> = {
	title: 'Aftermath',
	metaRows: ['Bill Wyman', '4:32'],
	durationRow: 0,
	progressRow: null,
	overview: null,
	artShape: 'portrait',
	art: null,
	workspace: { rows: [{ chapter: 1, title: 'Act one', duration: '2:16', current: false }] },
};

/** Carry the content, focus the pane, and paint the list's cursor. */
function carry(pane: LibraryPane, cursor: string | null = null): void {
	const carried = content(pane === 'hero' ? AUIB_HERO : heroFor(pane));
	carryLibraryContent({ revision: 1, search: null, content: carried });
	libraryUi.update((ui) => ({ ...ui, pane }));
	audiobooksUi.set({ ...initialAudiobooksUi(), cursor });
}

/** A hero-less pane still carries no Hero: the Upcoming-like frame. */
function heroFor(pane: LibraryPane): GuiAudiobooksFrame['hero'] {
	return pane === 'hero' ? AUIB_HERO : null;
}

test('Enter on the cursor book plays it from its saved position', () => {
	carry('list', 'b2');

	press('Enter');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlay', libIdx: 4, itemId: 'b2' },
	});
});

test('a movement key moves the book cursor and sends no key', () => {
	carry('list', 'b1');

	press('ArrowDown');

	expect(audiobooksCursorRow()?.id).toBe('b2');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('a on the list pane queues the cursor book', () => {
	carry('list', 'b3');

	press('a');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absQueue', libIdx: 4, itemId: 'b3' },
	});
});

// Books have no play-next: `n` is consumed, so no command leaves and no key
// reaches the core (design D12, row 9.1).
test('n on the list pane does nothing and is handled', () => {
	carry('list', 'b2');

	press('n');

	expect(invokeMock).not.toHaveBeenCalled();
});

// `/` opens the list's filter under Library focus (design D3, D7): the
// rail's keyboard path on the plain list, and the Queue filter never sees
// the key.
test('/ opens the Audiobooks filter and sends no key', () => {
	carry('list', 'b1');

	press('/');

	expect(get(libraryUi).filterOpen).toBe(true);
	expect(invokeMock).not.toHaveBeenCalled();
});

// Esc closes the open filter (design D3, step 2): its text goes with it,
// and the close runs the Audiobooks list's own resolve, not another
// destination's.
test('Esc closes the Audiobooks filter and drops its text', () => {
	carry('list');
	audiobooksUi.set({ ...initialAudiobooksUi(), filter: 'b1' });
	libraryUi.set({ ...initialLibraryUi(), filterOpen: true });

	press('Escape');

	expect(get(libraryUi).filterOpen).toBe(false);
	expect(get(audiobooksUi).filter).toBe('');
	expect(invokeMock).not.toHaveBeenCalled();
});

// The Hero pane's keys act through the Workspace's own contract functions
// (design D12): the movements move the chapter cursor, Enter plays the
// chapter, and `a` queues the selected book.
test('the Hero pane moves the chapter cursor through the Workspace', () => {
	carry('hero', 'b1');

	press('ArrowDown');

	expect(moveAbsChapterCursorMock).toHaveBeenCalledWith('down', expect.anything());
	expect(playAbsChapterMock).not.toHaveBeenCalled();
});

test('the Hero pane plays the cursor chapter on Enter', () => {
	carry('hero', 'b1');

	press('Enter');

	expect(playAbsChapterMock).toHaveBeenCalledTimes(1);
});

test('the Hero pane queues the selected book on a', () => {
	carry('hero', 'b1');

	press('a');

	expect(queueAbsBookMock).toHaveBeenCalledTimes(1);
});

test('the Hero pane consumes n without a Workspace call', () => {
	carry('hero', 'b1');

	press('n');

	expect(invokeMock).not.toHaveBeenCalled();
	expect(playAbsChapterMock).not.toHaveBeenCalled();
});

// Review of row 7.1's lesson, applied to Audiobooks from the start: no pane
// leaks a Library key to the core's router — the list pane and a hero-less
// Hero pane included. A pane that does not act on a key still consumes it, so
// `a` and `n` never reach the core's playback chords, which are not gated on
// Panel focus.
test('the list pane and a hero-less Hero pane consume every Library key', () => {
	carry('list');
	const list = LIBRARY_KEYS.map((key) => routeLibraryKey(new KeyboardEvent('keydown', { key }), frame()));
	libraryUi.update((ui) => ({ ...ui, pane: 'hero' }));
	carryLibraryContent({ revision: 1, search: null, content: content(null) });
	const hero = LIBRARY_KEYS.map((key) =>
		routeLibraryKey(new KeyboardEvent('keydown', { key }), frame()),
	);

	expect(list).toEqual(LIBRARY_KEYS.map(() => 'handled'));
	expect(hero).toEqual(LIBRARY_KEYS.map(() => 'handled'));
});

// The pane walk skips the panes the frame does not paint (row 9.1): the
// destination paints no pill row, so the list is the walk's first pane, and a
// frame without a Hero leaves the list the last one.
test('Shift+Tab before the list takes Queue Panel focus', () => {
	carry('list');

	press('Tab', { shiftKey: true });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab past the list without a Hero takes Queue Panel focus', () => {
	carry('list');

	press('Tab');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeQueueFocus' },
	});
});

test('Tab walks from the list into the Hero', () => {
	carry('list');
	carryLibraryContent({ revision: 1, search: null, content: content(AUIB_HERO) });

	press('Tab');

	expect(get(libraryUi).pane).toBe('hero');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Tab from the Queue panel focuses the list, not an absent pill row', () => {
	carry('list');

	press('Tab', {}, frame('queue'));

	expect(get(libraryUi).pane).toBe('list');
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
});
