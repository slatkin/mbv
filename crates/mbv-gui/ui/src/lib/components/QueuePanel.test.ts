// add-tauri-gui-client task 3.3: the Queue panel paints the frame's
// viewport rows and the empty-queue state, and reports capacity and
// regions through `report_layout` (designs D4/D5). The footer pills moved
// to `QueueFooter` in row 6.4. The Tauri IPC is mocked; the asserted
// payload is the serde wire shape of bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiQueue } from '$lib/bridge/types';
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

function queue(rows: GuiQueue['rows'], active: GuiQueue['active'] = null): GuiQueue {
	return { rows, cursor: null, marks: [], active, hover: null, drag: null };
}

test('a frame with slots renders its rows', () => {
	const { getByText, container } = render(QueuePanel, {
		queue: queue(
			[
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
				{ row: 'heading', text: 'album' },
				{
					row: 'item',
					slot: 2,
					primary: 'Second track',
					secondary: null,
					trailing: null,
					duration: '4:00',
					endsAt: null,
					kind: 'media',
					state: { state: 'nowPlaying', progress: 42 },
					striped: false,
				},
			],
			2,
		),
		focused: true,
		panelFocused: true,
	});

	expect(getByText('First track')).toBeTruthy();
	expect(getByText('album')).toBeTruthy();
	expect(getByText('Second track')).toBeTruthy();
	expect(container.querySelectorAll('[data-slot]')).toHaveLength(2);
});

test('an empty frame renders the empty-queue state', () => {
	const { getByText, container } = render(QueuePanel, {
		queue: queue([]),
		focused: true,
		panelFocused: true,
	});

	expect(getByText('¯\\_(ツ)_/¯')).toBeTruthy();
	expect(container.querySelectorAll('[data-slot]')).toHaveLength(0);
});

test('a layout change sends one merged report', async () => {
	const { container } = render(QueuePanel, {
		queue: queue([
			{
				row: 'item',
				slot: 7,
				primary: 'Only track',
				secondary: null,
				trailing: null,
				duration: null,
					endsAt: null,
				kind: 'media',
				state: { state: 'ordinary' },
				striped: false,
			},
		]),
		focused: true,
		panelFocused: true,
	});
	// The mount paint reports once; wait for that flush before the change.
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
	invokeMock.mockClear();

	// jsdom has no layout: the change is the list gaining height, so the
	// reported capacity moves. Two whole 28px rows fit in 56px.
	const listEl = container.querySelector('[data-slot]')?.parentElement;
	expect(listEl).toBeTruthy();
	Object.defineProperty(listEl, 'clientHeight', { value: 56, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: {
			lists: [{ list: 'queue', wholeRows: 2 }],
			regions: [
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queue', id: { slot: 7 } },
				},
			],
			containerSize: { width: 0, height: 0 },
		},
	});
});
