// add-gui-library-panel row 10.1: the Podcasts list (design D12) paints the
// flat episode rows with their date gutter and no letter heading and no rail,
// the cursor row's fill and the core's selected episode's raised fill, and a
// press selects while a double-click plays. Its `loading` and `failed` facts
// paint their own lines. The Tauri IPC is mocked; the asserted payloads are
// the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { initialListScroll, podcastsScroll } from '$lib/library/scroll';
import type { GuiPodcastsDetail, GuiPodcastsList, GuiPodcastsRow } from '$lib/bridge/types';
import { initialPodcastsUi, podcastsUi } from '$lib/library/podcasts';
import { carryLibrary } from '$lib/bridge/test-library';
import LibraryPodcastsList from './LibraryPodcastsList.svelte';

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

/** The Latest pill's merged rows: two dated episodes, one played, and one
 * partly played with its resume percent. */
const EPISODES: GuiPodcastsRow[] = [
	{ id: 'e1:ep1', title: 'Episode A', subtitle: 'Show One', date: '12 May', state: 'ordinary' },
	{ id: 'e2:ep2', title: 'Episode B', subtitle: 'Show Two', date: '9 May', state: 'played' },
	{
		id: 'e3:ep3',
		title: 'Episode C',
		subtitle: 'Show One',
		date: '4 May',
		state: { active: { progress: 40 } },
	},
];

/** The carried Podcasts content over `rows`, with the frame's other facts as
 * the caller sets them (design D12). */
function frame(
	rows: GuiPodcastsRow[],
	facts: { loading?: boolean; failed?: boolean } = {},
): GuiPodcastsList {
	return {
		libIdx: 0,
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false },
		],
		loading: facts.loading ?? false,
		failed: facts.failed ?? false,
		rows,
	};
}

/** The Podcasts detail part: the first row selected unless `selected` names
 * one, and no Hero. */
function detail(
	rows: GuiPodcastsRow[],
	selected: string | null = rows[0]?.id ?? null,
): GuiPodcastsDetail {
	return { selected, hero: null };
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	podcastsUi.set(initialPodcastsUi());
	podcastsScroll.set(initialListScroll());
	carryLibrary(null);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** Carry the Podcasts content and render the list at a `viewportHeight` px
 * viewport: an unmeasured viewport paints nothing, as the Music list does. */
async function mountList(
	rows: GuiPodcastsRow[],
	facts: { loading?: boolean; failed?: boolean; selected?: string | null } = {},
	viewportHeight = 300,
): Promise<HTMLElement> {
	const podcasts = frame(rows, facts);
	const picked = detail(rows, facts.selected);
	carryLibrary({
		list: { tabs: [], currentTab: 2, destination: { kind: 'podcasts', ...podcasts } },
		detail: { kind: 'podcasts', ...picked },
	});
	const { container } = render(LibraryPodcastsList, { list: podcasts, detail: picked });
	const list = container.querySelector<HTMLElement>('[data-library-podcasts-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	tick();
	return container;
}

/** The row ids the list painted, in paint order. */
function paintedRows(container: HTMLElement): (string | null)[] {
	return [...container.querySelectorAll('[data-library-podcasts-row]')].map((el) =>
		el.getAttribute('data-library-podcasts-row-id'),
	);
}

// The date-ordered list paints the date gutter and no letter heading and no
// rail (design D12, row 10.1), the episode title over its show.
test('the flat list paints its date gutter with no heading and no rail', async () => {
	const container = await mountList(EPISODES);

	expect(paintedRows(container)).toEqual(['e1:ep1', 'e2:ep2', 'e3:ep3']);
	expect(container.querySelector('[data-library-region="rail"]')).toBeNull();
	expect(container.querySelector('[data-library-podcasts-letter]')).toBeNull();
	expect(
		container.querySelector('[data-library-podcasts-row-id="e2:ep2"] [data-library-podcasts-date]')
			?.textContent,
	).toBe('9 May');
	expect(
		container.querySelector('[data-library-podcasts-row-id="e1:ep1"] [data-library-podcasts-subtitle]')
			?.textContent,
	).toBe('Show One');
});

test('a press sends the select command with the row id', async () => {
	const container = await mountList(EPISODES);

	await fireEvent.click(
		container.querySelector('[data-library-podcasts-row-id="e2:ep2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsSelect', libIdx: 0, row: 'e2:ep2' },
	});
});

test('a double-click plays the episode', async () => {
	const container = await mountList(EPISODES);

	await fireEvent.dblClick(
		container.querySelector('[data-library-podcasts-row-id="e2:ep2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsPlay', libIdx: 0, itemId: 'e2:ep2' },
	});
});

test('the cursor row and the core\'s selected episode carry their own fills', async () => {
	const container = await mountList(EPISODES);
	podcastsUi.update((ui) => ({ ...ui, cursor: 'e2:ep2' }));
	await tick();

	const cursorRow = container.querySelector(
		'[data-library-podcasts-row-id="e2:ep2"]',
	) as HTMLElement;
	const selectedRow = container.querySelector(
		'[data-library-podcasts-row-id="e1:ep1"]',
	) as HTMLElement;

	expect(cursorRow.getAttribute('data-library-podcasts-cursor')).toBe('');
	expect(selectedRow.getAttribute('data-library-podcasts-selected')).toBe('');
});

test('a played episode dims its title and an active one paints its resume percent', async () => {
	const container = await mountList(EPISODES, { selected: null });

	expect(
		(
			container.querySelector(
				'[data-library-podcasts-row-id="e2:ep2"] [data-library-podcasts-title]',
			) as HTMLElement
		).style.color,
	).toBe('var(--canvas-muted)');
	expect(
		container
			.querySelector('[data-library-podcasts-row-id="e3:ep3"]')
			?.querySelector('[data-library-podcasts-progress]'),
	).toBeTruthy();
});

test('loading and failed facts paint their own lines', async () => {
	const container = await mountList([], { loading: true, failed: true });

	expect(container.querySelector('[data-library-podcasts-loading]')).toBeTruthy();
	expect(container.querySelector('[data-library-podcasts-failed]')).toBeTruthy();
	expect(paintedRows(container)).toEqual([]);
});

// add-gui-library-panel design D15: the Podcasts column carries the Music list's
// header, naming the active pill as the carried scope.
test('the list paints the Podcasts header with the active pill as its label', async () => {
	const container = await mountList([], {});

	expect(container.querySelector('[data-library-list-title]')?.textContent).toBe('Podcasts');
	expect(container.querySelector('[data-library-podcasts-selection-label]')?.textContent).toBe(
		'Latest',
	);
});

// add-gui-library-panel row 10.1: an active view that holds no episode paints
// the TUI's empty line instead of a blank column.
test('a landed view with no episode paints the empty line', async () => {
	const container = await mountList([], {});

	expect(container.querySelector('[data-library-podcasts-empty]')?.textContent).toContain(
		'No episodes',
	);
});
