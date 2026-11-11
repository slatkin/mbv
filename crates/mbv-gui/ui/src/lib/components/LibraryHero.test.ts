// add-gui-library-panel row 4.7: the Library Hero paints the canvas's detail
// pane (design D5, D8) — the artist header with Shuffle artist, the album block
// with its square art and its Play, Play next, Add to queue, and More actions
// controls, and the tracklist with its hover play-next and add-to-queue
// controls — and every control sends its command with the item's id. The item
// menu paints the entries the frame carries and a chosen entry sends its index.
// The Tauri IPC is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicFrame, GuiMusicHero } from '$lib/bridge/types';
import { initialLibraryUi, libraryUi } from '$lib/library/state';
import LibraryHero from './LibraryHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	libraryUi.set(initialLibraryUi());
});

/** The picked album's Hero facts: two union albums behind it, a cached
 * tracklist, and the item menu the core built for the album (design D8). */
function hero(): GuiMusicHero {
	return {
		artist: 'Beastie Boys',
		collections: ['Hip-Hop'],
		albumCount: 2,
		artistTracks: { count: 2, duration: '7:02' },
		album: {
			albumId: 'al1',
			name: 'Licensed To Ill',
			year: '1986',
			tracks: [
				{ trackId: 't1', name: 'Rhymin & Stealin', number: 1, duration: '4:08' },
				{ trackId: 't2', name: 'The New Style', number: 2, duration: '4:35' },
			],
			total: { count: 2, duration: '8:43' },
		},
		menu: [
			{ label: 'Play All', keyHint: 'Enter', index: 0 },
			{ label: 'Shuffle', keyHint: null, index: 1 },
			{ label: 'Add to Queue', keyHint: null, index: 2 },
		],
	};
}

/** The carried Music content: the artist that owns the picked album, whose
 * union albums Shuffle artist names. */
function music(facts: GuiMusicHero): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [{ groupId: 'g1', name: 'Hip-Hop', selected: true, collection: true }],
		collectionNames: ['Hip-Hop'],
		loading: false,
		failure: null,
		artists: [
			{
				artistKey: { kind: 'service', id: 'a1' },
				name: 'Beastie Boys',
				sortLetter: 'B',
				albums: [facts.album, { albumId: 'al2', name: 'Paul’s Boutique', year: '1989', tracks: null, total: null }],
			},
		],
		hero: facts,
		art: null,
		playingTrackId: null,
	};
}

/** Render the Hero for the carried content. */
function mountHero(facts: GuiMusicHero = hero()): HTMLElement {
	return render(LibraryHero, { music: music(facts) }).container;
}

/** The element a press targets, found by its data attribute. */
function part(container: HTMLElement, name: string): HTMLElement {
	return container.querySelector(`[data-library-hero-${name}]`) as HTMLElement;
}

test("Play sends the picked album's id", async () => {
	const container = mountHero();

	await fireEvent.click(part(container, 'play'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 'al1' },
	});
});

test("Play next sends the picked album's id", async () => {
	const container = mountHero();

	await fireEvent.click(part(container, 'play-next'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'playNext', itemId: 'al1' },
	});
});

test("Add to queue sends the picked album's id", async () => {
	const container = mountHero();

	await fireEvent.click(part(container, 'add'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'addToQueue', itemId: 'al1' },
	});
});

test('Shuffle artist sends the artist’s albums in the union', async () => {
	const container = mountHero();

	await fireEvent.click(part(container, 'shuffle'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'shuffleArtist', albumIds: ['al1', 'al2'] },
	});
});

test("a track's Play next sends that track's id", async () => {
	const container = mountHero();

	await fireEvent.click(
		container.querySelector('[data-library-track="t2"] [data-library-track-action="playNext"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'playNext', itemId: 't2' },
	});
});

test("a track's Add to queue sends that track's id", async () => {
	const container = mountHero();

	await fireEvent.click(
		container.querySelector('[data-library-track="t1"] [data-library-track-action="addToQueue"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'addToQueue', itemId: 't1' },
	});
});

test("More actions paints the item menu's entries from the frame", async () => {
	const container = mountHero();

	await fireEvent.click(part(container, 'more'));

	// The Hero's own buttons already send Play All and Add to Queue, so the
	// dropped-duplicates rule leaves Shuffle the only painted entry, under its
	// shared-builder index.
	const entries = [...(container.querySelectorAll('[data-row-menu] [data-menu-entry]') ?? [])];
	expect(entries.map((entry) => entry.textContent?.trim())).toEqual(['Shuffle']);
	expect(entries.map((entry) => entry.getAttribute('data-menu-entry'))).toEqual(['1']);
});

test('a menu entry sends its index for the picked item', async () => {
	const container = mountHero();
	await fireEvent.click(part(container, 'more'));

	await fireEvent.click(
		container.querySelector('[data-row-menu] [data-menu-entry="1"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'menuEntry', itemId: 'al1', index: 1 },
	});
});

// The frame carries no entries when every entry duplicates a Hero button (the
// dropped-duplicates rule): the menu would add nothing, so no More actions
// control paints at all.
test('an empty hero menu paints no More actions control', () => {
	const container = mountHero({ ...hero(), menu: [] });

	expect(part(container, 'more')).toBeNull();
});

// Review of row 4.7, finding P2-b: the cursor was seeded only when a pane took
// the focus, so a tracklist that landed afterwards painted none. The painted
// cursor derives from the tracks, so the first track carries it.
test('the tracklist paints the cursor on its first track before any key moves it', () => {
	const container = mountHero();

	expect(
		container.querySelector('[data-library-track="t1"]')?.hasAttribute('data-library-track-cursor'),
	).toBe(true);
	expect(
		container.querySelector('[data-library-track="t2"]')?.hasAttribute('data-library-track-cursor'),
	).toBe(false);
});

// A tracklist row plays from its play button, which replaces the track number
// on hover, and from a double-click on its name, as an episode and a chapter
// do (add-gui-library-panel invariant 30).
test('a double-click on a track name sends that track play', async () => {
	const container = mountHero();

	await fireEvent.dblClick(
		container.querySelector('[data-library-track="t2"] [data-library-track-name]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 't2' },
	});
});

test('the play button on a tracklist row sends that track play', async () => {
	const container = mountHero();

	await fireEvent.click(
		container.querySelector('[data-library-track="t2"] [data-library-track-action="play"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 't2' },
	});
});

// The Library's playing row (add-gui-library-panel): the frame carries the
// playhead's track id, and that row shows a play glyph beside its title. A
// highlight marks focus only, so the playing row keeps its own fill.
test('the tracklist marks the frame playing track with a play glyph, not a fill', () => {
	const container = mountHeroWithPlaying('t1');

	const playing = container.querySelector('[data-library-track="t1"]') as HTMLElement;
	expect(playing.hasAttribute('data-library-track-playing')).toBe(true);
	expect(playing.querySelector('svg')).toBeTruthy();
	expect(playing.style.background).not.toBe('var(--canvas-accent)');
	expect(
		container.querySelector('[data-library-track="t2"]')?.hasAttribute('data-library-track-playing'),
	).toBe(false);
});

/** The Hero mounted with `trackId` playing at the playhead. */
function mountHeroWithPlaying(trackId: string): HTMLElement {
	return render(LibraryHero, { music: { ...music(hero()), playingTrackId: trackId } }).container;
}
