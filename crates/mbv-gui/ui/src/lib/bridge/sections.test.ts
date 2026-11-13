// gui-change-driven-sections tasks 3.1: the section stores keep the batch
// contract — `applyBatch` writes only the sections the batch carries and
// skips a section whose revision the window holds, `connectSections` calls
// `sync` only after `listen` resolves, and a `theme` section writes the role
// colours into CSS custom properties (design D6). The Tauri event and IPC
// APIs are mocked; the delivered payloads are the serde wire shapes of
// bridge/sections.rs.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { playhead } from './playhead';
import {
	SECTIONS_EVENT,
	applyBatch,
	connectSections,
	focus,
	footer,
	playback,
	queue,
	resetSectionsForTest,
	summary,
} from './sections';
import type { GuiChrome, GuiTheme } from './types';

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const listenMock = vi.mocked(listen);
const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	listenMock.mockReset();
	listenMock.mockResolvedValue(() => {});
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	document.documentElement.style.cssText = '';
	resetSectionsForTest();
});

// The batch carries only the sections that changed (design D1): this one
// carries the queue alone, so every other store keeps its value.
test('applyBatch writes only the sections the batch carries', () => {
	const before = { focus: get(focus), summary: get(summary), playback: get(playback) };

	applyBatch({
		queue: {
			revision: 1,
			value: {
				rows: [],
				cursor: 2,
				marks: [],
				active: null,
				hover: null,
				drag: null,
			},
		},
	});

	expect(get(queue).cursor).toBe(2);
	expect(get(focus)).toEqual(before.focus);
	expect(get(summary)).toEqual(before.summary);
	expect(get(playback)).toEqual(before.playback);
});

// A re-sent section whose revision the window already holds — a `sync`
// snapshot's copy — writes nothing (design D1).
test('applyBatch skips a section whose revision the store holds', () => {
	const batch = {
		summary: {
			revision: 7,
			value: {
				remaining: 3,
				nextTitle: null,
				tailSecs: null,
				rightColumn: 'length' as const,
				filter: '',
				filterActive: false,
			},
		},
	};
	applyBatch(batch);
	const written = get(summary);

	applyBatch(batch);

	expect(get(summary)).toBe(written);
});

test('a later revision writes the section again', () => {
	applyBatch({
		footer: {
			revision: 1,
			value: {
				owner: { text: '', role: 'textPrimary', bold: false },
				ownerLive: false,
				playlist: { text: '', role: 'textPrimary', bold: false },
				autosave: null,
				dirty: null,
			},
		},
	});
	const first = get(footer);

	applyBatch({
		footer: {
			revision: 2,
			value: {
				owner: { text: 'woims', role: 'textFocusAccent', bold: false },
				ownerLive: true,
				playlist: { text: '', role: 'textPrimary', bold: false },
				autosave: null,
				dirty: null,
			},
		},
	});

	expect(get(footer).ownerLive).toBe(true);
	expect(get(footer)).not.toBe(first);
});

test('connectSections subscribes to the sections event and asks for the snapshot', async () => {
	const unlisten = await connectSections();

	expect(listenMock).toHaveBeenCalledWith(SECTIONS_EVENT, expect.any(Function));
	expect(invokeMock).toHaveBeenCalledWith('sync');
	expect(typeof unlisten).toBe('function');
});

// design D4: the `sync` invoke runs only after the `listen` promise resolves,
// so no batch can fall between the subscribe and the snapshot.
test('connectSections calls sync only after listen resolves', async () => {
	let release!: (off: () => void) => void;
	listenMock.mockReturnValue(
		new Promise((resolve) => {
			release = resolve;
		}),
	);

	const connecting = connectSections();
	expect(invokeMock).not.toHaveBeenCalled();

	release(() => {});
	await connecting;

	expect(invokeMock).toHaveBeenCalledWith('sync');
});

// The `playhead` anchor is not a section store (design D5): a carried
// `playhead` section stamps the prediction stores instead, and a reset
// drops it.
test('applyBatch stamps the carried playhead anchor and a reset drops it', () => {
	applyBatch({
		playhead: {
			revision: 3,
			value: {
				active: true,
				activeSlot: 4,
				positionTicks: 1_000,
				runtimeTicks: 2_000,
				paused: false,
			},
		},
	});

	expect(get(playhead.anchor)).toEqual({
		active: true,
		activeSlot: 4,
		positionTicks: 1_000,
		runtimeTicks: 2_000,
		paused: false,
	});

	resetSectionsForTest();

	expect(get(playhead.anchor)).toBeNull();
});

// The theme section's contract, moved from the frame store: a theme section
// writes the role colours to :root properties (design D7).
test('a theme section writes the role colours to :root properties', () => {
	applyBatch({
		theme: {
			revision: 1,
			value: {
				roles: {
					textPrimary: { r: 1, g: 2, b: 3 },
					accent: { r: 4, g: 5, b: 6 },
				},
				chrome: chrome(),
			},
		},
	});

	const style = document.documentElement.style;
	expect(style.getPropertyValue('--mbv-textPrimary')).toBe('rgb(1, 2, 3)');
	expect(style.getPropertyValue('--mbv-accent')).toBe('rgb(4, 5, 6)');
});

/** A full chrome fixture, the serde wire shape of `GuiChrome`. */
function chrome(): GuiChrome {
	return {
		background: { r: 10, g: 11, b: 12 },
		card: { r: 20, g: 21, b: 22 },
		popover: { r: 30, g: 31, b: 32 },
		muted: { r: 40, g: 41, b: 42 },
		secondary: { r: 50, g: 51, b: 52 },
		sidebar: { r: 60, g: 61, b: 62 },
		border: { r: 70, g: 71, b: 72 },
		input: { r: 80, g: 81, b: 82 },
		queueColumn: { r: 90, g: 91, b: 92 },
		queueColumnFocused: { r: 93, g: 94, b: 95 },
		queuePanel: { r: 96, g: 97, b: 98 },
		queuePanelFocused: { r: 99, g: 100, b: 101 },
		queuePlaybackBand: { r: 102, g: 103, b: 104 },
		queueFooterBand: { r: 105, g: 106, b: 107 },
	};
}

test('a theme section writes the chrome fills to :root properties', () => {
	const theme: GuiTheme = { roles: {}, chrome: chrome() };
	applyBatch({ theme: { revision: 1, value: theme } });

	const style = document.documentElement.style;
	expect(style.getPropertyValue('--mbv-background')).toBe('rgb(10, 11, 12)');
	expect(style.getPropertyValue('--mbv-border')).toBe('rgb(70, 71, 72)');
});
