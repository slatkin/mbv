// add-tauri-gui-client task 3.4: the Queue playback panel paints the
// frame's header row in every state, the transport band while `shown`, and
// its controls and seek row while a now-playing title rides — the TUI's
// queue-band gates (design D4). It reports the painted control regions
// through `report_layout`. The Tauri IPC is mocked; the asserted payload is
// the serde wire shape of bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiQueuePlayback } from '$lib/bridge/types';
import QueuePlaybackPanel from './QueuePlaybackPanel.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

/** A ResizeObserver stub: jsdom has none and no layout. */
class ResizeObserverStub {
	static instances: ResizeObserverStub[] = [];
	readonly #callback: ResizeObserverCallback;
	constructor(callback: ResizeObserverCallback) {
		this.#callback = callback;
		ResizeObserverStub.instances.push(this);
	}
	observe() {}
	disconnect() {}
	fire() {
		this.#callback([], this as unknown as ResizeObserver);
	}
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`). */
const SECOND = 10_000_000;

function playing(overrides: Partial<GuiQueuePlayback> = {}): GuiQueuePlayback {
	return {
		status: 'playing',
		host: 'poo',
		hostIsRemote: true,
		shown: true,
		playback: {
			active: true,
			activeIdx: 0,
			positionTicks: 60 * SECOND,
			runtimeTicks: 200 * SECOND,
			paused: false,
		},
		showControls: true,
		nowPlayingTitle: 'Track title',
		availability: { stop: true, next: true, previous: false },
		...overrides,
	};
}

test('a playing frame renders the header, transport controls, and seek times', () => {
	const { getByText, container } = render(QueuePlaybackPanel, {
		playback: playing(),
	});

	expect(getByText('PLAYING')).toBeTruthy();
	expect(getByText('Track title')).toBeTruthy();
	expect(getByText('poo')).toBeTruthy();
	expect(getByText('1:00')).toBeTruthy();
	expect(getByText('3:20')).toBeTruthy();
	for (const target of ['playPause', 'stop', 'previous', 'next', 'seekbar']) {
		expect(
			container.querySelector(`[data-target="${target}"]`),
			`${target} paints`,
		).toBeTruthy();
	}
});

test('an idle frame renders the header alone', () => {
	const { getByText, container } = render(QueuePlaybackPanel, {
		playback: playing({
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
			showControls: false,
			nowPlayingTitle: null,
			availability: { stop: false, next: false, previous: false },
		}),
	});

	expect(getByText('IDLE')).toBeTruthy();
	expect(container.querySelector('[data-target="playPause"]')).toBeNull();
	expect(container.querySelector('[data-target="seekbar"]')).toBeNull();
});

test('a paint reports the painted control regions', async () => {
	render(QueuePlaybackPanel, { playback: playing() });
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: {
			lists: [],
			regions: [
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queuePlayback', id: 'playPause' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queuePlayback', id: 'stop' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queuePlayback', id: 'previous' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queuePlayback', id: 'next' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queuePlayback', id: 'seekbar' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queuePlayback', id: 'playbackTarget' },
				},
			],
			width: 1024,
		},
	});
});
