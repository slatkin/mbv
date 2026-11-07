// add-tauri-gui-client tasks 3.3 and 6.5: the Queue panel paints the
// frame's viewport rows (the canvas's 30 px rows and their watched,
// active, and partial-progress states), the canvas header's summary, the
// empty-queue state, the hovered row's grip and actions button, the open
// row menu's entries and key hints, and the drag's ghost and insertion
// line; it reports capacity and the header, slot, grip, actions, and
// menu-entry regions through `report_layout` (designs D4/D5). Presses forward
// through the shared input installer as `pointer` events and the core
// resolves them against the hit map. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/layout.rs and
// bridge/pointer.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {
	GuiOverlayFrame,
	GuiQueue,
	GuiQueueRow,
	GuiQueueSummary,
} from '$lib/bridge/types';
import QueuePanel from './QueuePanel.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

/** A ResizeObserver stub: jsdom has none and no layout. The test fires the
 * callback to stand in for a layout change. */
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

function queue(
	rows: GuiQueue['rows'],
	active: GuiQueue['active'] = null,
	rest: Partial<GuiQueue> = {},
): GuiQueue {
	return { rows, cursor: null, marks: [], active, hover: null, drag: null, ...rest };
}

function summary(overrides: Partial<GuiQueueSummary> = {}): GuiQueueSummary {
	return {
		remaining: 2,
		nextTitle: null,
		finishClock: '21:43',
		rightColumn: 'length',
		filter: '',
		filterActive: false,
		...overrides,
	};
}

function item(slot: number, rest: Partial<Extract<GuiQueueRow, { row: 'item' }>> = {}) {
	const row: Extract<GuiQueueRow, { row: 'item' }> = {
		row: 'item',
		slot,
		ordinal: slot,
		primary: `Track ${slot}`,
		secondary: null,
		trailing: null,
		duration: '3:12',
		endsAt: '21:40',
		kind: 'media',
		state: { state: 'ordinary' },
		striped: false,
		...rest,
	};
	return row;
}

function rowMenu(): GuiOverlayFrame {
	return {
		kind: 'rowMenu',
		anchorSlot: 2,
		cursor: 1,
		entries: [
			{ label: 'Play now', keyHint: 'Enter' },
			{ label: 'Play next', keyHint: 'n' },
			{ label: 'Mark watched', keyHint: 'w' },
			{ label: 'Go to episode', keyHint: 'g' },
			{ label: 'Remove from queue', keyHint: 'dd' },
		],
	};
}

function renderPanel(
	queueProp: GuiQueue,
	summaryProp: GuiQueueSummary = summary(),
	overlay: GuiOverlayFrame | null = null,
) {
	return render(QueuePanel, {
		queue: queueProp,
		summary: summaryProp,
		overlay,
		focused: true,
		panelFocused: true,
	});
}

interface ReportedReport {
	report: { regions: { target: { id: unknown } }[] };
}

/** The region target ids of the last layout report, in report order. */
function reportedRegionIds(): unknown[] {
	const calls = invokeMock.mock.calls.filter((entry) => entry[0] === 'report_layout');
	const report = calls.at(-1)?.[1] as ReportedReport | undefined;
	return report?.report.regions.map((entry) => entry.target.id) ?? [];
}

/** The `pointer` invoke payloads the test's presses produced. */
function pointerCalls(): { event: { x: number; y: number } }[] {
	return invokeMock.mock.calls
		.filter((call) => call[0] === 'pointer')
		.map((call) => call[1] as { event: { x: number; y: number } });
}

test('a frame with slots renders its rows', () => {
	const { getByText, container } = renderPanel(
		queue(
			[
				item(1, { primary: 'First track', endsAt: null }),
				{ row: 'heading', text: 'album' },
				item(2, { primary: 'Second track', endsAt: null }),
			],
			2,
		),
	);

	expect(getByText('First track')).toBeTruthy();
	expect(getByText('album')).toBeTruthy();
	expect(getByText('Second track')).toBeTruthy();
	expect(container.querySelectorAll('[data-slot]')).toHaveLength(2);
});

