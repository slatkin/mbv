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
	GuiMusicPill,
	GuiPanel,
} from '$lib/bridge/types';
import { routeLibraryKey } from './keys';
import type { LibraryKeyFrame } from './keys';
import {
	carryLibraryContent,
	initialLibraryUi,
	libraryUi,
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

/** The carried Music content: two group pills, one selected. */
function content(): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: {
			libIdx: 0,
			pills,
			loading: false,
			failure: null,
			artists: [],
			hero: null,
			art: null,
		},
	};
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
