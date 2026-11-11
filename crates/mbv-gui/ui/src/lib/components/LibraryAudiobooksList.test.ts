// add-gui-library-panel row 9.1: the book list (design D12) paints the
// catalog's letter headings with the A–Z rail, a press on an enabled letter
// moves the cursor to that letter's first book, and a press selects the book
// the Hero shows while a double-click plays it from its saved position. The
// cursor row and the core's selected book paint their fills. The Tauri IPC is
// mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiAudiobooksFrame, GuiAudiobooksRow } from '$lib/bridge/types';
import { resetAbsChapterCursor } from '$lib/library/audiobooks-workspace';
import { audiobooksCursorRow, audiobooksUi, initialAudiobooksUi } from '$lib/library/audiobooks';
import { carryLibraryContent, libraryUi, initialLibraryUi } from '$lib/library/state';
import LibraryAudiobooksList from './LibraryAudiobooksList.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

vi.mock('$lib/library/audiobooks-workspace', () => ({
	resetAbsChapterCursor: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const resetMock = vi.mocked(resetAbsChapterCursor);

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

/** The catalog's book rows: two under `A` and one under `B`. */
const CATALOG: GuiAudiobooksRow[] = [
	{ id: 'b1', title: 'Aftermath', subtitle: 'Bill Wyman', duration: '4:32', sortLetter: 'A' },
	{ id: 'b2', title: 'Audition', subtitle: 'Ryu Murakami', duration: '2:14', sortLetter: 'A' },
	{ id: 'b3', title: 'Blindsight', subtitle: 'Peter Watts', duration: '10:02', sortLetter: 'B' },
];

/** The carried Audiobooks content over `rows` (row 9.1). */
function frame(rows: GuiAudiobooksRow[], selected: string | null): GuiAudiobooksFrame {
	return {
		libIdx: 4,
		loading: false,
		failed: false,
		workspaceLoading: false,
		rows,
		selected,
		hero: null,
	};
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	resetMock.mockClear();
	audiobooksUi.set(initialAudiobooksUi());
	libraryUi.set(initialLibraryUi());
	carryLibraryContent({ revision: 1, search: null, content: null });
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** Carry the content and render the list at a `viewportHeight` px viewport:
 * an unmeasured viewport paints nothing, as the Music list does. */
async function mountList(
	rows: GuiAudiobooksRow[],
	selected: string | null = 'b1',
	viewportHeight = 300,
): Promise<HTMLElement> {
	carryLibraryContent({
		revision: 1,
		search: null,
		content: {
			tabs: [],
			currentTab: 4,
			music: null,
			home: null,
			shows: null,
			emby: null,
			audiobooks: frame(rows, selected),
			podcasts: null,
			feeds: null,
			rowMenu: null,
		},
	});
	const { container } = render(LibraryAudiobooksList, { audiobooks: frame(rows, selected) });
	const list = container.querySelector<HTMLElement>('[data-library-abs-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	tick();
	return container;
}

/** The row ids the list painted, in paint order. */
function paintedRows(container: HTMLElement): (string | null)[] {
	return [...container.querySelectorAll('[data-library-abs-row]')].map((el) =>
		el.getAttribute('data-library-abs-row-id'),
	);
}

test('the list paints letter headings, its rows, and the rail a press moves the cursor with', async () => {
	const container = await mountList(CATALOG);

	expect(
		[...container.querySelectorAll('[data-library-abs-letter]')].map((el) =>
			el.getAttribute('data-library-abs-letter'),
		),
	).toEqual(['A', 'B']);
	expect(paintedRows(container)).toEqual(['b1', 'b2', 'b3']);

	await fireEvent.click(container.querySelector('[data-library-rail-letter="B"]') as HTMLElement);
	await tick();

	expect(audiobooksCursorRow()?.id).toBe('b3');
});

test('a rail letter no row carries is disabled', async () => {
	const container = await mountList(CATALOG);

	const disabled = container.querySelector('[data-library-rail-letter="C"]');
	expect(disabled?.hasAttribute('disabled')).toBe(true);

	await fireEvent.click(disabled as HTMLElement);
	await tick();

	expect(audiobooksCursorRow()).toBeNull();
});

test('a press selects the book and sends the select command', async () => {
	const container = await mountList(CATALOG);

	await fireEvent.click(container.querySelector('[data-library-abs-row-id="b2"]') as HTMLElement);
	await tick();

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absSelect', libIdx: 4, row: 'b2' },
	});
});

test('a double-click plays the book from its saved position', async () => {
	const container = await mountList(CATALOG);

	await fireEvent.dblClick(
		container.querySelector('[data-library-abs-row-id="b3"]') as HTMLElement,
	);
	await tick();

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlay', libIdx: 4, itemId: 'b3' },
	});
});

test('the cursor row and the selected book paint their fills', async () => {
	const container = await mountList(CATALOG, 'b3');
	audiobooksUi.update((ui) => ({ ...ui, cursor: 'b1' }));
	await tick();

	const cursor = container.querySelector('[data-library-abs-row-id="b1"]');
	const selected = container.querySelector('[data-library-abs-row-id="b3"]');

	expect(cursor?.getAttribute('data-library-abs-cursor')).toBe('');
	expect(selected?.getAttribute('data-library-abs-selected')).toBe('');
	expect((cursor as HTMLElement).style.background).not.toBe('');
	expect((selected as HTMLElement).style.background).not.toBe('');
});

test('loading and failed paint their placeholder lines', () => {
	const rows: GuiAudiobooksRow[] = [];
	return (async () => {
		const container = await mountList(rows, null);
		expect(container.querySelector('[data-library-abs-loading]')).toBeNull();
		expect(container.querySelector('[data-library-abs-failed]')).toBeNull();

		carryLibraryContent({
			revision: 1,
			search: null,
			content: {
				tabs: [],
				currentTab: 4,
				music: null,
				home: null,
				shows: null,
				emby: null,
				audiobooks: {
					...frame(rows, null),
					loading: true,
					failed: true,
				},
				podcasts: null,
				feeds: null,
				rowMenu: null,
			},
		});
		const failed = render(LibraryAudiobooksList, {
			audiobooks: { ...frame(rows, null), loading: true, failed: true },
		});
		expect(
			failed.container.querySelector('[data-library-abs-loading]'),
		).toBeTruthy();
		expect(failed.container.querySelector('[data-library-abs-failed]')).toBeTruthy();
	})();
});

// A selection change starts the Hero's chapter cursor over (the
// `audiobooks-workspace` contract): the chapters it rested on belong to the
// previous book.
test('a selection change calls resetAbsChapterCursor', () => {
	carryLibraryContent({
		revision: 1,
		search: null,
		content: {
			tabs: [],
			currentTab: 4,
			music: null,
			home: null,
			shows: null,
			emby: null,
			audiobooks: frame(CATALOG, 'b2'),
			podcasts: null,
			feeds: null,
			rowMenu: null,
		},
	});
	render(LibraryAudiobooksList, { audiobooks: frame(CATALOG, 'b2') });

	expect(resetMock).toHaveBeenCalledTimes(1);
});

// add-gui-library-panel design D15: the Audiobooks column carries the Music
// list's header instead of opening with a bare filter field.
test('the list paints the Audiobooks header', async () => {
	const container = await mountList(CATALOG);

	expect(container.querySelector('[data-library-list-title]')?.textContent).toBe('Audiobooks');
	expect(container.querySelector('[data-library-selection-label]')?.textContent).toBe('All');
});