test('an empty frame renders the empty-queue state', () => {
	const { getByText, container } = renderPanel(queue([]));

	expect(getByText('¯\\_(ツ)_/¯')).toBeTruthy();
	expect(container.querySelectorAll('[data-slot]')).toHaveLength(0);
});

test('the header paints the count, the finish clock, the switch, and the filter', () => {
	const { getByText, container } = renderPanel(queue([]));

	expect(getByText('Queue')).toBeTruthy();
	expect(container.querySelector('[data-remaining]')?.textContent).toBe('2');
	expect(container.querySelector('[data-finish-clock]')?.textContent).toBe('21:43');
	expect(getByText('Length')).toBeTruthy();
	expect(getByText('Ends at')).toBeTruthy();
	expect(getByText('Filter')).toBeTruthy();
});

test('the active filter paints its text and the caret', () => {
	const { getByText, container } = renderPanel(
		queue([]),
		summary({ filter: 'amb', filterActive: true }),
	);

	expect(getByText('amb')).toBeTruthy();
	expect(container.querySelector('[data-caret]')).toBeTruthy();
});

test('the right column paints the ends-at clock in the ends-at mode', () => {
	const { getByText, queryByText } = renderPanel(
		queue([item(1)]),
		summary({ rightColumn: 'endsAt' }),
	);

	expect(getByText('21:40')).toBeTruthy();
	expect(queryByText('3:12')).toBeNull();
});

test('the rows paint the watched, active, and partial-progress states', () => {
	const { getByText, container } = renderPanel(
		queue(
			[
				item(1, { primary: 'Watched', state: { state: 'played' } }),
				item(2, {
					primary: 'Playing',
					state: { state: 'nowPlaying', progress: 12 },
				}),
				item(3, { primary: 'Partial', state: { state: 'active', progress: 42 } }),
			],
			2,
		),
	);

	// The watched row strikes its title through.
	expect(getByText('Watched').className).toContain('line-through');
	// The active row fills with the accent.
	const playingRow = container.querySelector('[data-slot="2"]') as HTMLElement;
	expect(playingRow.style.background).toBe('var(--mbv-accent)');
	// The partial row paints a thin progress bar under its title.
	const partialRow = container.querySelector('[data-slot="3"]') as HTMLElement;
	const bar = partialRow.querySelector('[data-progress] span') as HTMLElement;
	expect(bar.style.width).toBe('42%');
});

test('the hovered row alone paints the grip and the actions button', () => {
	const { container } = renderPanel(queue([item(1), item(2), item(3)], null, { hover: 2 }));

	expect(container.querySelectorAll('[data-grip]')).toHaveLength(1);
	expect(container.querySelectorAll('[data-actions]')).toHaveLength(1);
	const hovered = container.querySelector('[data-slot="2"]') as HTMLElement;
	expect(hovered.querySelector('[data-grip]')).toBeTruthy();
	expect(hovered.querySelector('[data-actions]')).toBeTruthy();
});

test('the open row menu lists its entries with their key hints', () => {
	const { getByText, container } = renderPanel(
		queue([item(1)], null, { hover: 1 }),
		summary(),
		rowMenu(),
	);

	const entries = container.querySelectorAll('[data-menu-entry]');
	expect(entries).toHaveLength(5);
	expect(entries[0].textContent).toContain('Play now');
	expect(entries[0].textContent).toContain('Enter');
	expect(getByText('dd')).toBeTruthy();
});

test('a drag paints the ghost and the insertion line on the anchored row', () => {
	const { getByText, container } = renderPanel(
		queue([item(1), item(2), item(3)], null, {
			drag: {
				slot: 1,
				ghost: item(1, { primary: 'Dragged track' }),
				dropIndex: 2,
				insertion: { at: 'after', slot: 2 },
			},
		}),
		summary({ rightColumn: 'endsAt' }),
	);

	const ghost = container.querySelector('[data-ghost]') as HTMLElement;
	expect(ghost.textContent).toContain('Dragged track');
	// The ghost paints the same right column as the row it lifts.
	expect(ghost.textContent).toContain('21:40');
	expect(ghost.textContent).not.toContain('3:12');
	const line = container.querySelector('[data-insertion]') as HTMLElement;
	expect(line.dataset.insertion).toBe('after');
	expect(container.querySelector('[data-slot="2"]')?.contains(line)).toBe(true);
	expect(getByText('Track 1')).toBeTruthy();
});

