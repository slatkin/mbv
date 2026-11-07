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
			{ label: 'Play now', keyHint: 'Enter', destructive: false },
			{ label: 'Play next', keyHint: 'n', destructive: false },
			{ label: 'Mark watched', keyHint: 'w', destructive: false },
			{ label: 'Go to episode', keyHint: 'g', destructive: false },
			{ label: 'Remove from queue', keyHint: 'dd', destructive: true },
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

/** The painted progress fill of one row's track. */
function progressBar(container: HTMLElement, slot: number): HTMLElement {
	return container.querySelector(`[data-slot="${slot}"] [data-progress] span`) as HTMLElement;
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

test('the header paints the canvas heading, the finish clock, the switch, and the filter', () => {
	const { getByText, container } = renderPanel(queue([]));

	expect(container.querySelector('[data-queue-heading]')?.textContent).toBe('Up next · 2');
	expect(container.querySelector('[data-finish-clock]')?.textContent).toBe('queue ends 21:43');
	expect(getByText('Length')).toBeTruthy();
	expect(getByText('Ends at')).toBeTruthy();
	// The filter is the canvas's icon-only magnifier button at rest
	// (tasks.md A.2: a magnifier icon, not the funnel).
	expect(container.querySelector('[data-filter]')?.getAttribute('aria-label')).toBe(
		'Filter queue',
	);
	const filterIcon = container.querySelector('[data-filter] svg');
	expect(filterIcon?.classList).toContain('lucide-search');
});

test('the active filter paints its text and the caret', () => {
	const { getByText, container } = renderPanel(
		queue([]),
		summary({ filter: 'amb', filterActive: true }),
	);

	expect(getByText('amb')).toBeTruthy();
	expect(container.querySelector('[data-caret]')).toBeTruthy();
});

test('the right column paints the mode\'s value in the canvas colour', () => {
	// "Ends at" reads green, "Length" muted (the canvas's right-column rule).
	const ends = renderPanel(queue([item(1)]), summary({ rightColumn: 'endsAt' }));

	expect(ends.getByText('21:40').style.color).toBe('var(--canvas-accent)');
	expect(ends.queryByText('3:12')).toBeNull();

	const length = renderPanel(queue([item(1)]), summary({ rightColumn: 'length' }));

	expect(length.getByText('3:12').style.color).toBe('var(--canvas-muted)');
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
				item(4, { primary: 'Started', state: { state: 'active', progress: 1 } }),
			],
			2,
		),
	);

	// The watched row strikes its title through in the canvas's strike colour.
	const watched = getByText('Watched');
	expect(watched.className).toContain('line-through');
	expect(watched.style.textDecorationColor).toBe('var(--canvas-strike)');
	// The active row fills with the canvas's green and weights its title 600.
	const playingRow = container.querySelector('[data-slot="2"]') as HTMLElement;
	expect(playingRow.style.background).toBe('var(--canvas-accent)');
	expect(getByText('Playing').style.fontWeight).toBe('600');
	// The track under the title fills to max(percent, 3)%.
	expect(progressBar(container, 3).style.width).toBe('42%');
	expect(progressBar(container, 4).style.width).toBe('3%');
});

test('the row paints the canvas one-liner: its ordinal, one title line, and the value', () => {
	const { getByText, queryByText, container } = renderPanel(
		queue([
			item(1, {
				primary: 'Nether Zone',
				secondary: 'Peep Show',
				trailing: { gutter: 'mon 08' },
			}),
		]),
	);

	const row = container.querySelector('[data-slot="1"]') as HTMLElement;
	expect(row.tagName).toBe('BUTTON');
	expect(row.querySelector('[data-ordinal]')?.textContent).toBe('01');
	expect(getByText('Nether Zone')).toBeTruthy();
	// The canvas row has no second line, no date gutter, and no play marker.
	expect(queryByText('Peep Show')).toBeNull();
	expect(queryByText('mon 08')).toBeNull();
	expect(row.querySelector('svg')).toBeNull();
});

test('the cursor row paints the canvas picked fill, and the active row wins over it', () => {
	const { container } = renderPanel(
		queue(
			[item(1), item(2, { state: { state: 'nowPlaying', progress: null } })],
			2,
			{ cursor: 1, marks: [2] },
		),
	);

	expect((container.querySelector('[data-slot="1"]') as HTMLElement).style.background).toBe(
		'var(--canvas-selected)',
	);
	expect((container.querySelector('[data-slot="2"]') as HTMLElement).style.background).toBe(
		'var(--canvas-accent)',
	);
});

