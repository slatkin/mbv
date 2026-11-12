// add-gui-library-panel row 6.2: the Home list (design D12, the USER-APPROVED
// GUI Home design) paints the flat list's sections — the heading rows in
// order, the item rows with their date gutter, and only the rows in view —
// and a press sends the select command with the row's opaque identity, a
// double-click the play command. Headings and spacers paint no control, so
// the cursor and a press can reach the item rows only. The Tauri IPC is
// mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiHomeDetail, GuiHomeList } from '$lib/bridge/types';
import { homeRowKey, HOME_ITEM_ROW_HEIGHT } from '$lib/library/home-list';
import { homeUi, initialHomeUi } from '$lib/library/home';
import { carryLibrary } from '$lib/bridge/test-library';
import type { LibrarySections } from '$lib/bridge/test-library';
import LibraryHomeList from './LibraryHomeList.svelte';

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

/** The carried content while Home is active (design D12): the frame the
 * panel paints and the row model reads for a cursor move. */
function carriedContent(): LibrarySections {
	return {
		list: {
			tabs: [{ position: 1, label: 'Music' }],
			currentTab: null,
			destination: { kind: 'home', ...homeFrame() },
		},
		detail: { kind: 'home', ...homeDetail() },
	};
}

/** The Home detail part: the first item row selected, and no Hero painted here. */
function homeDetail(): GuiHomeDetail {
	return { selected: { kind: 'continue', id: 'e1' }, hero: null };
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	homeUi.set(initialHomeUi());
	carryLibrary(carriedContent());
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** The two sections' item rows: the Emby row with art and a resume percent,
 * the Feed row without art, and the played row between them. */
function homeFrame(): GuiHomeList {
	return { loading: false,
		rows: [
			{ row: 'heading', text: 'Continue watching' },
			{
				row: 'item',
				id: { kind: 'continue', id: 'e1' },
				title: 'Better Call Saul',
				subtitle: 'S3E4 · Sabrosito',
				date: null,
				kind: 'media',
				state: { active: { progress: 42 } },
				art: { kind: 'image', src: 'mbv-art://localhost/card/x' },
			},
			{
				row: 'item',
				id: { kind: 'continue', id: 'e2' },
				title: 'The Wire',
				subtitle: null,
				date: null,
				kind: 'media',
				state: 'played',
				art: null,
			},
			{ row: 'spacer' },
			{ row: 'heading', text: 'Recently added' },
			{
				row: 'item',
				id: { kind: 'recent', id: { provider: 'Feed', value: 'f1' } },
				title: 'A Feed Entry',
				subtitle: 'The Feed',
				date: '12 May',
				kind: 'media',
				state: 'ordinary',
				art: null,
			},
		],
	};
}

/** Render the list and report a `viewportHeight` px viewport, so the rows in
 * view are the rows a measured list of that height paints: an unmeasured
 * viewport paints nothing, as the Music list does. */
async function mountList(viewportHeight = 40): Promise<HTMLElement> {
	const { container } = render(LibraryHomeList, { list: homeFrame(), detail: homeDetail() });
	const list = container.querySelector<HTMLElement>('[data-library-home-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	await tick();
	return container;
}

test('the sections paint their headings and the date gutter in order', async () => {
	const container = await mountList(300);

	const headings = [...container.querySelectorAll('[data-library-home-heading]')].map((el) =>
		el.textContent?.trim(),
	);
	expect(headings).toEqual(['Continue watching', 'Recently added']);
	expect(
		container
			.querySelector(`[data-library-home-row-id="${homeRowKey({ kind: 'recent', id: { provider: 'Feed', value: 'f1' } })}"] [data-library-home-date]`)
			?.textContent?.trim(),
	).toBe('12 May');
	expect(container.querySelector('[data-library-home-row="spacer"]')).toBeNull();
});

// The structural rows are divs, the item rows buttons (design D12): the
// cursor and a press reach the item rows only.
test('the headings and the spacer paint no control', async () => {
	const container = await mountList(300);

	const buttons = [...container.querySelectorAll('button')];
	expect(buttons.length).toBeGreaterThan(0);
	expect(buttons.every((button) => button.getAttribute('data-library-home-row') === 'item')).toBe(
		true,
	);
});

test('the list paints only the rows in view', async () => {
	const container = await mountList(HOME_ITEM_ROW_HEIGHT);

	expect(
		container.querySelector(`[data-library-home-row-id="${homeRowKey({ kind: 'continue', id: 'e1' })}"]`),
	).toBeTruthy();
	expect(
		container.querySelector(`[data-library-home-row-id="${homeRowKey({ kind: 'continue', id: 'e2' })}"]`),
	).toBeNull();
});

test('a press sends the select command with the row\u2019s opaque id', async () => {
	const container = await mountList(300);
	const row = container.querySelector(
		`[data-library-home-row-id="${homeRowKey({ kind: 'recent', id: { provider: 'Feed', value: 'f1' } })}"]`,
	);

	await fireEvent.click(row as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: {
			command: 'homeSelect',
			row: { kind: 'recent', id: { provider: 'Feed', value: 'f1' } },
		},
	});
});

test('a double-click sends the play command with the row\u2019s opaque id', async () => {
	const container = await mountList(300);
	const row = container.querySelector(
		`[data-library-home-row-id="${homeRowKey({ kind: 'continue', id: 'e1' })}"]`,
	);

	await fireEvent.dblClick(row as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: {
			command: 'homePlay',
			row: { kind: 'continue', id: 'e1' },
		},
	});
});

