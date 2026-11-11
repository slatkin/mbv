// add-gui-library-panel row 10.1: the Podcasts Hero (design D12) paints the
// selected episode's facts — the title, the meta rows with their duration and
// progress roles, and the description as the overview — and places the artwork
// box per the frame's `artShape`. Play and Add to queue send the Podcasts
// commands with the frame's selected episode, and no play-next control paints:
// `n` does nothing for podcast episodes. The Tauri IPC is mocked; the asserted
// payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiPodcastsFrame, GuiPodcastsHero } from '$lib/bridge/types';
import LibraryPodcastsHero from './LibraryPodcastsHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** The library index the Podcasts commands name. */
const LIB_IDX = 4;

/** The selected episode's Hero facts: the show as the parent, two meta rows — the
 * duration and the publish date — with the duration role marked, and the
 * description (design D12, row 10.1). */
function hero(artShape: GuiPodcastsHero['artShape'] = 'square'): GuiPodcastsHero {
	return {
		title: 'Episode 12: The Long Road',
		parent: 'Bits and Bites',
		metaRows: ['0 h 54 m', '08 Oct'],
		durationRow: 0,
		progressRow: null,
		overview: 'A conversation about long walks.',
		artShape,
		art: null,
	};
}

/** The carried Podcasts content with the hero picked. */
function podcasts(facts: GuiPodcastsHero | null): GuiPodcastsFrame {
	return {
		libIdx: LIB_IDX,
		loading: false,
		failed: false,
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false },
		],
		rows: [
			{
				id: 'show1:ep12',
				title: 'Episode 12: The Long Road',
				subtitle: 'Bits and Bites',
				date: '08 Oct',
				state: 'ordinary',
			},
		],
		selected: 'show1:ep12',
		hero: facts,
	};
}

/** Render the Hero for the carried content. */
function mount(facts: GuiPodcastsHero | null = hero()): HTMLElement {
	return render(LibraryPodcastsHero, { podcasts: podcasts(facts) }).container;
}

// The artwork box takes the arm every Hero shares (design D15): the square
// keeps the Music Hero's 300 px side, the portrait the tall box, and the
// landscape fills the Hero content width at 16:9.
test.each([
	['landscape', '100%', 'auto'],
	['square', '300px', '300px'],
	['portrait', '300px', '450px'],
] as const)('a %s Hero places its artwork box at the shared arm', (shape, width, height) => {
	const container = mount(hero(shape));
	const box = container.querySelector('[data-library-podcasts-hero-art]') as HTMLElement;

	expect(box.style.width).toBe(width);
	expect(box.style.height).toBe(height);
});

test('the Hero paints the show as its title, the episode in the block, the duration beside the art, and the overview', () => {
	const container = mount();

	expect(container.querySelector('[data-library-podcasts-hero-title]')?.textContent).toBe(
		'Bits and Bites',
	);
	expect(container.querySelector('[data-library-podcasts-hero-facts]')?.textContent).toBe(
		'08 Oct',
	);
	expect(container.querySelector('[data-library-podcasts-hero-block-name]')?.textContent).toBe(
		'Episode 12: The Long Road',
	);
	const meta = [...container.querySelectorAll('[data-library-podcasts-hero-role]')];
	expect(meta.map((row) => row.textContent)).toEqual(['0 h 54 m']);
	expect(meta.map((row) => row.getAttribute('data-library-podcasts-hero-role'))).toEqual([
		'duration',
	]);
	expect(container.querySelector('[data-library-podcasts-hero-overview]')?.textContent).toBe(
		'A conversation about long walks.',
	);
});

test('a Hero that paints none paints nothing', () => {
	const container = mount(null);

	expect(container.querySelector('[data-library-podcasts-hero]')).toBeNull();
});

// A podcast Hero is episode information only: no Workspace and no tracklist
// paints (design D12, row 10.1).
test('the Hero paints no Workspace and no tracklist', () => {
	const container = mount();

	expect(container.querySelector('[data-library-shows-workspace]')).toBeNull();
	expect(container.querySelector('[data-library-tracklist]')).toBeNull();
});

test('Play sends the play command for the selected episode', async () => {
	const container = mount();

	await fireEvent.click(
		container.querySelector('[data-library-podcasts-hero-play]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsPlay', libIdx: LIB_IDX, itemId: 'show1:ep12' },
	});
});

test('Add to queue sends the enqueue command for the selected episode', async () => {
	const container = mount();

	await fireEvent.click(
		container.querySelector('[data-library-podcasts-hero-add]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsQueue', libIdx: LIB_IDX, itemId: 'show1:ep12' },
	});
});

// There is no play-next command for podcast episodes (design D12, row 10.1):
// `n` is consumed by the front end and does nothing, so no control paints it.
test('the Hero paints no play-next control', () => {
	const container = mount();

	expect(container.querySelector('[data-library-podcasts-hero-play-next]')).toBeNull();
	expect(
		container.querySelector('[data-library-podcasts-hero-block]')?.textContent,
	).not.toContain('Play next');
});

// The Hero paints the frame's header band, as Music and Shows do: the kicker
// over the title, above the artwork (the frame is one look for every Hero).
test('the Hero paints the frame header above its artwork', () => {
	const container = mount();

	expect(container.querySelector('[data-library-podcasts-hero-kicker]')?.textContent).toBe('podcast');
	expect(container.querySelector('[data-library-podcasts-hero-title]')?.tagName).toBe('H1');
});