// The Menus artboard's hovered row (B.3): the six-dot grip replaces the
// ordinal in the leading slot, the 30 px actions button replaces the
// right-column value in the trailing slot, and the row fills with the hover
// colour in the hover text.
test('the hovered row alone paints the grip and the actions button in place of its ordinal and value', () => {
	const { getByText, container } = renderPanel(
		queue([item(1), item(2), item(3)], null, { hover: 2 }),
	);

	expect(container.querySelectorAll('[data-grip]')).toHaveLength(1);
	expect(container.querySelectorAll('[data-actions]')).toHaveLength(1);
	const hovered = container.querySelector('[data-row="2"]') as HTMLElement;
	const row = hovered.querySelector('[data-slot="2"]') as HTMLElement;

	// The grip replaces the ordinal: six dots in the leading slot, and the
	// right-column value gives way to the actions button.
	expect(hovered.querySelectorAll('[data-grip] svg circle')).toHaveLength(6);
	expect(row.querySelector('[data-ordinal]')).toBeNull();
	expect(row.querySelector('[data-right-value]')).toBeNull();
	expect(container.querySelector('[data-row="1"] [data-ordinal]')).toBeTruthy();

	// The actions button is the canvas's 30 px box in its fill and radius.
	const actions = hovered.querySelector('[data-actions]') as HTMLElement;
	expect(actions.getAttribute('aria-label')).toBe('Row actions');
	expect(actions.style.width).toBe('30px');
	expect(actions.style.height).toBe('30px');
	expect(actions.style.borderRadius).toBe('4px');
	expect(actions.style.background).toBe('var(--canvas-action)');

	// The row fills with the hover colour, its title ellipsises in the hover
	// text.
	expect(row.style.background).toBe('var(--canvas-selected)');
	expect(row.style.borderRadius).toBe('4px');
	const title = getByText('Track 2');
	expect(title.className).toContain('truncate');
	expect(title.style.color).toBe('var(--canvas-heading)');
});

// The Menus artboard's row menu (B.2): a 230 px card in the canvas chrome,
// 34 px entries at 13 px, the muted mono hints with the paint-time `⏎`
// glyph, and the projected destructive entry in its tone under a divider
// (B.1).
test('the open row menu paints the canvas chrome, hints, and destructive entry', () => {
	const { getByText, container } = renderPanel(
		queue([item(1)], null, { hover: 1 }),
		summary(),
		rowMenu(),
	);

	const menu = container.querySelector('[data-row-menu]') as HTMLElement;
	// The canvas's menu semantics (Menus.dc.html: the `role="menu"
	// aria-label="Row actions"` card whose entries are `menuitem`s).
	expect(menu.getAttribute('role')).toBe('menu');
	expect(menu.getAttribute('aria-label')).toBe('Row actions');
	expect(menu.style.width).toBe('230px');
	expect(menu.style.padding).toBe('6px');
	expect(menu.style.gap).toBe('2px');
	expect(menu.style.background).toBe('var(--canvas-menu)');
	expect(menu.style.border).toBe('1px solid var(--canvas-menu-border)');
	expect(menu.style.borderRadius).toBe('8px');
	expect(menu.style.boxShadow).toBe('var(--canvas-menu-shadow)');

	const entries = container.querySelectorAll('[data-menu-entry]');
	expect(entries).toHaveLength(5);
	expect([...entries].every((entry) => entry.getAttribute('role') === 'menuitem')).toBe(true);
	expect(entries[0].textContent).toContain('Play now');
	// The core's raw `Enter` hint chord paints as the canvas's return glyph.
	expect(entries[0].textContent).toContain('⏎');
	expect(entries[0].textContent).not.toContain('Enter');
	expect(getByText('dd')).toBeTruthy();

	const first = entries[0] as HTMLElement;
	expect(first.style.height).toBe('34px');
	expect(first.style.padding).toBe('0px 10px');
	expect(first.style.fontSize).toBe('13px');
	const hint = first.querySelector('span:last-child') as HTMLElement;
	expect(hint.style.fontSize).toBe('11px');
	expect(hint.style.color).toBe('var(--canvas-muted)');

	// The core's cursor row fills with the canvas's selected colour.
	expect((entries[1] as HTMLElement).style.background).toBe('var(--canvas-selected)');
	// The projected destructive flag paints the tone, not the cursor or the
	// label; the divider sits directly above that entry.
	const remove = entries[4] as HTMLElement;
	expect(remove.style.color).toBe('var(--canvas-destructive)');
	const divider = container.querySelector('[data-menu-divider]') as HTMLElement;
	expect(divider.nextElementSibling).toBe(remove);
	expect(divider.style.background).toBe('var(--canvas-menu-border)');
});

