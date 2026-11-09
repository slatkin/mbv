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
import type { GuiHomeFrame, GuiLibraryContent } from '$lib/bridge/types';
import { homeRowKey, HOME_ITEM_ROW_HEIGHT } from '$lib/library/home-list';
import { carryLibraryContent } from '$lib/library/state';
import { homeUi, initialHomeUi } from '$lib/library/home';
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
function carriedContent(): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: null,
		music: null,
		home: homeFrame(),
		shows: null,
		emby: null,
		audiobooks: null,
		podcasts: null,
	};
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	homeUi.set(initialHomeUi());
	carryLibraryContent({ revision: 1, content: carriedContent() });
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** The two sections' item rows: the Emby row with art and a resume percent,
 * the Feed row without art, and the played row between them. */
function homeFrame(): GuiHomeFrame {
	return {
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
				art: { kind: 'image', detail: 'data:image/png;base64,x' },
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
		selected: { kind: 'continue', id: 'e1' },
		hero: null,
	};
}

/** Render the list and report a `viewportHeight` px viewport, so the rows in
 * view are the rows a measured list of that height paints: an unmeasured
 * viewport paints nothing, as the Music list does. */
async function mountList(viewportHeight = 40): Promise<HTMLElement> {
	const { container } = render(LibraryHomeList, { home: homeFrame() });
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
