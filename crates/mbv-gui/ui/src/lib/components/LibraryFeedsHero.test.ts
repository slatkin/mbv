// add-gui-library-panel row 11.1: the Feeds Hero (design D12) paints the
// selected entry's facts — the title and the meta rows with their duration
// role — and places its placeholder artwork box per the frame's `artShape`.
// Play and Add to queue send the Feeds commands with the frame's selected
// entry, and no play-next control and no overview paragraph paints. The
// Tauri IPC is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiFeedsFrame, GuiFeedsHero } from '$lib/bridge/types';
import LibraryFeedsHero from './LibraryFeedsHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** A feed entry has no artwork source, so the Hero always paints the
 * placeholder; only the arm's box shape differs. */
function hero(artShape: GuiFeedsHero['artShape']): GuiFeedsHero {
	return {
		title: 'The Long Road Home',
		parent: 'Bits and Bites',
		metaRows: ['0 h 54 m'],
		durationRow: 0,
		progressRow: null,
		artShape,
	};
}

/** The carried Feeds content with the hero picked. */
function feeds(facts: GuiFeedsHero | null): GuiFeedsFrame {
	return {
		loading: false,
		failed: false,
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false },
		],
		rows: [
			{
				id: 'sub1:guid12',
				title: 'The Long Road Home',
				subtitle: 'Bits and Bites',
				date: '08 Oct',
				state: 'ordinary',
			},
		],
		selected: 'sub1:guid12',
		hero: facts,
	};
}

/** Render the Hero for the carried content. */
function mount(facts: GuiFeedsHero | null = hero('square')): HTMLElement {
	return render(LibraryFeedsHero, { feeds: feeds(facts) }).container;
}

// A feed entry has no artwork source, so the box paints the placeholder in
// every arm the shared artwork component reserves (design D15, row 11.1): the
// square keeps the 300 px side, and the landscape fills the Hero content width
// at 16:9.
test.each([
	['square', '300px', '300px'],
	['landscape', '100%', 'auto'],
] as const)('a %s Hero places its placeholder artwork box at the shared arm', (shape, width, height) => {
	const container = mount(hero(shape));
	const box = container.querySelector('[data-library-feeds-hero-art]') as HTMLElement;

	expect(box.style.width).toBe(width);
	expect(box.style.height).toBe(height);
	expect(box.textContent).toContain("the window's placeholder");
});

test('the Hero paints the feed as its title, the entry in the block, and the duration beside the art', () => {
	const container = mount();

	expect(container.querySelector('[data-library-feeds-hero-title]')?.textContent).toBe(
		'Bits and Bites',
	);
	expect(container.querySelector('[data-library-feeds-hero-block-name]')?.textContent).toBe(
		'The Long Road Home',
	);
	const meta = [...container.querySelectorAll('[data-library-feeds-hero-role]')];
	expect(meta.map((row) => row.textContent)).toEqual(['0 h 54 m']);
	expect(meta.map((row) => row.getAttribute('data-library-feeds-hero-role'))).toEqual([
		'duration',
	]);
});

// A feed entry carries no overview, so no overview paragraph paints (design
// D12, row 11.1).
test('the Hero paints no overview paragraph', () => {
	const container = mount();

	expect(container.querySelector('[data-library-feeds-hero-overview]')).toBeNull();
});

test('a Hero that paints none paints nothing', () => {
	const container = mount(null);

	expect(container.querySelector('[data-library-feeds-hero]')).toBeNull();
});

// A Feeds Hero is entry information only: no Workspace and no tracklist
// paints (design D12, row 11.1).
test('the Hero paints no Workspace and no tracklist', () => {
	const container = mount();

	expect(container.querySelector('[data-library-shows-workspace]')).toBeNull();
	expect(container.querySelector('[data-library-tracklist]')).toBeNull();
});

// Play is the selected entry, named by the row id the frame painted (design
// D2, row 11.1); a Client holds one feed tab, so no library index rides it.
test('Play sends the play command for the selected entry', async () => {
	const container = mount();

	await fireEvent.click(container.querySelector('[data-library-feeds-hero-play]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsPlay', itemId: 'sub1:guid12' },
	});
});

test('Add to queue sends the enqueue command for the selected entry', async () => {
	const container = mount();

	await fireEvent.click(container.querySelector('[data-library-feeds-hero-add]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsQueue', itemId: 'sub1:guid12' },
	});
});

// There is no play-next command for Feeds (row 11.1), so no control paints
// it.
test('the Hero paints no play-next control', () => {
	const container = mount();

	expect(container.querySelector('[data-library-feeds-hero-play-next]')).toBeNull();
	expect(
		container.querySelector('[data-library-feeds-hero-block]')?.textContent,
	).not.toContain('Play next');
});

// The Hero paints the frame's header band, as Music and Shows do: the kicker
// over the title, above the artwork (the frame is one look for every Hero).
test('the Hero paints the frame header above its artwork', () => {
	const container = mount();

	expect(container.querySelector('[data-library-feeds-hero-kicker]')?.textContent).toBe('feed');
	expect(container.querySelector('[data-library-feeds-hero-title]')?.tagName).toBe('H1');
});

// A feed is a collection (invariant 30): the list band holds the selected
// feed's entries, with the selected one carrying the cursor, and no other
// feed's entry.
test('the list band holds the selected feed entries and marks the selected one', () => {
	const frame = feeds(hero('square'));
	frame.rows = [
		{ ...frame.rows[0] },
		{ ...frame.rows[0], id: 'sub1:guid11', title: 'Earlier Walk' },
		{ ...frame.rows[0], id: 'sub2:guid20', title: 'Other Feed', subtitle: 'Elsewhere' },
	];
	const { container } = render(LibraryFeedsHero, { feeds: frame });

	const rows = [...container.querySelectorAll('[data-library-feeds-entry]')];
	expect(rows.map((row) => row.getAttribute('data-library-feeds-entry'))).toEqual([
		'sub1:guid12',
		'sub1:guid11',
	]);
	expect(rows[0].hasAttribute('data-library-feeds-entry-cursor')).toBe(true);
	expect(rows[1].hasAttribute('data-library-feeds-entry-cursor')).toBe(false);
});
