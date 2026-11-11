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
	unmountSearchField,
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
		},
		home: null,
		shows: null,
		emby: null,
		audiobooks: null,
		podcasts: null,
		feeds: null,
		rowMenu: null,
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

/** Arm the field's keyboard the way the mounted top bar's focus event does: the
** mount fact and the focus fact hold together (design D3, step 2, row 12.1).
*/
function focusField(): void {
	librarySearch.update((s) => (s.mounted ? s : { ...s, mounted: true }));
	focusLibrarySearch();
}

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
	// A mounted top bar paints the field (row 12.1): the store holds its fact.
	librarySearch.set({ ...initialLibrarySearch(), mounted: true });
	const outcome = routeLibraryKey(keyEvent('k', { ctrlKey: true }), shownSearch(null));

	expect(outcome).toBe('handled');
	expect(get(librarySearch).focused).toBe(true);
});

test('Ctrl+K focuses the Search field while the Queue panel holds Panel focus', () => {
	// The chord is a window key (design D3, step 3): it is not a Library
	// panel-focus key, so Queue Panel focus does not keep it from the field.
	librarySearch.set({ ...initialLibrarySearch(), mounted: true });
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

test('Ctrl+K answers the core while no top bar mounts the Search field', () => {
	// Review of row 12.1, finding P1: the chord is a window key, but the top
	// bar mounts the field only where the arrangement paints the bar — in Mini
	// with Queue focus the panel shows none, so arming the focus fact there
	// would leave the keyboard on a field that holds no element. The chord
	// keeps its core effect (`[keys]`) until the bar mounts.
	librarySearch.set({ ...initialLibrarySearch(), mounted: false });
	const outcome = routeLibraryKey(keyEvent('k', { ctrlKey: true }), shownSearch(null));

	expect(outcome).toBe('core');
	expect(get(librarySearch).focused).toBe(false);
});

test('a key meets the core after the top bar unmounts under the armed field', () => {
	// Review of row 12.1, finding P1: the unmounted top bar retracts the focus
	// fact with its mount fact, so the routing gate never traps the keyboard
	// on the retracted field.
	librarySearch.set({ ...initialLibrarySearch(), mounted: true, focused: true });
	unmountSearchField();
	const outcome = routeLibraryKey(keyEvent('b'), shownSearch(results()));

	expect(outcome).toBe('core');
	expect(get(librarySearch).focused).toBe(false);
});

test('the field takes its typing without a core forward (the text answer)', () => {
	focusField();
	const outcome = routeLibraryKey(keyEvent('b'), shownSearch(results()));

	expect(outcome).toBe('text');
	// The forwarder sends the core nothing for a text answer.
	expect(invokeMock).not.toHaveBeenCalled();
});

test('Esc closes the results, clears the field, and sends searchClose', () => {
	focusField();
	const outcome = routeLibraryKey(keyEvent('Escape'), shownSearch(results()));

	expect(outcome).toBe('handled');
	expect(get(librarySearch)).toEqual(initialLibrarySearch());
	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchClose' },
	});
});

test('Down and Up move the highlight along the row ids', () => {
	focusField();
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
	focusField();
	const next = frame('library', {
		library: { revision: 2, search: results(), content: content() },
	});
	setSearchHighlight('r2');
	routeLibraryKey(keyEvent('ArrowDown'), next);
	expect(get(librarySearch).highlight).toBe('r3');
});

test('Enter on the highlighted row sends searchActivate and no close', () => {
	focusField();
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
	focusField();
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

test('Escape closes the carried results after the field lost the keyboard', () => {
	// Review of row 12.1, finding P2: the dropdown paints while the frame
	// carries the section, so Esc must close it even after a pointer takes
	// the keyboard elsewhere — the routeEscapeKey arm runs before the panel
	// keys and it is the only close the open results have.
	librarySearch.set({ ...initialLibrarySearch(), mounted: true });
	const outcome = routeLibraryKey(keyEvent('Escape'), shownSearch(results()));

	expect(outcome).toBe('handled');
	expect(get(librarySearch)).toEqual(initialLibrarySearch());
	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchClose' },
	});
});

test('a core overlay keeps the keyboard even from the focused Search field', () => {
	focusField();
	const overlay = frame('library', {
		library: { revision: 1, search: results(), content: content() },
		overlay: { kind: 'confirm' } as LibraryKeyFrame['overlay'],
	});
	const outcome = routeLibraryKey(keyEvent('Escape'), overlay);

	expect(outcome).toBe('core');
});