// The Menus artboard's drag section (B.5): the ghost carries the title and
// the dragged row's percent only, the insertion line is inset 8 px with its
// radius, and the dragged row no longer dims in place.
test('a drag paints the ghost without an ordinal, its percent, and the inset insertion line', () => {
	const { getByText, container } = renderPanel(
		queue([item(1), item(2), item(3)], null, {
			drag: {
				slot: 1,
				ghost: item(1, {
					primary: 'Dragged track',
					state: { state: 'active', progress: 5 },
				}),
				dropIndex: 2,
				insertion: { at: 'after', slot: 2 },
			},
		}),
		summary({ rightColumn: 'endsAt' }),
	);

	const ghost = container.querySelector('[data-ghost]') as HTMLElement;
	const card = ghost.firstElementChild as HTMLElement;
	expect(ghost.textContent).toContain('Dragged track');
	// The ghost paints no ordinal and no right-column value: the percent
	// replaces the row's ends-at fact.
	expect(ghost.querySelector('[data-ordinal]')).toBeNull();
	expect(ghost.querySelector('[data-ghost-percent]')?.textContent).toBe('5%');
	expect(ghost.textContent).not.toContain('21:40');
	// The canvas ghost's offset, overhang, fill, radius, tilt, and lift.
	expect(ghost.style.left).toBe('28px');
	expect(ghost.style.right).toBe('-6px');
	expect(card.style.background).toBe('var(--canvas-ghost)');
	expect(card.style.borderRadius).toBe('4px');
	expect(card.style.transform).toBe('rotate(-1deg)');
	expect(card.style.boxShadow).toBe('var(--canvas-ghost-shadow)');
	expect(getByText('Dragged track').style.color).toBe('var(--canvas-heading)');

	// The insertion line is inset 8 px and rounded 1 px in the canvas colour.
	const line = container.querySelector('[data-insertion]') as HTMLElement;
	expect(line.dataset.insertion).toBe('after');
	expect(line.style.margin).toBe('0px 8px');
	expect(line.style.borderRadius).toBe('1px');
	expect(line.style.background).toBe('var(--canvas-insertion)');
	expect(container.querySelector('[data-row="2"]')?.contains(line)).toBe(true);

	// The dragged row stays lit: the canvas paints no in-place dim.
	const dragged = container.querySelector('[data-slot="1"]') as HTMLElement;
	expect(dragged.className).not.toContain('opacity-40');
});

test('the drag ghost paints no percent when the dragged row has no progress', () => {
	const { container } = renderPanel(
		queue([item(1), item(2)], null, {
			drag: {
				slot: 1,
				ghost: item(1, { primary: 'Dragged track' }),
				dropIndex: 1,
				insertion: { at: 'before', slot: 2 },
			},
		}),
	);

	expect(container.querySelector('[data-ghost]')).toBeTruthy();
	expect(container.querySelector('[data-ghost-percent]')).toBeNull();
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

// The menu's top edge sits 6 px inside the bottom edge of the row its
// `anchorSlot` names — the canvas's overlap of the 230 px menu over its
// anchor row — not under it.
test('the open menu overlaps the bottom edge of the row its anchorSlot names', () => {
	// The anchor (slot 2, the second painted row) places the menu over
	// its bottom edge — not the hovered row's (slot 3) and not the
	// cursor row's (slot 1).
	const { container } = renderPanel(
		queue([item(1), item(2), item(3)], null, { hover: 3, cursor: 1 }),
		summary(),
		rowMenu(),
	);

	const menuEl = container.querySelector('[data-row-menu]') as HTMLElement;
	expect(menuEl.style.top).toBe('calc(var(--canvas-queue-header-height) + 54px)');
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
	const listEl = container.querySelector('[data-queue-list]');
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
