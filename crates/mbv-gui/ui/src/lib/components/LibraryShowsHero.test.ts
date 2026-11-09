// add-gui-library-panel row 7.1: the Shows Hero and the series Hero's
// Workspace (design D12) — the artwork box per art shape, the facts, the season
// pills with the core's selection pressed, and the selected season's episode
// rows with their cursor and hover actions. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
	GuiShowsEpisodeRow,
	GuiShowsFrame,
	GuiShowsHero,
	GuiShowsWorkspace,
} from '$lib/bridge/types';
import { initialShowsWorkspace, showsWorkspace } from '$lib/library/shows-workspace';
import LibraryShowsHero from './LibraryShowsHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	showsWorkspace.set(initialShowsWorkspace());
});

/** The selected season's episodes: one played, one untouched. */
const EPISODES: GuiShowsEpisodeRow[] = [
	{ episodeId: 'ep1', number: 1, name: 'Pilot', duration: '58:00', state: 'played' },
	{ episodeId: 'ep2', number: 2, name: 'Cat in the Bag', duration: '47:00', state: 'ordinary' },
];

/** The series Hero's Workspace: two seasons, the first selected. */
function workspace(): GuiShowsWorkspace {
	return {
		seasons: [
			{ seasonId: 's1', name: 'Season 1', selected: true },
			{ seasonId: 's2', name: 'Season 2', selected: false },
		],
		selectedSeason: 's1',
		episodes: EPISODES,
	};
}

/** The selected row's Hero facts: the series' facts with its Workspace. */
function hero(overrides: Partial<GuiShowsHero> = {}): GuiShowsHero {
	return {
		title: 'Better Call Saul',
		metaRows: ['TV Series', '5 seasons'],
		durationRow: null,
		progressRow: null,
		overview: 'A lawyer turns to the wrong clients.',
		artShape: 'landscape',
		art: null,
		workspace: workspace(),
		...overrides,
	};
}

/** The carried Shows content with the hero picked. */
function shows(facts: GuiShowsHero | null): GuiShowsFrame {
	return {
		libIdx: 2,
		pills: [],
		collectionNames: ['All series'],
		loading: false,
		workspaceLoading: false,
		rows: [],
		selected: 'series1',
		hero: facts,
	};
}

/** Render the Shows Hero for the carried content. */
function mount(facts: GuiShowsHero | null = hero()): HTMLElement {
	return render(LibraryShowsHero, { shows: shows(facts) }).container;
}

/** One episode row of the painted Workspace. */
function episodeRow(container: HTMLElement, episodeId: string): HTMLElement {
	return container.querySelector(
		`[data-library-shows-episode="${episodeId}"]`,
	) as HTMLElement;
}

// The artwork box reserves the arm the frame declares (design D12): the
// series' landscape box keeps the 16:9 height, the square the Music Hero's
// 300 px side, and the portrait the tall box.
test.each([
	['landscape', 169],
	['square', 300],
	['portrait', 450],
] as const)('a %s Hero places its artwork box at %s px high', (shape, height) => {
	const container = mount(hero({ artShape: shape }));
	const box = container.querySelector('[data-library-shows-hero-art]') as HTMLElement;

	expect(box.style.width).toBe('300px');
	expect(box.style.height).toBe(`${height}px`);
});

test('the Hero paints its title, its meta rows, and its overview', () => {
	const container = mount(hero({ durationRow: 1 }));

	expect(container.querySelector('[data-library-shows-hero-title]')?.textContent).toBe(
		'Better Call Saul',
	);
	const meta = [...container.querySelectorAll('[data-library-shows-hero-role]')];
	expect(meta.map((row) => row.textContent)).toEqual(['TV Series', '5 seasons']);
	expect(meta[1]?.getAttribute('data-library-shows-hero-role')).toBe('duration');
	expect(container.querySelector('[data-library-shows-hero-overview]')?.textContent).toBe(
		'A lawyer turns to the wrong clients.',
	);
});

test('the Workspace paints the season pills with the core selection pressed', () => {
	const container = mount();

	const pills = [...container.querySelectorAll('[data-library-shows-season]')];
	expect(pills.map((pill) => pill.textContent?.trim())).toEqual(['Season 1', 'Season 2']);
	expect(pills.map((pill) => pill.getAttribute('aria-pressed'))).toEqual(['true', 'false']);
});

test("the Workspace paints the selected season's episodes", () => {
	const container = mount();

	const rows = [...container.querySelectorAll('[data-library-shows-episode]')];
	expect(rows.map((row) => row.getAttribute('data-library-shows-episode'))).toEqual([
		'ep1',
		'ep2',
	]);
	expect(episodeRow(container, 'ep2').querySelector('[data-library-shows-episode-number]')?.textContent).toBe('2');
	expect(episodeRow(container, 'ep2').querySelector('[data-library-shows-episode-name]')?.textContent?.trim()).toBe('Cat in the Bag');
	expect(episodeRow(container, 'ep2').querySelector('[data-library-shows-episode-duration]')?.textContent).toBe('47:00');
	expect(episodeRow(container, 'ep1').getAttribute('data-library-shows-episode-state')).toBe('played');
});

test("the Latest mode's Hero paints its facts and no Workspace", () => {
	const container = mount(hero({ workspace: null }));

	expect(container.querySelector('[data-library-shows-hero-title]')?.textContent).toBe(
		'Better Call Saul',
	);
	expect(container.querySelector('[data-library-shows-workspace]')).toBeNull();
});

test('a Hero that paints none paints nothing', () => {
	const container = mount(null);

	expect(container.querySelector('[data-library-shows-hero]')).toBeNull();
});

test('a season press sends the select-season command', async () => {
	const container = mount();

	await fireEvent.click(
		container.querySelector('[data-library-shows-season="s2"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsSelectSeason', libIdx: 2, seasonId: 's2' },
	});
});

test('a double-click on an episode sends its play command', async () => {
	const container = mount();

	await fireEvent.dblClick(
		episodeRow(container, 'ep2').querySelector('[data-library-shows-episode-name]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsPlay', libIdx: 2, itemId: 'ep2' },
	});
});

test('a hovered row plays its episode next', async () => {
	const container = mount();
	const row = episodeRow(container, 'ep2');
	await fireEvent.pointerEnter(row);

	await fireEvent.click(
		row.querySelector('[data-library-shows-episode-action="playNext"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsPlayNext', libIdx: 2, itemId: 'ep2' },
	});
});

test('a hovered row adds its episode to the queue', async () => {
	const container = mount();
	const row = episodeRow(container, 'ep2');
	await fireEvent.pointerEnter(row);

	await fireEvent.click(
		row.querySelector('[data-library-shows-episode-action="queue"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsQueue', libIdx: 2, itemId: 'ep2' },
	});
});

test('a press on an episode moves the cursor onto its row', async () => {
	const container = mount();

	await fireEvent.click(
		episodeRow(container, 'ep2').querySelector('[data-library-shows-episode-name]') as HTMLElement,
	);

	expect(get(showsWorkspace).cursor).toBe('ep2');
});
