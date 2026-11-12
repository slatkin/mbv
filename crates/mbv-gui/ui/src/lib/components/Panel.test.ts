// add-tauri-gui-client row 6.6: the panel's four states are picked from the
// panel container's measured size and the Client core's two toggles (design
// D10) — the expanded artboard, the compact strip under 520 px, the expanded
// panel without its queue, and the 72 px rail. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import {
	focus as focusStore,
	footer as footerStore,
	hover as hoverStore,
	overlay as overlayStore,
	panel as panelStore,
	playback as playbackStore,
	queue as queueStore,
	resetSectionsForTest,
	status as statusStore,
	summary as summaryStore,
} from '$lib/bridge/sections';
import type {
	GuiPanelToggles,
	GuiQueue,
	GuiQueuePlayback,
	GuiQueueRow,
	GuiQueueSummary,
	LayoutSize,
} from '$lib/bridge/types';
import Panel from './Panel.svelte';

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
	// The panel and its children read the window's section stores; a section
	// one test carried would outlive it.
	resetSectionsForTest();
});

/** One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`). */
const SECOND = 10_000_000;

/** A container tall enough for the expanded artboard. */
const TALL: LayoutSize = { width: 640, height: 900 };

/** A container shorter than the expanded artboard's 520 px. */
const SHORT: LayoutSize = { width: 640, height: 480 };

/** One item row of the frame's viewport. */
function item(slot: number, primary: string): GuiQueueRow {
	return {
		row: 'item',
		slot,
		ordinal: slot,
		primary,
		secondary: null,
		trailing: null,
		duration: '3:12',
		endsAt: '21:05',
		kind: 'media',
		state: { state: 'ordinary' },
		striped: false,
	};
}

/** A two-slot queue with the first slot playing. */
function queue(): GuiQueue {
	return {
		rows: [item(1, 'First track'), item(2, 'Second track')],
		cursor: 1,
		marks: [],
		active: 1,
		hover: null,
		drag: null,
	};
}

/** The attached sections the expanded panel paints: a playing item with
 * carried artwork, the two slot queue, and the footer line. A case overrides
 * the sections the state under test reads. */
function seed(
	panel: Partial<GuiPanelToggles>,
	overrides: {
		queue?: GuiQueue;
		nextTitle?: string | null;
		panel?: Partial<GuiPanelToggles>;
		playback?: Partial<GuiQueuePlayback>;
		summary?: Partial<GuiQueueSummary>;
	} = {},
): void {
	panelStore.set({
		collapsed: false,
		queueFolded: false,
		uiResetEpoch: 0,
		...panel,
		...overrides.panel,
	});
	focusStore.set({ surface: 'queue', panel: 'queue' });
	queueStore.set(overrides.queue ?? queue());
	summaryStore.set({
		remaining: 1,
		nextTitle: overrides.nextTitle === undefined ? 'Second track' : overrides.nextTitle,
		finishClock: '21:43',
		rightColumn: 'length',
		filter: '',
		filterActive: false,
		...overrides.summary,
	});
	overlayStore.set(null);
	playbackStore.set(playing(overrides.playback));
	footerStore.set({
		owner: { text: 'woims', role: 'textFocusAccent', bold: false },
		ownerLive: true,
		playlist: { text: 'mix', role: 'textMetadata', bold: false },
		autosave: null,
		dirty: null,
	});
	statusStore.set({ prefixArmed: false, toast: null });
	hoverStore.set(null);
}

/** A playing transport band with the carried artwork the panel adopts. */
function playing(overrides: Partial<GuiQueuePlayback> = {}): GuiQueuePlayback {
	return {
		status: 'playing',
		host: 'poo',
		hostIsRemote: false,
		shown: true,
		playback: {
			active: true,
			activeIdx: 0,
			positionTicks: 60 * SECOND,
			runtimeTicks: 200 * SECOND,
			paused: false,
		},
		elapsedClock: '1:00',
		remainingClock: '2:20',
		showControls: true,
		nowPlayingTitle: 'Track One',
		parentLine: 'The Show',
		episodeBadge: null,
		stillCaption: null,
		finishClock: '21:00',
		chapters: [],
		introMark: null,
		creditsMark: null,
		trickplay: null,
		availability: { stop: false, next: true, previous: true },
		statusIndicators: null,
		artwork: { kind: 'image', detail: 'data:image/jpeg;base64,AAAA' },
		trackChips: null,
		targetChoices: [],
		...overrides,
	};
}

