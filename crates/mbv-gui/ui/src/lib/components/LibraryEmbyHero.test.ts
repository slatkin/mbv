// add-gui-library-panel row 8.1: the plain Emby-library Hero (design D12, D17)
// paints the shared frame — the header's kind kicker, title, and plain facts,
// the block's duration and progress rows beside the artwork the `artShape` arm
// places, the action row, and the overview — and the movie's credits table with
// the IMDb button under the block, as the TUI's hero does. The actions send the
// Emby commands with the frame's selected row and its library index, and no
// Workspace paints. The Tauri IPC is mocked; the asserted payloads are the serde
// wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiEmbyFrame, GuiEmbyHero } from '$lib/bridge/types';
import LibraryEmbyHero from './LibraryEmbyHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** The library index the Emby commands name. */
const LIB_IDX = 3;

/** The selected row's Hero facts: two meta rows, with the runtime and the
 * resume roles marked, and the overview text (design D12, row 8.1). */
function hero(
	artShape: GuiEmbyHero['artShape'] = 'landscape',
	extra: Partial<GuiEmbyHero> = {},
): GuiEmbyHero {
	return {
		kind: 'movie',
		title: 'Heat',
		metaRows: [[{ text: '2 h 50 m', kind: 'time' }], [{ text: '1995 · Crime', kind: 'plain' }]],
		durationRow: 0,
		progressRow: 1,
		overview: 'A crew plans a bank job.',
		artShape,
		art: null,
		credits: [],
		imdbUrl: null,
		...extra,
	};
}

/** The carried plain Emby-library content with the hero picked. */
function emby(facts: GuiEmbyHero | null): GuiEmbyFrame {
	return {
		libIdx: LIB_IDX,
		libraryName: 'Movies',
		pills: [],
		collectionNames: [],
		feedGroup: false,
		loading: false,
		failed: false,
		rows: [{ id: 'movie1', title: 'Heat', subtitle: '1995', date: null, sortLetter: 'H', kind: 'media' }],
		selected: 'movie1',
		hero: facts,
	};
}

/** Render the Hero for the carried content. */
function mount(facts: GuiEmbyHero | null = hero()): HTMLElement {
	return render(LibraryEmbyHero, { emby: emby(facts) }).container;
}

// The artwork box reserves the arm the frame declares (design D15): a portrait
// or square cover keeps the Music Hero's 300 px box at the aspect's height, and
// a landscape still fills the Hero content width at 16:9, as the TUI's hero
// does — a 16:9 still shrunk into the square box is not the TUI's hero.
test.each([
	['square', 300],
	['portrait', 450],
] as const)('a %s Hero places its artwork box at 300 px by %s px', (shape, height) => {
	const container = mount(hero(shape));
	const box = container.querySelector('[data-library-emby-hero-art]') as HTMLElement;

	expect(box.style.width).toBe('300px');
	expect(box.style.height).toBe(`${height}px`);
});

test('a landscape Hero fills the content width at 16:9 instead of the 300 px box', () => {
	const container = mount(hero('landscape'));
	const box = container.querySelector('[data-library-emby-hero-art]') as HTMLElement;

	expect(box.style.width).toBe('100%');
	expect(box.style.aspectRatio).toBe('16 / 9');
	expect(box.style.height).toBe('auto');
});

// Invariant 30: the header's big title is the movie itself (a movie is its own
// largest unit), and the kicker is the item's kind word in upper case.
test('the header paints the item kind\'s kicker and the movie\'s title', () => {
	const container = mount();

	expect(container.querySelector('[data-library-emby-hero-kicker]')?.textContent).toBe('MOVIE');
	expect(container.querySelector('[data-library-emby-hero-title]')?.textContent).toBe('Heat');
});

// The movie's credits table and the IMDb link paint as the TUI's hero shows
// them (design D17, Q7), and a press on the IMDb button sends the open-url
// command with the url the frame carries.
test('the Hero paints its credits table and the IMDb button, and a press sends the open-url command', async () => {
	const container = mount(
		hero('landscape', {
			credits: [
				{ name: 'Michael Mann', role: 'Director' },
				{ name: 'Al Pacino', role: 'Lt. Vincent Hanna' },
			],
			imdbUrl: 'https://www.imdb.com/title/tt0113277/',
		}),
	);

	const link = container.querySelector('[data-library-emby-imdb]') as HTMLElement;
	expect(link.textContent).toBe('IMDb');
	expect(link.getAttribute('data-library-emby-imdb')).toBe('https://www.imdb.com/title/tt0113277/');
	const names = [...container.querySelectorAll('[data-library-emby-hero-credit-name]')];
	const roles = [...container.querySelectorAll('[data-library-emby-hero-credit-role]')];
	expect(names.map((row) => row.textContent)).toEqual(['Michael Mann', 'Al Pacino']);
	expect(roles.map((row) => row.textContent)).toEqual(['Director', 'Lt. Vincent Hanna']);

	await fireEvent.click(link);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openUrl', url: 'https://www.imdb.com/title/tt0113277/' },
	});
});

test('a Hero that paints none paints nothing', () => {
	const container = mount(null);

	expect(container.querySelector('[data-library-emby-hero]')).toBeNull();
});

// A plain Emby library carries no Workspace and no tracklist: a folder or a
// boxset plays through the folder-play rule, so the Hero is one block.
test('the Hero paints no Workspace and no tracklist', () => {
	const container = mount();

	expect(container.querySelector('[data-library-shows-workspace]')).toBeNull();
	expect(container.querySelector('[data-library-tracklist]')).toBeNull();
});

test('Play sends the play command for the selected row', async () => {
	const container = mount();

	await fireEvent.click(container.querySelector('[data-library-emby-hero-play]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlay', libIdx: LIB_IDX, itemId: 'movie1' },
	});
});

test('Play next sends the play-next command for the selected row', async () => {
	const container = mount();

	await fireEvent.click(
		container.querySelector('[data-library-emby-hero-play-next]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyPlayNext', libIdx: LIB_IDX, itemId: 'movie1' },
	});
});

test('Add to queue sends the enqueue command for the selected row', async () => {
	const container = mount();

	await fireEvent.click(container.querySelector('[data-library-emby-hero-add]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'embyQueue', libIdx: LIB_IDX, itemId: 'movie1' },
	});
});
