// add-gui-library-panel row 8.1: the plain Emby-library Hero (design D12) paints
// the selected row's facts — the title, the meta rows with their duration and
// progress roles, and the overview — and places the artwork box per the frame's
// `artShape`. The actions send the Emby commands with the frame's selected row
// and its library index, and no Workspace paints. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

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
function hero(artShape: GuiEmbyHero['artShape'] = 'landscape'): GuiEmbyHero {
	return {
		title: 'Heat',
		metaRows: ['2 h 50 m', '1995 · Crime'],
		durationRow: 0,
		progressRow: 1,
		overview: 'A crew plans a bank job.',
		artShape,
		art: null,
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

test('the Hero paints its title, its meta rows with their roles, and its overview', () => {
	const container = mount();

	expect(container.querySelector('[data-library-emby-hero-title]')?.textContent).toBe('Heat');
	const meta = [...container.querySelectorAll('[data-library-emby-hero-role]')];
	expect(meta.map((row) => row.textContent)).toEqual(['2 h 50 m', '1995 · Crime']);
	expect(meta.map((row) => row.getAttribute('data-library-emby-hero-role'))).toEqual([
		'duration',
		'progress',
	]);
	expect(container.querySelector('[data-library-emby-hero-overview]')?.textContent).toBe(
		'A crew plans a bank job.',
	);
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
