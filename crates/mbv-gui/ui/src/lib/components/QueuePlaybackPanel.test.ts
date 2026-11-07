// add-tauri-gui-client task 6.4: the player panel's top block paints the
// canvas artboard from a frame — header with the playback target and state
// word, artwork with the episode badge, title and parent line, the scrubber
// with its clocks, and the transport. It reports the painted regions through
// `report_layout` and forwards pointer presses through `pointer`; the core
// resolves gestures against the hit map. The Tauri IPC is mocked, and the
// asserted payloads are the serde wire shapes of bridge/layout.rs and
// bridge/pointer.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type { GuiQueuePlayback } from '$lib/bridge/types';
import QueuePlaybackPanel from './QueuePlaybackPanel.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

/** A ResizeObserver stub: jsdom has none and no layout. */
class ResizeObserverStub {
	observe() {}
	unobserve() {}
	disconnect() {}
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
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
		parentLine: 'The Show',
		episodeBadge: 'S2 E5',
		finishClock: '21:00',
		availability: { stop: true, next: true, previous: false },
		statusIndicators: null,
		artwork: null,
		chapters: [
			{ name: 'Chapter One', startTicks: 0 },
			{ name: 'Chapter Two', startTicks: 120 * SECOND },
		],
		introMark: { startTicks: 0, endTicks: 30 * SECOND },
		creditsMark: { startTicks: 180 * SECOND, endTicks: 200 * SECOND },
		trickplay: null,
		trackChips: null,
		targetChoices: [],
		...overrides,
	};
}

function seekbar(container: HTMLElement): HTMLElement {
	const el = container.querySelector<HTMLElement>('[data-target="seekbar"]');
	if (!el) {
		throw new Error('the seekbar paints');
	}
	return el;
}

function stubWidth(el: HTMLElement, width: number): void {
	Object.defineProperty(el, 'getBoundingClientRect', {
		value: () => ({ x: 0, y: 0, width, height: 12 }),
	});
}

test('a playing frame renders the header, artwork badge, title, and clocks', () => {
	const { getByText, container } = render(QueuePlaybackPanel, {
		playback: playing({ artwork: { kind: 'image', detail: 'data:image/jpeg;base64,AAAA' } }),
	});

	expect(getByText('PLAYING')).toBeTruthy();
	expect(getByText('poo')).toBeTruthy();
	expect(getByText('Track title')).toBeTruthy();
	expect(getByText('The Show')).toBeTruthy();
	expect(getByText('S2 E5')).toBeTruthy();
	expect(getByText('1:00')).toBeTruthy();
	expect(getByText('-2:20')).toBeTruthy();
	expect(getByText('21:00')).toBeTruthy();
	expect(container.querySelector('[data-target="visualSlot"] img')).toBeTruthy();
});

test('a paint reports the transport, seekbar, visual-slot, and target regions', async () => {
	render(QueuePlaybackPanel, {
		playback: playing({ artwork: { kind: 'image', detail: 'data:image/jpeg;base64,AAAA' } }),
	});

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	const report = invokeMock.mock.calls.find((call) => call[0] === 'report_layout');
	const ids = (
		report?.[1] as { report: { regions: { target: { id: string } }[] } }
	).report.regions.map((entry) => entry.target.id);
	expect(ids).toEqual([
		'playPause',
		'stop',
		'previous',
		'next',
		'seekbar',
		'visualSlot',
		'playbackTarget',
	]);
});

test('a press on the scrubber forwards a pointer press with its position', async () => {
	const { container } = render(QueuePlaybackPanel, { playback: playing() });
	const detach = installInput();
	fireEvent.pointerDown(seekbar(container), { clientX: 42, clientY: 7, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(invokeMock).toHaveBeenCalledWith('pointer', {
			event: {
				kind: 'press',
				button: 0,
				buttons: 1,
				x: 42,
				y: 7,
				metaKey: false,
				shiftKey: false,
				ctrlKey: false,
				altKey: false,
				deltaX: 0,
				deltaY: 0,
			},
		}),
	);
});

test('a hover over the scrubber previews the time and its chapter', async () => {
	const { container, getByText } = render(QueuePlaybackPanel, { playback: playing() });
	const bar = seekbar(container);
	stubWidth(bar, 200);
	fireEvent.pointerMove(bar, { clientX: 150 });

	expect(getByText('2:30')).toBeTruthy();
	expect(getByText('Chapter Two')).toBeTruthy();
});
