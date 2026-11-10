// add-gui-library-panel row 12.1: the Search field's front end. The typing
// sends one debounced `searchQuery` per settled query (the injected scheduler
// stands in for the timer, so no test sleeps), Esc closes the field with the
// `searchClose` command, and the highlight works on row ids, never object
// identity, so a new results frame keeps the held id. The Tauri IPC is
// mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs (row 12.1).

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSearchFrame, GuiSearchRow } from '$lib/bridge/types';
import {
	activateSearchRow,
	blurSearchField,
	closeLibrarySearch,
	focusLibrarySearch,
	initialLibrarySearch,
	librarySearch,
	moveSearchHighlight,
	setSearchQuery,
	setSearchScheduler,
} from './search';

const invokeMock = vi.mocked(invoke);

/** An injected scheduler that holds the pending send: `flush()` runs it, and a
 * later injection releases it, which is exactly what the real timer's cancel
 * does. */
function recordedScheduler() {
	let pending: (() => void) | null = null;
	let canceled = 0;
	const scheduler = (send: () => void) => {
		pending = send;
		return {
			cancel: () => {
				pending = null;
				canceled += 1;
			},
		};
	};
	return {
		scheduler,
		flush: () => {
			const send = pending;
			pending = null;
			send?.();
		},
		cancelCount: () => canceled,
	};
}

/** A results frame whose rows are fresh objects on every call: two frames
 * built from the same ids share no row object, so an id, not an identity,
 * is all the movement can match on. */
function results(ids: string[]): GuiSearchFrame {
	const rows: GuiSearchRow[] = ids.map((id) => ({
		id,
		itemType: 'Artist',
		title: `Title ${id}`,
		subtitle: 'Artist',
	}));
	return { query: 'q', loading: false, failed: false, results: rows };
}

beforeEach(() => {
	invokeMock.mockClear();
	setSearchScheduler(recordedScheduler().scheduler);
	librarySearch.set(initialLibrarySearch());
});

test('the settled typing sends one debounced query', () => {
	const timer = recordedScheduler();
	setSearchScheduler(timer.scheduler);

	setSearchQuery('ba');
	expect(invokeMock).not.toHaveBeenCalled();

	setSearchQuery('bat');
	timer.flush();

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchQuery', query: 'bat' },
	});
});

test('a fresh keystroke replaces the pending send, never stacks one', () => {
	const timer = recordedScheduler();
	setSearchScheduler(timer.scheduler);

	setSearchQuery('ba');
	setSearchQuery('bat');
	timer.flush();

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchQuery', query: 'bat' },
	});
});

test('an emptied field clears the results at once, with no debounce', () => {
	setSearchQuery('');

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchQuery', query: '' },
	});
});

test('Esc closes the field, clears it, and sends the close command', () => {
	const timer = recordedScheduler();
	setSearchScheduler(timer.scheduler);
	focusLibrarySearch();
	setSearchQuery('bat');
	timer.flush();
	invokeMock.mockClear();

	closeLibrarySearch();

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchClose' },
	});
	expect(get(librarySearch)).toEqual(initialLibrarySearch());
});

test('a close cancels the pending send with it', () => {
	const timer = recordedScheduler();
	setSearchScheduler(timer.scheduler);
	setSearchQuery('bat');
	const cancels = timer.cancelCount();

	closeLibrarySearch();

	expect(timer.cancelCount()).toBe(cancels + 1);
});

test('the arrows move the highlight along held row ids', () => {
	const search = results(['r1', 'r2', 'r3']);
	moveSearchHighlight(1, search);
	expect(get(librarySearch).highlight).toBe('r1');
	moveSearchHighlight(1, search);
	expect(get(librarySearch).highlight).toBe('r2');
});

test('the highlight moves by id through a new frame whose rows share no object', () => {
	// The row-object identity lesson (row 12.1): a new results frame's rows
	// are fresh objects, so the movement must find the held id's place by id
	// and keep walking from there.
	moveSearchHighlight(1, results(['r1', 'r2']));
	moveSearchHighlight(1, results(['r1', 'r2']));
	expect(get(librarySearch).highlight).toBe('r2');
});

test('an arrow with no results moves nothing and holds no id', () => {
	moveSearchHighlight(1, results([]));
	expect(get(librarySearch).highlight).toBeNull();
});

test('activating a row sends its id and item type, never the close command', () => {
	activateSearchRow({
		id: 'r7',
		itemType: 'Artist',
		title: 'Artist',
		subtitle: '1 album',
	});

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchActivate', id: 'r7', itemType: 'Artist' },
	});
});

test('a field blur retracts the keyboard without closing anything', () => {
	focusLibrarySearch();
	blurSearchField();
	expect(get(librarySearch).focused).toBe(false);
});