test('the regions carry the header, slot, grip, actions, and menu-entry ids, menu last', async () => {
	renderPanel(
		queue([item(1), item(2)], null, { hover: 2 }),
		summary(),
		rowMenu(),
	);

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	expect(reportedRegionIds()).toEqual([
		'rightColumn',
		'filter',
		{ slot: 1 },
		{ slot: 2 },
		{ rowGrip: 2 },
		{ rowActions: 2 },
		{ rowMenuEntry: 0 },
		{ rowMenuEntry: 1 },
		{ rowMenuEntry: 2 },
		{ rowMenuEntry: 3 },
		{ rowMenuEntry: 4 },
	]);
});

test('only the inactive right-column segment reports a press target', async () => {
	const { container } = renderPanel(queue([]), summary({ rightColumn: 'length' }));

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	// A press on the active segment would flip away from the mode it shows,
	// so the segment reports no region and only the other one does.
	const segments = [...container.querySelectorAll('[data-right-column-segment]')];
	expect(segments.map((el) => el.textContent?.trim())).toEqual(['Ends at']);
});

test('a press on the right-column switch forwards a pointer press', async () => {
	const { container } = renderPanel(queue([item(1)]));
	const control = container.querySelector('[data-right-column-switch]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(control, { clientX: 11, clientY: 4, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 11,
					y: 4,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});

test('a press on the filter forwards a pointer press', async () => {
	const { container } = renderPanel(queue([item(1)]));
	const control = container.querySelector('[data-filter]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(control, { clientX: 13, clientY: 6, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 13,
					y: 6,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});

test('the open menu anchors under the row its anchorSlot names', () => {
	// The anchor (slot 2, the second painted row) places the menu under
	// its bottom edge — not the hovered row's (slot 3) and not the
	// cursor row's (slot 1).
	const { container } = renderPanel(
		queue([item(1), item(2), item(3)], null, { hover: 3, cursor: 1 }),
		summary(),
		rowMenu(),
	);

	const menuEl = container.querySelector('[data-row-menu]') as HTMLElement;
	expect(menuEl.style.top).toBe('calc(var(--mbv-cell-row) + 60px)');
});

test('a press on the grip forwards a pointer press', async () => {
	const { container } = renderPanel(queue([item(1), item(2)], null, { hover: 2 }));
	const grip = container.querySelector('[data-grip]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(grip, { clientX: 5, clientY: 7, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 5,
					y: 7,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});

test('a press on the actions button forwards a pointer press', async () => {
	const { container } = renderPanel(queue([item(1), item(2)], null, { hover: 2 }));
	const actions = container.querySelector('[data-actions]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(actions, { clientX: 9, clientY: 3, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 9,
					y: 3,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});

test('a press on a menu entry forwards a pointer press', async () => {
	const { container } = renderPanel(queue([item(1)], null, { hover: 1 }), summary(), rowMenu());
	const entry = container.querySelector('[data-menu-entry="4"]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(entry, { clientX: 30, clientY: 40, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 30,
					y: 40,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});

test('a layout change sends one merged report', async () => {
	const { container } = renderPanel(queue([item(7, { duration: null, endsAt: null })]));
	// The mount paint reports once; wait for that flush before the change.
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
	invokeMock.mockClear();

	// jsdom has no layout: the change is the list gaining height, so the
	// reported capacity moves. Two whole 30px rows fit in 60px.
	const listEl = container.querySelector('[data-slot]')?.parentElement;
	expect(listEl).toBeTruthy();
	Object.defineProperty(listEl, 'clientHeight', { value: 60, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: {
			lists: [{ list: 'queue', wholeRows: 2 }],
			regions: [
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queue', id: 'rightColumn' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queue', id: 'filter' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queue', id: { slot: 7 } },
				},
			],
			containerSize: { width: 0, height: 0 },
		},
	});
});
