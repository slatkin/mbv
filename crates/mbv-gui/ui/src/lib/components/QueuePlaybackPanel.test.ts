// add-tauri-gui-client task 6.4: the player panel's top block paints the
// canvas artboard from a frame — header with the playback target, its
// chooser and the collapse and queue-fold controls, artwork with the episode
// badge, title and parent line, the scrubber with its clocks and hover
// preview, and the transport. It reports the painted regions through
// `report_layout` and forwards pointer presses through `pointer`; the core
// resolves gestures against the hit map. The Tauri IPC is mocked, and the
// asserted payloads are the serde wire shapes of bridge/layout.rs and
// bridge/pointer.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {
	GuiOverlayFrame,
	GuiQueue,
	GuiQueueFooter,
	GuiQueuePlayback,
	GuiStatusBar,
} from '$lib/bridge/types';
import QueueFooter from './QueueFooter.svelte';
import QueuePanel from './QueuePanel.svelte';
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
		hover: null,
		...overrides,
	};
}

/** An open playback-target chooser with two choices, the first attached. */
function chooserOverlay(): GuiOverlayFrame {
	return {
		kind: 'targetChooser',
		cursor: 0,
		choices: [
			{
				key: { kind: 'emby', id: 'session-1' },
				label: 'Living room',
				isActive: true,
			},
			{
				key: { kind: 'cast', id: 'receiver-1' },
				label: 'Kitchen speaker',
				isActive: false,
			},
		],
	};
}

function renderPanel(
	playback: GuiQueuePlayback = playing(),
	overlay: GuiOverlayFrame | null = null,
) {
	return render(QueuePlaybackPanel, { playback, overlay });
}

/** A one-item queue, so the panel reports one `queue` slot region. */
function queue(): GuiQueue {
	return {
		rows: [
			{
				row: 'item',
				slot: 1,
				primary: 'First track',
				secondary: null,
				trailing: null,
				duration: '3:12',
				endsAt: null,
				kind: 'media',
				state: { state: 'ordinary' },
				striped: false,
			},
		],
		cursor: null,
		marks: [],
		active: null,
		hover: null,
		drag: null,
	};
}

const footer: GuiQueueFooter = {
	owner: { text: 'woims', role: 'textFocusAccent', bold: false },
	playlist: { text: 'mix', role: 'textMetadata', bold: false },
	autosave: null,
	dirty: null,
};

const idle: GuiStatusBar = { prefixArmed: false, toast: null };

/** One painted region element, by its target id. */
function control(container: HTMLElement, id: string): HTMLElement {
	const el = container.querySelector<HTMLElement>(`[data-target="${id}"]`);
	if (!el) {
		throw new Error(`the ${id} control paints`);
	}
	return el;
}

interface ReportedReport {
	report: { regions: { target: { id: unknown } }[] };
}

/** The region target ids of the first layout report the mocks saw. */
function reportedRegionIds(): unknown[] {
	const call = invokeMock.mock.calls.find((entry) => entry[0] === 'report_layout');
	const report = call?.[1] as ReportedReport | undefined;
	return report?.report.regions.map((entry) => entry.target.id) ?? [];
}

/** The region target ids of the last merged layout report. */
function lastReportedRegionIds(): unknown[] {
	const calls = invokeMock.mock.calls.filter((entry) => entry[0] === 'report_layout');
	const report = calls.at(-1)?.[1] as ReportedReport | undefined;
	return report?.report.regions.map((entry) => entry.target.id) ?? [];
}

/** Whether a region id is a chooser row's `targetChoice`. */
function isChoice(id: unknown): boolean {
	return typeof id === 'object' && id !== null && 'targetChoice' in id;
}

