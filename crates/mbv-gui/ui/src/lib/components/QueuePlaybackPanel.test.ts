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
	unobserve() {}
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
		parentLine: null,
		episodeBadge: null,
		finishClock: null,
		availability: { stop: true, next: true, previous: false },
		statusIndicators: null,
		artwork: null,
		chapters: [],
		introMark: null,
		creditsMark: null,
		trickplay: null,
		trackChips: null,
		targetChoices: [],
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

test('the controls row paints the status-indicator cluster the frame carries', () => {
	const { getByText } = render(QueuePlaybackPanel, {
		playback: playing({
			statusIndicators: [
				{ label: 'FHD', role: 'indicatorResolutionFg' },
				{ label: 'EN', role: 'indicatorAudioFg' },
				{ label: 'ENG', role: 'textMetadata' },
			],
		}),
	});

	expect(getByText('FHD')).toBeTruthy();
	expect(getByText('EN')).toBeTruthy();
	expect(getByText('ENG')).toBeTruthy();
});

// The playback-target icon's contract (the layout-parity brief): the TUI's
// `target_icon` paints the cast-family glyph for both states — the cast
// glyph while local, the connected-cast glyph while remote — in the target
// colour (yellow local, green remote), so the GUI maps both states to
// Lucide's one `Cast`, whose shape is the connected variant, and the colour
// plus the host label carry the distinction. Lucide renders the icon's name
// as the svg's `lucide-cast` class.
test.each([
	['local', false, 'var(--mbv-playbackTargetLocalFg)'],
	['remote', true, 'var(--mbv-playbackTargetRemoteFg)'],
])('%s target paints the cast glyph in its target colour', (_state, remote, colour) => {
	const { container } = render(QueuePlaybackPanel, {
		playback: playing({ hostIsRemote: remote, host: remote ? 'poo' : '' }),
	});

	const svg = container.querySelector('[data-target="playbackTarget"] svg');
	expect(svg?.classList.contains('lucide-cast')).toBe(true);
	// Lucide maps the `color` prop onto the svg's stroke (the outline shape
	// the TUI's target glyph keeps).
	expect(svg?.getAttribute('stroke')).toBe(colour);
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

test.each([
	[
		'playPause',
		'var(--mbv-textFocusAccent)',
	],
	['stop', 'var(--mbv-statusError)'],
	['previous', 'var(--mbv-textMuted)'],
	['next', 'var(--mbv-textStrong)'],
])('%s paints as a solid shape in its role colour', (target, colour) => {
	const { container } = render(QueuePlaybackPanel, {
		playback: playing(),
	});

	// A playing frame paints the pause glyph, not the play one; the colour
	// rule is fill = the glyph's own role colour, never the inherited text
	// colour the `currentColor` fill would resolve to (the user-reported
	// outline look).
	const svg = container.querySelector(`[data-target="${target}"] svg`);
	expect(svg?.getAttribute('fill')).toBe(colour);
	expect(svg?.getAttribute('stroke')).toBe(colour);
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
			containerSize: { width: 0, height: 0 },
		},
	});
});

test('the slot paints the frame\'s artwork and keeps it between frames', async () => {
	const art = 'data:image/jpeg;base64,AAAA';
	const { container, rerender } = render(QueuePlaybackPanel, {
		playback: playing({ artwork: { kind: 'image', detail: art } }),
	});

	// The carried image paints as the visual slot, in the band's wide
	// arrangement (design D10).
	const img = container.querySelector('img[data-target="visualSlot"]');
	expect(img).toBeTruthy();
	expect(img?.getAttribute('src')).toBe(art);

	// The slot's paint reports its region (the TUI's VisualSlot hit
	// target), ahead of the header's playback-target indicator.
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	const firstCall = invokeMock.mock.calls[0] as unknown as [
		string,
		{ report: { regions: { target: { id: string } }[] } },
	];
	const ids = firstCall[1].report.regions.map((entry) => entry.target.id);
	expect(ids).toContain('visualSlot');

	// A frame without a carried artwork keeps painting the last one: the
	// core arms the artwork again only when the art changed (design D10).
	await rerender({ playback: playing({ artwork: null }) });
	expect(
		container.querySelector('img[data-target="visualSlot"]')?.getAttribute('src'),
	).toBe(art);
});
