// add-tauri-gui-client task 3.1/3.2: the frame store keeps only the last
// `mbv://frame` payload, and a frame's theme writes the role colours into
// CSS custom properties (designs D4 and D7). The Tauri event API is mocked;
// the delivered payloads are the serde wire shapes of bridge/frame.rs.

import { listen } from '@tauri-apps/api/event';
import { beforeEach, expect, test, vi } from 'vitest';
import { FRAME_EVENT, FrameStore } from './frames';
import type { GuiChrome, GuiTheme, GuiWindowFrame } from './types';

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(),
}));

const listenMock = vi.mocked(listen);

beforeEach(() => {
	listenMock.mockReset();
	listenMock.mockResolvedValue(() => {});
	document.documentElement.style.cssText = '';
});

/** The last handler registered through `listen`. */
function frameHandler(): (event: { payload: GuiWindowFrame }) => void {
	const call = listenMock.mock.calls.at(-1);
	return call?.[1] as (event: { payload: GuiWindowFrame }) => void;
}

const attachFrame: GuiWindowFrame = {
	attach: {
		state: { kind: 'waiting' },
		theme: null,
	},
};

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

function sliceFrame(theme: GuiTheme | null): GuiWindowFrame {
	return {
		slice: {
			focus: { surface: 'queue', panel: 'queue' },
			queue: { rows: [], cursor: null, marks: [], active: null, hover: null, drag: null },
			summary: {
				remaining: 0,
				nextTitle: null,
				finishClock: null,
				rightColumn: 'length',
				filter: '',
				filterActive: false,
			},
			panel: { collapsed: false, queueFolded: false },
			overlay: null,
			playback: {
				status: 'idle',
				host: '',
				hostIsRemote: false,
				shown: false,
				playback: {
					active: false,
					activeIdx: null,
					positionTicks: 0,
					runtimeTicks: 0,
					paused: false,
				},
				elapsedClock: '0:00',
				remainingClock: '0:00',
				showControls: false,
				nowPlayingTitle: null,
				parentLine: null,
				episodeBadge: null,
				stillCaption: null,
				finishClock: null,
				availability: { stop: false, next: false, previous: false },
				statusIndicators: null,
				artwork: null,
				chapters: [],
				introMark: null,
				creditsMark: null,
				trickplay: null,
				trackChips: null,
				targetChoices: [],
				hover: null,
			},
			footer: {
				owner: { text: '', role: 'textPrimary', bold: false },
				ownerLive: false,
				playlist: { text: '', role: 'textPrimary', bold: false },
				autosave: null,
				dirty: null,
			},
			status: { prefixArmed: false, toast: null },
			library: null,
			settings: null,
			uiResetEpoch: 0,
			theme,
		},
	};
}

test('a frame event replaces the last frame in the store', async () => {
	const store = new FrameStore();
	await store.listen();
	expect(listenMock).toHaveBeenCalledWith(FRAME_EVENT, expect.any(Function));

	frameHandler()({ payload: attachFrame });
	expect(store.frame).toEqual(attachFrame);

	const slice = sliceFrame(null);
	frameHandler()({ payload: slice });
	expect(store.frame).toEqual(slice);
});

test("a frame's theme writes the role colours to :root properties", async () => {
	const store = new FrameStore();
	await store.listen();

	frameHandler()({
		payload: {
			attach: {
				state: { kind: 'waiting' },
				theme: {
					roles: {
						textPrimary: { r: 1, g: 2, b: 3 },
						accent: { r: 4, g: 5, b: 6 },
					},
					chrome: chrome(),
				},
			},
		},
	});

	const style = document.documentElement.style;
	expect(style.getPropertyValue('--mbv-textPrimary')).toBe('rgb(1, 2, 3)');
	expect(style.getPropertyValue('--mbv-accent')).toBe('rgb(4, 5, 6)');
});

test("a frame's theme writes the chrome fills to :root properties", async () => {
	const store = new FrameStore();
	await store.listen();

	frameHandler()({
		payload: {
			attach: {
				state: { kind: 'waiting' },
				theme: {
					roles: {},
					chrome: chrome(),
				},
			},
		},
	});

	const style = document.documentElement.style;
	expect(style.getPropertyValue('--mbv-background')).toBe('rgb(10, 11, 12)');
	expect(style.getPropertyValue('--mbv-border')).toBe('rgb(70, 71, 72)');
});