test('a playing frame renders the header, artwork badge, title, and clocks', () => {
	const { getByText, container } = renderPanel(
		playing({ artwork: { kind: 'image', detail: 'data:image/jpeg;base64,AAAA' } }),
	);

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

test('a paint reports the transport, seek, header, seekbar, and target regions', async () => {
	renderPanel(playing({ artwork: { kind: 'image', detail: 'data:image/jpeg;base64,AAAA' } }));

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	expect(reportedRegionIds()).toEqual([
		'playPause',
		'previous',
		'seekBack',
		'seekForward',
		'next',
		'seekbar',
		'visualSlot',
		'collapse',
		'queueFold',
		'playbackTarget',
	]);
});

test('a press on the scrubber forwards a pointer press with its position', async () => {
	const { container } = renderPanel();
	const detach = installInput();
	fireEvent.pointerDown(control(container, 'seekbar'), {
		clientX: 42,
		clientY: 7,
		button: 0,
		buttons: 1,
	});
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

test('the scrubber paints the hover preview from the frame', () => {
	const { getByText } = renderPanel(
		playing({
			hover: { fraction: 0.75, ticks: 150 * SECOND, chapter: 'Chapter Two' },
		}),
	);

	expect(getByText('2:30')).toBeTruthy();
	expect(getByText('Chapter Two')).toBeTruthy();
});

test('an open target chooser renders its choices', () => {
	const { getByText } = renderPanel(playing(), chooserOverlay());

	expect(getByText('Living room')).toBeTruthy();
	expect(getByText('Kitchen speaker')).toBeTruthy();
});

test('an open target chooser reports one region per row', async () => {
	renderPanel(playing(), chooserOverlay());

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	expect(reportedRegionIds().slice(-2)).toEqual([
		{ targetChoice: 0 },
		{ targetChoice: 1 },
	]);
});

// The hit map resolves a painted-over point to the LAST region reported
// under it, and `report_layout` merges contributions in Map insertion order:
// this panel mounts before the queue panel and footer, so the chooser's rows
// report under a second key first inserted when the chooser opens — after
// every other surface's key.
test('an open chooser reports its rows after the queue and footer regions', async () => {
	const { rerender } = renderPanel();
	render(QueuePanel, { queue: queue(), focused: false, panelFocused: false });
	render(QueueFooter, { footer, status: idle });

	await rerender({ playback: playing(), overlay: chooserOverlay() });

	await vi.waitFor(() =>
		expect(lastReportedRegionIds()).toContainEqual({ targetChoice: 1 }),
	);
	const ids = lastReportedRegionIds();
	const firstChoice = ids.findIndex(isChoice);
	expect(ids.slice(0, firstChoice)).toEqual(
		expect.arrayContaining([{ slot: 1 }, 'playlistPill']),
	);
	expect(ids.slice(firstChoice)).toEqual([{ targetChoice: 0 }, { targetChoice: 1 }]);
});

test('closing the chooser retracts its row regions', async () => {
	const { rerender } = renderPanel(playing(), chooserOverlay());
	await vi.waitFor(() =>
		expect(lastReportedRegionIds()).toContainEqual({ targetChoice: 0 }),
	);

	await rerender({ playback: playing(), overlay: null });

	await vi.waitFor(() => {
		const ids = lastReportedRegionIds();
		expect(ids).toContain('playbackTarget');
		expect(ids.some(isChoice)).toBe(false);
	});
});

test('a press on a chooser row forwards a pointer press with its position', async () => {
	const { container } = renderPanel(playing(), chooserOverlay());
	const detach = installInput();
	fireEvent.pointerDown(container.querySelector('[data-choice="1"]') as HTMLElement, {
		clientX: 9,
		clientY: 11,
		button: 0,
		buttons: 1,
	});
	detach();

	await vi.waitFor(() =>
		expect(invokeMock).toHaveBeenCalledWith('pointer', {
			event: {
				kind: 'press',
				button: 0,
				buttons: 1,
				x: 9,
				y: 11,
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

test('a press on the back-10 and forward-30 controls forwards a pointer press', () => {
	const { container } = renderPanel();
	const detach = installInput();
	fireEvent.pointerDown(control(container, 'seekBack'), {
		clientX: 31,
		clientY: 5,
		button: 0,
		buttons: 1,
	});
	fireEvent.pointerDown(control(container, 'seekForward'), {
		clientX: 47,
		clientY: 5,
		button: 0,
		buttons: 1,
	});
	detach();

	const xs = invokeMock.mock.calls
		.filter((call) => call[0] === 'pointer')
		.map((call) => (call[1] as { event: { x: number } }).event.x);
	expect(xs).toEqual([31, 47]);
});
