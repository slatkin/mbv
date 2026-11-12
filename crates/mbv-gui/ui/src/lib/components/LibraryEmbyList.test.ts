// add-gui-library-panel row 8.1: the plain Emby-library list (design D12) paints
// the plain mode's name-sorted rows under their letter headings with the A–Z
// rail, and the feed-group mode's flat rows with their date gutter and no rail.
// A press sends the select command with the row's stable id and a double-click
// the play command, a collection row included. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiEmbyFrame, GuiEmbyRow } from '$lib/bridge/types';
import { embyUi, initialEmbyUi } from '$lib/library/emby';
import { library } from '$lib/bridge/sections';
import LibraryEmbyList from './LibraryEmbyList.svelte';

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

/** The plain mode's name-sorted rows: two `A` movies and one `B` movie. */
const MOVIES: GuiEmbyRow[] = [
	{ id: 'm1', title: 'Alien', subtitle: '1979', date: null, sortLetter: 'A', kind: 'media' },
	{ id: 'm2', title: 'Amélie', subtitle: '2001', date: null, sortLetter: 'A', kind: 'media' },
	{ id: 'm3', title: 'Brazil', subtitle: '1985', date: null, sortLetter: 'B', kind: 'media' },
];

/** The feed-group mode's date-ordered rows, with a boxset collection row. */
const FEED: GuiEmbyRow[] = [
	{ id: 'f1', title: 'Family', subtitle: null, date: '1998', sortLetter: null, kind: 'collection' },
	{ id: 'f2', title: 'Fargo', subtitle: null, date: '1996', sortLetter: null, kind: 'media' },
];

/** The carried Emby content in one mode over `rows` (row 8.1). */
function embyFrame(feedGroup: boolean, rows: GuiEmbyRow[]): GuiEmbyFrame {
	return {
		libIdx: 3,
		libraryName: 'Movies',
		pills: feedGroup
			? [
					{ group: { kind: 'all' }, name: 'All', selected: true, collection: false },
					{
						group: { kind: 'folder', id: 'folder-1' },
						name: 'Family',
						selected: false,
						collection: false,
					},
				]
			: [],
		collectionNames: feedGroup ? ['All'] : [],
		feedGroup,
		loading: false,
		failed: false,
		rows,
		selected: rows[0]?.id ?? null,
		hero: null,
	};
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	embyUi.set(initialEmbyUi());
	library.set(null);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** Carry the Emby content and render the list at a `viewportHeight` px
 * viewport: an unmeasured viewport paints nothing, as the Music list does. */
async function mountList(
	feedGroup: boolean,
	rows: GuiEmbyRow[],
	viewportHeight = 300,
): Promise<HTMLElement> {
	const emby = embyFrame(feedGroup, rows);
	library.set({ tabs: [], currentTab: 3, music: null, home: null, shows: null, emby, audiobooks: null, podcasts: null, feeds: null, rowMenu: null });
	const { container } = render(LibraryEmbyList, { emby });
	const list = container.querySelector<HTMLElement>('[data-library-emby-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	tick();
	return container;
}

/** The row ids the list painted, in paint order. */
function paintedRows(container: HTMLElement): (string | null)[] {
	return [...container.querySelectorAll('[data-library-emby-row]')].map((el) =>
		el.getAttribute('data-library-emby-row-id'),
	);
}

// The plain mode paints the letter headings with the rail beside the list (row
// 8.1): a press on an enabled letter moves the cursor to that letter's first
// row.
test('the plain mode paints letter headings, its rows, and the rail a press moves the cursor with', async () => {
	const container = await mountList(false, MOVIES);

	expect(
		[...container.querySelectorAll('[data-library-emby-letter]')].map((el) =>
			el.getAttribute('data-library-emby-letter'),
		),
	).toEqual(['A', 'B']);
	expect(paintedRows(container)).toEqual(['m1', 'm2', 'm3']);

	await fireEvent.click(container.querySelector('[data-library-rail-letter="B"]') as HTMLElement);
	await tick();

	expect(
		container
			.querySelector('[data-library-emby-row-id="m3"]')
			?.getAttribute('data-library-emby-cursor'),
	).toBe('');
});

test('the plain mode paints the row\u2019s year as its secondary label', async () => {
	const container = await mountList(false, MOVIES);

	expect(
		container.querySelector('[data-library-emby-row-id="m1"] [data-library-emby-subtitle]')
			?.textContent,
	).toBe('1979');
	expect(container.querySelector('[data-library-emby-date]')).toBeNull();
});

// The feed-group mode paints the flat date-ordered rows with their gutter and
// no rail and no heading (row 8.1).
test('the feed-group mode paints the date gutter and no rail', async () => {
	const container = await mountList(true, FEED);

	expect(paintedRows(container)).toEqual(['f1', 'f2']);
	expect(container.querySelector('[data-library-emby-letter]')).toBeNull();
	expect(container.querySelector('[data-library-region="rail"]')).toBeNull();
	expect(
		container.querySelector('[data-library-emby-row-id="f1"] [data-library-emby-date]')
			?.textContent,
	).toBe('1998');
});

test('a press sends the select command with the row id', async () => {
	const container = await mountList(false, MOVIES);

	await fireEvent.click(container.querySelector('[data-library-emby-row-id="m2"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embySelect', libIdx: 3, row: 'm2' },
	});
});

test('a double-click on a row plays it', async () => {
	const container = await mountList(false, MOVIES);

	await fireEvent.dblClick(container.querySelector('[data-library-emby-row-id="m3"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlay', libIdx: 3, itemId: 'm3' },
	});
});

