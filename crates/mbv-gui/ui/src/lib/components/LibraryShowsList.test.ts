// add-gui-library-panel row 7.1: the Shows list (design D12, the USER-APPROVED
// GUI Shows design) paints the All series mode's letter headings and series
// rows with its rail, and the Latest and Upcoming modes' flat rows with their
// date gutter and no rail. A press sends the select command with the row's
// stable id and a double-click on a flat row the play command. The Tauri IPC is
// mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiShowsFrame, GuiShowsMode, GuiShowsPill, GuiShowsRow } from '$lib/bridge/types';
import { showsUi, initialShowsUi } from '$lib/library/shows';
import { carryLibraryContent } from '$lib/library/state';
import LibraryShowsList from './LibraryShowsList.svelte';

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

/** The pills the frame carries, with `mode` selected (design D12). */
function pills(mode: GuiShowsMode): GuiShowsPill[] {
	return [
		{ mode: 'latest', name: 'Latest', selected: mode === 'latest', collection: false },
		{ mode: 'upcoming', name: 'Upcoming', selected: mode === 'upcoming', collection: false },
		{ mode: 'all', name: 'All series', selected: mode === 'all', collection: false },
	];
}

/** The carried Shows content in `mode` over `rows` (design D12). */
function showsFrame(mode: GuiShowsMode, rows: GuiShowsRow[]): GuiShowsFrame {
	return {
		libIdx: 0,
		pills: pills(mode),
		collectionNames: [mode],
		loading: false,
		workspaceLoading: false,
		rows,
		selected: rows[0]?.id ?? null,
		hero: null,
	};
}

/** The All series mode's rows: two `A` series and one `B` series. */
const SERIES: GuiShowsRow[] = [
	{ id: 's1', title: 'Atlanta', subtitle: null, date: null, sortLetter: 'A' },
	{ id: 's2', title: 'The Bear', subtitle: null, date: null, sortLetter: 'A' },
	{ id: 's3', title: 'Better Call Saul', subtitle: null, date: null, sortLetter: 'B' },
];

/** The Latest mode's rows: the flat newest episodes with their dates. */
const EPISODES: GuiShowsRow[] = [
	{
		id: 'e1',
		title: 'Atlanta',
		subtitle: 'S3E4 · Sabrosito',
		date: '12 May',
		sortLetter: null,
	},
	{
		id: 'e2',
		title: 'The Bear',
		subtitle: 'S2E1 · Beef',
		date: '9 May',
		sortLetter: null,
	},
];

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	showsUi.set(initialShowsUi());
	carryLibraryContent({ revision: 1, content: null });
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** Carry the Shows content and render the list at a `viewportHeight` px
 * viewport: an unmeasured viewport paints nothing, as the Music list does. */
async function mountList(
	mode: GuiShowsMode,
	rows: GuiShowsRow[],
	viewportHeight = 300,
): Promise<HTMLElement> {
	const shows = showsFrame(mode, rows);
	carryLibraryContent({
		revision: 1,
		content: { tabs: [], currentTab: 2, music: null, home: null, shows, emby: null, audiobooks: null, podcasts: null },
	});
	const { container } = render(LibraryShowsList, { shows });
	const list = container.querySelector<HTMLElement>('[data-library-shows-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	tick();
	return container;
}

/** The row ids the list painted, in paint order. */
function paintedRows(container: HTMLElement): (string | null)[] {
	return [...container.querySelectorAll('[data-library-shows-row]')].map((el) =>
		el.getAttribute('data-library-shows-row-id'),
	);
}

// The All series mode paints the letter headings and the A–Z rail beside the
// list (design D12): a press on an enabled letter moves the cursor to that
// letter's first series row.
test('the All series mode paints letter headings, its rows, and the rail a press moves the cursor with', async () => {
	const container = await mountList('all', SERIES);

	expect(
		[...container.querySelectorAll('[data-library-shows-letter]')].map((el) =>
			el.getAttribute('data-library-shows-letter'),
		),
	).toEqual(['A', 'B']);
	expect(paintedRows(container)).toEqual(['s1', 's2', 's3']);

	await fireEvent.click(
		container.querySelector('[data-library-rail-letter="B"]') as HTMLElement,
	);
	await tick();

	expect(
		container
			.querySelector('[data-library-shows-row-id="s3"]')
			?.getAttribute('data-library-shows-cursor'),
	).toBe('');
});

// The Latest and Upcoming modes paint the flat rows with their date gutter and
// no rail and no heading (design D12).
test('the flat modes paint the date gutter and no rail', async () => {
	const container = await mountList('latest', EPISODES);

	expect(paintedRows(container)).toEqual(['e1', 'e2']);
	expect(container.querySelector('[data-library-shows-letter]')).toBeNull();
	expect(container.querySelector('[data-library-region="rail"]')).toBeNull();
	expect(
		container.querySelector('[data-library-shows-row-id="e1"] [data-library-shows-date]')
			?.textContent,
	).toBe('12 May');
});

test('a press sends the select command with the row id', async () => {
	const container = await mountList('all', SERIES);

	await fireEvent.click(
		container.querySelector('[data-library-shows-row-id="s2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsSelect', libIdx: 0, row: 's2' },
	});
});

test('a double-click on a flat row plays the episode', async () => {
	const container = await mountList('latest', EPISODES);

	await fireEvent.dblClick(
		container.querySelector('[data-library-shows-row-id="e2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsPlay', libIdx: 0, itemId: 'e2' },
	});
});