test('the rows without art paint no art box', async () => {
	const container = await mountList(300);

	expect(
		container.querySelector(
			`[data-library-home-row-id="${homeRowKey({ kind: 'continue', id: 'e1' })}"] [data-library-home-art]`,
		),
	).toBeTruthy();
	expect(
		container.querySelector(
			`[data-library-home-row-id="${homeRowKey({ kind: 'recent', id: { provider: 'Feed', value: 'f1' } })}"] [data-library-home-art]`,
		),
	).toBeNull();
});

test('the active row paints its resume percent and the played row none', async () => {
	const container = await mountList(300);

	expect(
		container.querySelector(
			`[data-library-home-row-id="${homeRowKey({ kind: 'continue', id: 'e1' })}"] [data-library-home-progress]`,
		),
	).toBeTruthy();
	expect(
		container.querySelector(
			`[data-library-home-row-id="${homeRowKey({ kind: 'continue', id: 'e2' })}"] [data-library-home-progress]`,
		),
	).toBeNull();
});

// add-gui-library-panel design D15: the Home column carries the Music list's
// header instead of opening with a bare filter field.
test('the list paints the Home header', async () => {
	const container = await mountList();

	expect(container.querySelector('[data-library-list-title]')?.textContent).toBe('Home');
	expect(container.querySelector('[data-library-selection-label]')?.textContent).toBe('All');
});

// add-gui-library-panel design D12: Home holds nothing while its content has not
// landed, and nothing at all when both sections are empty — each says so in one
// line rather than painting a blank column.
test('a loading Home paints the loading line', async () => {
	const list = { ...homeFrame(), loading: true, rows: [] };
	const { container } = render(LibraryHomeList, { list, detail: homeDetail() });

	expect(container.querySelector('[data-library-home-loading]')).toBeTruthy();
});

test('an empty Home paints the empty line', async () => {
	const list = { ...homeFrame(), rows: [] };
	const { container } = render(LibraryHomeList, { list, detail: homeDetail() });

	expect(container.querySelector('[data-library-home-empty]')?.textContent).toContain(
		'Nothing to play',
	);
});

// add-gui-library-panel design D8: a right-click opens the item menu for the
// row's stable id, which the core resolves and the panel paints (D15).
test('a right-click on a row sends the row-menu command with its id', async () => {
	const container = await mountList(300);
	const row = container.querySelector('[data-library-home-row-id="home-continue:e1"]') as HTMLElement;

	await fireEvent.contextMenu(row, { clientX: 5, clientY: 6 });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'rowMenu', itemId: 'e1' },
	});
});