// A folder row plays through the core's folder-play rule with no drill-in (row
// 8.1), so the double-click sends the same play command as any other row.
test('a double-click on a collection row plays it', async () => {
	const container = await mountList(true, FEED);

	await fireEvent.dblClick(container.querySelector('[data-library-emby-row-id="f1"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlay', libIdx: 3, itemId: 'f1' },
	});
});

test('a list that has not landed paints the loading line', async () => {
	const emby = { ...embyFrame(false, []), loading: true };
	library.set({ tabs: [], currentTab: 3, music: null, home: null, shows: null, emby, audiobooks: null, podcasts: null, feeds: null, rowMenu: null });

	const { container } = render(LibraryEmbyList, { emby });

	expect(container.querySelector('[data-library-emby-loading]')).toBeTruthy();
});

test('a failed plain-mode fetch paints the failure line', async () => {
	const emby = { ...embyFrame(false, []), failed: true };
	library.set({ tabs: [], currentTab: 3, music: null, home: null, shows: null, emby, audiobooks: null, podcasts: null, feeds: null, rowMenu: null });

	const { container } = render(LibraryEmbyList, { emby });

	expect(container.querySelector('[data-library-emby-failed]')).toBeTruthy();
});

// add-gui-library-panel design D15: the list column carries the Music list's
// header — the library's name, the carried selection's label, and the `/`
// field — so the plain Emby destination stopped opening with a bare filter.
test('the list paints the library header with the selection label', async () => {
	const emby = embyFrame(false, MOVIES);
	library.set({ tabs: [], currentTab: 3, music: null, home: null, shows: null, emby, audiobooks: null, podcasts: null, feeds: null, rowMenu: null });

	const { container } = render(LibraryEmbyList, { emby });

	expect(container.querySelector('[data-library-list-title]')?.textContent).toBe('Movies');
	expect(container.querySelector('[data-library-emby-selection-label]')?.textContent).toBe('All');
});

// add-gui-library-panel row 8.1: a library that landed empty says so, as the
// Music list's `No artists` line does, instead of painting a blank column.
test('a landed library with no row paints the empty line', async () => {
	const emby = embyFrame(false, []);
	library.set({ tabs: [], currentTab: 3, music: null, home: null, shows: null, emby, audiobooks: null, podcasts: null, feeds: null, rowMenu: null });

	const { container } = render(LibraryEmbyList, { emby });

	expect(container.querySelector('[data-library-emby-empty]')?.textContent).toContain('No items');
});

// add-gui-library-panel design D8: a right-click opens the item menu for the
// row's stable id, which the panel paints (D15).
test('a right-click on a row sends the row-menu command with its id', async () => {
	const container = await mountList(false, MOVIES);
	const row = container.querySelector('[data-library-emby-row-id="m2"]') as HTMLElement;

	await fireEvent.contextMenu(row, { clientX: 12, clientY: 34 });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'rowMenu', itemId: 'm2' },
	});
});
