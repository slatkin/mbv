// add-gui-library-panel row 6.2: the Home Hero (design D12, the USER-APPROVED
// GUI Home design) paints the selected row's facts — the title, the meta rows
// with their duration and progress roles, and the overview — and places the
// artwork box per the frame's `artShape`. The actions send the Home commands
// with the frame's selected row, the core's selection, and no Workspace
// paints. The Tauri IPC is mocked; the asserted payloads are the serde wire
// shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiHomeFrame, GuiHomeHero } from '$lib/bridge/types';
import { initialLibraryUi, libraryUi } from '$lib/library/state';
import LibraryHomeHero from './LibraryHomeHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	libraryUi.set(initialLibraryUi());
});

/** The selected row's identity: the Hero's actions name it. */
const SELECTED = { kind: 'continue', id: 'e1' } as const;

/** The selected row's Hero facts: two meta rows, with the duration and the
 * resume roles marked, and the overview text (design D12). */
function hero(artShape: GuiHomeHero['artShape'] = 'landscape'): GuiHomeHero {
	return {
		title: 'Better Call Saul',
		metaRows: ['1 h 47 m left', 'S3E4 · Sabrosito'],
		durationRow: 0,
		progressRow: null,
		overview: 'Jimmy turns to paying work.',
		artShape,
		art: null,
	};
}

/** The carried Home content with the hero picked. */
function home(facts: GuiHomeHero): GuiHomeFrame {
	return { loading: false, rows: [], selected: SELECTED, hero: facts };
}

/** Render the Hero for the carried content. */
function mountHero(facts: GuiHomeHero = hero()): HTMLElement {
	return render(LibraryHomeHero, { home: home(facts) }).container;
}

test('the Hero paints the title, the meta rows, and the overview, and no Workspace', () => {
	const container = mountHero();

	expect(container.querySelector('[data-library-home-hero-title]')?.textContent).toBe(
		'Better Call Saul',
	);
	const meta = [...container.querySelectorAll('[data-library-home-hero-role]')];
	expect(meta.map((row) => row.textContent)).toEqual(['1 h 47 m left', 'S3E4 · Sabrosito']);
	expect(meta[0]?.getAttribute('data-library-home-hero-role')).toBe('duration');
	expect(container.querySelector('[data-library-home-hero-overview]')?.textContent).toBe(
		'Jimmy turns to paying work.',
	);
	expect(container.querySelector('[data-library-tracklist]')).toBeNull();
});

// The artwork box reserves the arm the frame declares (design D15): a square or
// portrait cover keeps the Music Hero's 300 px box at the aspect's height, and a
// landscape still fills the Hero content width at 16:9, as the TUI's hero does.
test.each([
	['square', 300],
	['portrait', 450],
] as const)('a %s hero places its artwork box at 300 px by %s px', (shape, height) => {
	const container = mountHero(hero(shape));
	const box = container.querySelector('[data-library-home-hero-art]') as HTMLElement;

	expect(box.style.width).toBe('300px');
	expect(box.style.height).toBe(`${height}px`);
});

test('a landscape hero fills the content width at 16:9 instead of the 300 px box', () => {
	const container = mountHero(hero('landscape'));
	const box = container.querySelector('[data-library-home-hero-art]') as HTMLElement;

	expect(box.style.width).toBe('100%');
	expect(box.style.aspectRatio).toBe('16 / 9');
});

test('Play sends the play command for the selected row', async () => {
	const container = mountHero();

	await fireEvent.click(
		container.querySelector('[data-library-home-hero-play]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'homePlay', row: SELECTED },
	});
});

test('Play next sends the play-next command for the selected row', async () => {
	const container = mountHero();

	await fireEvent.click(
		container.querySelector('[data-library-home-hero-play-next]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'homePlayNext', row: SELECTED },
	});
});

test('Add to queue sends the enqueue command for the selected row', async () => {
	const container = mountHero();

	await fireEvent.click(
		container.querySelector('[data-library-home-hero-add]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'homeQueue', row: SELECTED },
	});
});