/** The region target ids of the last merged layout report. */
function lastReportedRegionIds(): unknown[] {
	const calls = invokeMock.mock.calls.filter((entry) => entry[0] === 'report_layout');
	const report = calls.at(-1)?.[1] as
		| { report: { regions: { target: { id: unknown } }[] } }
		| undefined;
	return report?.report.regions.map((entry) => entry.target.id) ?? [];
}

test('a tall container renders the expanded artboard', () => {
	seed({ collapsed: false, queueFolded: false });
	const { container, getByText } = render(Panel, {
		container: TALL,
		libraryVisible: false,
	});

	expect(container.querySelector('[data-panel-state="expanded"]')).toBeTruthy();
	expect(getByText('Track One')).toBeTruthy();
	expect(getByText('Second track')).toBeTruthy();
	expect(getByText('woims')).toBeTruthy();
});

test('a container shorter than 520 px renders the compact strip', async () => {
	// The playhead row (slot 1) is outside the viewport the frame carries, the
	// state the review found (row 6.6, finding P2): with no Queue list painted,
	// the old viewport reading named no next item. The summary carries it, so
	// the strip still paints it.
	seed(
			{ collapsed: false, queueFolded: false },
			{ queue: { ...queue(), rows: [item(2, 'Second track')], active: 1 } },
		);
	const { container, getByText, queryByText } = render(Panel, {
		container: SHORT,
		libraryVisible: false,
	});

	expect(container.querySelector('[data-panel-state="compactStrip"]')).toBeTruthy();
	expect(getByText('The Show · Next: Second track')).toBeTruthy();
	expect(container.querySelector('[data-panel-state="compactStrip"] img')).toBeTruthy();
	// The strip paints no queue: the next item is a summary datum, not a
	// painted row of its own.
	expect(queryByText('Second track')).toBeNull();

	await vi.waitFor(() => expect(lastReportedRegionIds()).toContain('playPause'));
	expect(lastReportedRegionIds()).toEqual(['visualSlot', 'playPause', 'next']);
});

test('the strip paints the parent line alone while no slot follows the playhead', () => {
	seed({ collapsed: false, queueFolded: false }, { nextTitle: null });
	const { getByText, queryByText } = render(Panel, {
		container: SHORT,
		libraryVisible: false,
	});

	expect(getByText('The Show')).toBeTruthy();
	expect(queryByText(/Next:/)).toBeNull();
});

test('the queue-fold toggle replaces the rows with one summary line', async () => {
	seed({ collapsed: false, queueFolded: true });
	const { container, getByText, queryByText } = render(Panel, {
		container: TALL,
		libraryVisible: false,
	});

	expect(container.querySelector('[data-panel-state="expandedWithoutQueue"]')).toBeTruthy();
	expect(container.querySelector('[data-remaining]')?.textContent).toBe('1');
	expect(getByText('21:43')).toBeTruthy();
	expect(queryByText('Second track')).toBeNull();
	const expand = container.querySelector<HTMLElement>(
		'[data-panel-state="expandedWithoutQueue"] [data-target="queueFold"]',
	);
	expect(expand?.getAttribute('aria-label')).toBe('Expand queue');

	await vi.waitFor(() => expect(lastReportedRegionIds()).toContain('queueFold'));
});

test('the collapse toggle renders the 72 px rail', async () => {
	seed({ collapsed: true, queueFolded: false });
	const { container, getByText, queryByText } = render(Panel, {
		container: TALL,
		libraryVisible: false,
	});

	expect(container.querySelector('[data-panel-state="rail"]')).toBeTruthy();
	expect(getByText('-2:20')).toBeTruthy();
	const expand = container.querySelector<HTMLElement>('[data-target="collapse"]');
	expect(expand?.getAttribute('aria-label')).toBe('Expand panel');
	// The rail paints no queue and no footer line; the core keeps both.
	expect(queryByText('Second track')).toBeNull();
	expect(queryByText('woims')).toBeNull();

	await vi.waitFor(() =>
		expect(lastReportedRegionIds()).toEqual([
			'playPause',
			'next',
			'visualSlot',
			'collapse',
		]),
	);
});

test('the collapsed rail paints the dim canvas fill, the TUI BgDim slot', () => {
	seed({ collapsed: true, queueFolded: false });
	const { container } = render(Panel, {
		container: TALL,
		libraryVisible: false,
	});

	const rail = container.querySelector<HTMLElement>('[data-panel-state="rail"]');
	expect(rail?.style.background).toBe('var(--canvas-dim)');
});
