// add-gui-library-panel row 12.1: the Search field's keys route through the
// Library key router (design D3). `Ctrl+K` focuses the field wherever the
// Library panel shows, the field's typing answers `text` so no key is stolen
// from it or sent to the core, Esc closes the field with the `searchClose`
// command, and the arrows and Enter act on the results. While the field does
// not hold the keyboard none of its handlers fire (the key-leak rule): the
// keys go where the panel's other keys go. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
	GuiLibraryContent,
	GuiPanel,
	GuiSearchFrame,
	GuiSearchRow,
} from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import {
	carryLibraryContent,
	initialLibraryUi,
	libraryUi,
} from './state';
import {
	focusLibrarySearch,
	initialLibrarySearch,
	librarySearch,
	setSearchHighlight,
} from './search';
import type { LibrarySearchState } from './search';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockClear();
	libraryUi.set(initialLibraryUi());
	librarySearch.set(initialLibrarySearch());
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
		library: { revision: 1, search: null, content: null },
		...rest,
	};
}

/** A carried Music content: the panel's presence fact needs one active
 * destination's section, and the pills go unused here. */
function content(): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: {
			libIdx: 0,
			pills: [{ groupId: 'g1', name: 'Rock', selected: true, collection: true }],
			collectionNames: ['Rock'],
			loading: false,
			failure: null,
			artists: [],
			hero: null,
			art: null,
			playingTrackId: null,
			rowMenu: null,
		},
		home: null,
		shows: null,
		emby: null,
		audiobooks: null,
		podcasts: null,
		feeds: null,
	};
}

/** Panel shown with the carried content, and `search` on the frame. */
function shownSearch(search: GuiSearchFrame | null): LibraryKeyFrame {
	carryLibraryContent({ revision: 1, search: null, content: content() });
	return frame('library', { library: { revision: 1, search, content: content() } });
}

const ROWS: GuiSearchRow[] = [
	{ id: 'r1', itemType: 'Artist', title: 'Beastie Boys', subtitle: 'Artist' },
	{ id: 'r2', itemType: 'Artist', title: 'Brand Nubian', subtitle: 'Artist' },
	{ id: 'r3', itemType: 'Album', title: 'Licensed To Ill', subtitle: 'Beastie Boys' },
];

/** The results frame the core carries. */
function results(): GuiSearchFrame {
	return {
		query: 'ba',
		loading: false,
		failed: false,
		results: ROWS,
	};
}

/** A keydown as the webview's event carries it. */
function keyEvent(
	key: string,
	rest: Partial<KeyboardEventInit> = {},
): KeyboardEvent {
	return new KeyboardEvent('keydown', { key, bubbles: true, ...rest });
}

test('Ctrl+K focuses the Search field while the Library panel shows', () => {
	const outcome = routeLibraryKey(keyEvent('k', { ctrlKey: true }), shownSearch(null));

	expect(outcome).toBe('handled');
	expect(get(librarySearch).focused).toBe(true);
});

test('Ctrl+K focuses the Search field while the Queue panel holds Panel focus', () => {
	// The chord is a window key (design D3, step 3): it is not a Library
	// panel-focus key, so Queue Panel focus does not keep it from the field.
	const outcome = routeLibraryKey(
		keyEvent('k', { ctrlKey: true }),
		shownSearch(null),
	);

	expect(outcome).toBe('handled');
	expect(get(librarySearch).focused).toBe(true);
});

test('Ctrl+K out of a Queue panel frame sends the key to the core instead', () => {
	// No Library section in the frame: the panel paints nothing and its keys
	// are unwired, so the core's own Ctrl+K (`[keys]`) keeps the key.
	const outcome = routeLibraryKey(keyEvent('k', { ctrlKey: true }), {
		...frame('queue'),
		library: null,
	});

	expect(outcome).toBe('core');
});

test('the field takes its typing without a core forward (the text answer)', () => {
	focusLibrarySearch();
	const outcome = routeLibraryKey(keyEvent('b'), shownSearch(results()));

	expect(outcome).toBe('text');
	// The forwarder sends the core nothing for a text answer.
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Esc closes the results, clears the field, and sends searchClose', () => {
	focusLibrarySearch();
	const outcome = routeLibraryKey(keyEvent('Escape'), shownSearch(results()));

	expect(outcome).toBe('handled');
	expect(get(librarySearch)).toEqual(initialLibrarySearch());
	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchClose' },
	});
});

test('Down and Up move the highlight along the row ids', () => {
	focusLibrarySearch();
	const f = shownSearch(results());
	setSearchHighlight('r1');
	routeLibraryKey(keyEvent('ArrowDown'), f);
	expect(get(librarySearch).highlight).toBe('r2');
	routeLibraryKey(keyEvent('ArrowUp'), f);
	expect(get(librarySearch).highlight).toBe('r1');
});

test('the highlight movement walks a new frame by id, not object identity', () => {
	// The key-leak lesson's identity half (row 12.1): a refresh's results are
	// fresh row objects, so the movement must place the held id inside the new
	// frame by id — r2 moves to r3 there.
	focusLibrarySearch();
	const next = frame('library', {
		library: { revision: 2, search: results(), content: content() },
	});
	setSearchHighlight('r2');
	routeLibraryKey(keyEvent('ArrowDown'), next);
	expect(get(librarySearch).highlight).toBe('r3');
});

test('Enter on the highlighted row sends searchActivate and no close', () => {
	focusLibrarySearch();
	setSearchHighlight('r2');
	invokeMock.mockClear();
	const outcome = routeLibraryKey(keyEvent('Enter'), shownSearch(results()));

	expect(outcome).toBe('handled');
	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchActivate', id: 'r2', itemType: 'Artist' },
	});
});

test('Enter with no highlighted row sends nothing', () => {
	focusLibrarySearch();
	const outcome = routeLibraryKey(keyEvent('Enter'), shownSearch(results()));

	expect(outcome).toBe('handled');
	expect(invokeMock).not.toHaveBeenCalled();
});

test('no search key acts while the field does not hold the keyboard', () => {
	const f = shownSearch(results());
	// The list pane's own Down keeps the list cursor's job; the Search state
	// holds no highlight and stays unfocused.
	const before: LibrarySearchState = get(librarySearch);
	routeLibraryKey(keyEvent('ArrowDown'), f);

	expect(get(librarySearch)).toEqual(before);
});

test('a core overlay keeps the keyboard even from the focused Search field', () => {
	focusLibrarySearch();
	const overlay = frame('library', {
		library: { revision: 1, search: results(), content: content() },
		overlay: { kind: 'confirm' } as LibraryKeyFrame['overlay'],
	});
	const outcome = routeLibraryKey(keyEvent('Escape'), overlay);

	expect(outcome).toBe('core');
});
