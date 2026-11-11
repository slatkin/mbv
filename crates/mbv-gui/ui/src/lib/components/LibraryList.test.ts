// add-gui-library-panel row 4.5: the Library list paints the canvas tree (design
// D7) over native scroll and only the rows in view, opens an album whose
// tracklist is not loaded and asks the core for the tracks, and narrows the
// painted rows by the `/` filter. The Tauri IPC is mocked; the asserted
// payloads are the serde wire shapes of bridge/library.rs. The row geometry is
// asserted on the model in `$lib/library/list.test.ts`.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicArtist, GuiMusicFrame } from '$lib/bridge/types';
import { initialLibraryUi, libraryUi } from '$lib/library/state';
import LibraryList from './LibraryList.svelte';

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
	disconnect() {}
	fire() {
		this.#callback([], this as unknown as ResizeObserver);
	}
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	libraryUi.set(initialLibraryUi());
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** The Beastie Boys artist: one album with a cached tracklist, one without. */
const BEASTIE: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a1' },
	name: 'Beastie Boys',
	sortLetter: 'B',
	albums: [
		{
			albumId: 'al1',
			name: 'Licensed To Ill',
			year: '1986',
			tracks: [{ trackId: 't1', name: 'Rhymin & Stealin', number: 1, duration: '3:27' }],
			total: { count: 1, duration: '3:27' },
		},
		{ albumId: 'al2', name: 'Paul’s Boutique', year: '1989', tracks: null, total: null },
	],
};

/** The Brand Nubian artist: one album, closed under the artist. */
const BRAND: GuiMusicArtist = {
	artistKey: { kind: 'service', id: 'a2' },
	name: 'Brand Nubian',
	sortLetter: 'B',
	albums: [{ albumId: 'al3', name: 'One For All', year: '1990', tracks: null, total: null }],
};

/** The carried Music content: the two artists, and the collection names the
 * header reads (design D6). */
function music(collectionNames: string[] = ['Hip-Hop']): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [{ groupId: 'g1', name: 'Hip-Hop', selected: true, collection: true }],
		collectionNames,
		loading: false,
		failure: null,
		artists: [BEASTIE, BRAND],
		hero: null,
		art: null,
		playingTrackId: null,
	};
}

/** Render the list and report a `viewportHeight` px viewport, so the rows in
 * view are the rows a measured list of that height paints. */
async function mountList(viewportHeight = 40): Promise<HTMLElement> {
	const { container } = render(LibraryList, { music: music() });
	const list = container.querySelector<HTMLElement>('[data-library-list]');
	Object.defineProperty(list, 'clientHeight', { value: viewportHeight, configurable: true });
	ResizeObserverStub.instances[0]?.fire();
	await tick();
	return container;
}

/** Open the Beastie Boys node, so its albums paint. */
function openBeastie(): void {
	libraryUi.update((ui) => ({ ...ui, openNodes: ['artist:service:a1'] }));
}

/** Open the Licensed To Ill album node, so its cached tracks paint. */
function openLicensedToIll(): void {
	libraryUi.update((ui) => ({ ...ui, openNodes: [...ui.openNodes, 'album:al1'] }));
}

test('a press on an album without its tracklist opens it and asks for the tracks', async () => {
	openBeastie();
	const container = await mountList(240);
	const row = container.querySelector('[data-library-row-id="album:al2"]');

	await fireEvent.click(row as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openAlbum', albumId: 'al2' },
	});
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showDetail', itemId: 'al2' },
	});
	expect(
		container.querySelector('[data-library-row-id="album:al2"]')?.getAttribute('aria-expanded'),
	).toBe('true');
});

test('the header names the carried selection, not a derivation from the pills', () => {
	const { container } = render(LibraryList, { music: music(['Hip-Hop', 'Soul']) });

	expect(container.querySelector('[data-library-selection-label]')?.textContent).toBe(
		'Hip-Hop + Soul',
	);
});

// An empty pill selection is the whole library: the header names it `All`
// instead of printing no collection name (the Library's empty-selection
// label).
test('an empty selection names the header All', () => {
	const { container } = render(LibraryList, { music: music([]) });

	expect(container.querySelector('[data-library-selection-label]')?.textContent).toBe('All');
});

// A right-click on a row asks the core for that row's full item menu through
// the row-menu command, naming the row's stable Service id (the artist's item
// id, an album's own id, a track's id).
test("a right-press on a row requests the item's menu for its id", async () => {
	openBeastie();
	openLicensedToIll();
	const container = await mountList(240);
	const row = container.querySelector('[data-library-row-id="album:al1"]');

	await fireEvent.contextMenu(row as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'rowMenu', itemId: 'al1' },
	});
});

test('the list paints only the rows in view', async () => {
	const container = await mountList();

	expect(container.querySelector('[data-library-row-id="artist:service:a1"]')).toBeTruthy();
	expect(container.querySelector('[data-library-row-id="artist:service:a2"]')).toBeNull();
});

test('the filter narrows the painted rows to the names that contain its text', async () => {
	const container = await mountList(240);
	const field = container.querySelector<HTMLInputElement>('[data-library-filter]');

	await fireEvent.input(field as HTMLElement, { target: { value: 'nubian' } });
	await tick();

	expect(container.querySelector('[data-library-row-id="artist:service:a1"]')).toBeNull();
	expect(container.querySelector('[data-library-row-id="artist:service:a2"]')).toBeTruthy();
});

// Review of row 4.5, finding P2: the filter opens a node that holds a match,
// so a press under the filter must still close it — the close half of the
// design's "a press on an artist or album row opens or closes it".
test('a press closes and reopens a node while the filter narrows the rows', async () => {
	libraryUi.update((ui) => ({ ...ui, filter: 'rhymin' }));
	const container = await mountList(240);
	const album = () =>
		container.querySelector('[data-library-row-id="album:al1"]');

	expect(album()?.getAttribute('aria-expanded')).toBe('true');
	await fireEvent.click(album() as HTMLElement);
	expect(album()?.getAttribute('aria-expanded')).toBe('false');
	await fireEvent.click(album() as HTMLElement);
	expect(album()?.getAttribute('aria-expanded')).toBe('true');
});

// The user's double-click contract (add-gui-library-panel): a double-click on
// a track row plays that track, through the same play helper the Hero's
// tracklist rows use.
test('a double-click on a track row sends that track play', async () => {
	openBeastie();
	openLicensedToIll();
	const container = await mountList(240);
	const track = container.querySelector('[data-library-row-id="track:t1"]');

	await fireEvent.dblClick(track as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'play', itemId: 't1' },
	});
});

// The user's live finding: a track press had no observable effect, because its
// only effect — picking its album — was already the state once the album node
// was open. The press now also moves the cursor onto the row, as the Queue
// panel's click moves the queue cursor.
test('a press on a track row moves the cursor onto it', async () => {
	openBeastie();
	openLicensedToIll();
	const container = await mountList(240);
	const track = container.querySelector('[data-library-row-id="track:t1"]');

	await fireEvent.click(track as HTMLElement);

	expect(
		container
			.querySelector('[data-library-row-id="track:t1"]')
			?.hasAttribute('data-library-cursor'),
	).toBe(true);
});
