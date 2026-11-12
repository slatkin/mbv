// add-gui-library-panel row 11.1: the Feeds list (design D12) paints the fat
// entry rows with their date gutter and no letter heading and no rail, the
// cursor row's fill and the core's selected entry's raised fill, and a press
// selects while a double-click plays. Its `loading` fact paints its own line.
// The Tauri IPC is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiFeedsDetail, GuiFeedsList, GuiFeedsRow } from '$lib/bridge/types';
import { initialFeedsUi, feedsUi } from '$lib/library/feeds';
import { carryLibrary } from '$lib/bridge/test-library';
import LibraryFeedsList from './LibraryFeedsList.svelte';

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
	disconnect() {
		return undefined;
	}
	fire() {
		this.#callback([], this as unknown as ResizeObserver);
	}
}

/** The Latest pill's merged rows: two dated entries, one played, and one
 * partly played with its resume percent. */
const ENTRIES: GuiFeedsRow[] = [
	{ id: 's1::e1', title: 'Entry A', subtitle: 'Sub One', date: '12 May', state: 'ordinary' },
	{ id: 's2::e2', title: 'Entry B', subtitle: 'Sub Two', date: '9 May', state: 'played' },
	{
		id: 's1::e3',
		title: 'Entry C',
		subtitle: 'Sub One',
		date: '4 May',
		state: { active: { progress: 40 } },
	},
];

/** The carried Feeds list part over `rows`, with its load fact as the caller
 * sets it (design D12). */
function frame(rows: GuiFeedsRow[], loading = false): GuiFeedsList {
	return {
		pills: [{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false }],
		loading,
		failed: false,
		rows,
	};
}

/** The Feeds detail part: the first row selected unless `selected` names one,
 * and no Hero. */
function detail(
	rows: GuiFeedsRow[],
	selected: string | null = rows[0]?.id ?? null,
): GuiFeedsDetail {
	return { selected, hero: null };
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	feedsUi.set(initialFeedsUi());
	carryLibrary(null);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** Carry the Feeds content and render the list at a `viewportHeight` px
 * viewport: an unmeasured viewport paints nothing, as the Music list does. */
async function mountList(
	rows: GuiFeedsRow[],
	facts: { loading?: boolean; selected?: string | null } = {},
	viewportHeight = 300,
): Promise<HTMLElement> {
	const feeds = frame(rows, facts.loading);
	const picked = detail(rows, facts.selected);
	carryLibrary({
		list: { tabs: [], currentTab: 2, destination: { kind: 'feeds', ...feeds } },
		detail: { kind: 'feeds', ...picked },
	});
	const { container } = render(LibraryFeedsList, { list: feeds, detail: picked });
	const list = container.querySelector<HTMLElement>('[data-library-feeds-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	tick();
	return container;
}

/** The row ids the list painted, in paint order. */
function paintedRows(container: HTMLElement): (string | null)[] {
	return [...container.querySelectorAll('[data-library-feeds-row]')].map((el) =>
		el.getAttribute('data-library-feeds-row-id'),
	);
}

// The date-ordered list paints the date gutter and no letter heading and no
// rail (design D12, row 11.1), the entry title over its subscription.
test('the flat list paints its date gutter with no heading and no rail', async () => {
	const container = await mountList(ENTRIES);

	expect(paintedRows(container)).toEqual(['s1::e1', 's2::e2', 's1::e3']);
	expect(container.querySelector('[data-library-region="rail"]')).toBeNull();
	expect(container.querySelector('[data-library-feeds-letter]')).toBeNull();
	expect(
		container.querySelector('[data-library-feeds-row-id="s2::e2"] [data-library-feeds-date]')
			?.textContent,
	).toBe('9 May');
	expect(
		container.querySelector('[data-library-feeds-row-id="s1::e1"] [data-library-feeds-subtitle]')
			?.textContent,
	).toBe('Sub One');
});

test('a press sends the select command with the row id', async () => {
	const container = await mountList(ENTRIES);

	await fireEvent.click(
		container.querySelector('[data-library-feeds-row-id="s2::e2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsSelect', row: 's2::e2' },
	});
});

test('a double-click plays the entry', async () => {
	const container = await mountList(ENTRIES);

	await fireEvent.dblClick(
		container.querySelector('[data-library-feeds-row-id="s2::e2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsPlay', itemId: 's2::e2' },
	});
});

test('the cursor row and the core\'s selected entry carry their own fills', async () => {
	const container = await mountList(ENTRIES);
	feedsUi.update((ui) => ({ ...ui, cursor: 's2::e2' }));
	await tick();

	const cursorRow = container.querySelector(
		'[data-library-feeds-row-id="s2::e2"]',
	) as HTMLElement;
	const selectedRow = container.querySelector(
		'[data-library-feeds-row-id="s1::e1"]',
	) as HTMLElement;

	expect(cursorRow.getAttribute('data-library-feeds-cursor')).toBe('');
	expect(selectedRow.getAttribute('data-library-feeds-selected')).toBe('');
});

test('a played entry dims its title and an active one paints its resume percent', async () => {
	const container = await mountList(ENTRIES, { selected: null });

	expect(
		(
			container.querySelector(
				'[data-library-feeds-row-id="s2::e2"] [data-library-feeds-title]',
			) as HTMLElement
		).style.color,
	).toBe('var(--canvas-muted)');
	expect(
		container
			.querySelector('[data-library-feeds-row-id="s1::e3"]')
			?.querySelector('[data-library-feeds-progress]'),
	).toBeTruthy();
});

test('a loading tab paints its line and no row', async () => {
	const container = await mountList([], { loading: true });

	expect(container.querySelector('[data-library-feeds-loading]')).toBeTruthy();
	expect(paintedRows(container)).toEqual([]);
});

// add-gui-library-panel design D15, invariant 30: the Feeds column carries the
// Music list's header, naming the active pill as the carried scope.
test('the list paints the Feeds header with the active pill as its label', async () => {
	const container = await mountList([], {});

	expect(container.querySelector('[data-library-list-title]')?.textContent).toBe('Feeds');
	expect(container.querySelector('[data-library-feeds-selection-label]')?.textContent).toBe(
		'Latest',
	);
});

// A landed view that holds no entry paints the TUI's empty line, not a blank
// column.
test('a landed view with no entry paints the empty line', async () => {
	const container = await mountList([], {});

	expect(container.querySelector('[data-library-feeds-empty]')?.textContent).toContain(
		'No entries',
	);
});
