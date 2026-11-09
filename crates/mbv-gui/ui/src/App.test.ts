// add-gui-library-panel row 4.2: the window arranges the Queue panel and the
// Library panel from the measured Library column width (design D4) — Mini
// below 480 px showing only the panel with Panel focus, Narrow from 480 px
// with both panels, and Wide from 880 px when the container is at least
// 600 px tall. The Tauri event and IPC APIs are mocked; the frame is the
// serde wire shape of bridge/frame.rs.

import { listen } from '@tauri-apps/api/event';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { frames } from '$lib/bridge/frames';
import { DEFAULT_DRAGGED_WIDTHS, draggedWidths } from '$lib/bridge/resize';
import type {
	GuiFrame,
	GuiLibraryContent,
	GuiMusicFrame,
	GuiMusicHero,
	GuiPanel,
	GuiWindowFrame,
	LayoutSize,
} from '$lib/bridge/types';
import { libraryContent } from '$lib/library/state';
import App from './App.svelte';

const listenMock = vi.mocked(listen);

/** A ResizeObserver stub: jsdom has none and no layout. The test fires the
 * observer after render, so the measured size lands outside the effect that
 * observes the container. */
class ResizeObserverStub {
	static instances: ResizeObserverStub[] = [];
	static size: LayoutSize = { width: 0, height: 0 };
	readonly #callback: ResizeObserverCallback;
	constructor(callback: ResizeObserverCallback) {
		this.#callback = callback;
		ResizeObserverStub.instances.push(this);
	}
	observe() {}
	disconnect() {}
	fire() {
		this.#callback(
			[{ contentRect: ResizeObserverStub.size }] as unknown as ResizeObserverEntry[],
			this as unknown as ResizeObserver,
		);
	}
}

beforeEach(() => {
	listenMock.mockReset();
	listenMock.mockResolvedValue(() => {});
	libraryContent.set(null);
	// The divider widths are session state (design D4): a drag one test holds
	// would outlive it.
	draggedWidths.set(DEFAULT_DRAGGED_WIDTHS);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** The Music content with an optional picked album for the Hero. */
function music(hero: GuiMusicHero | null = null): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [],
		collectionNames: [],
		loading: false,
		failure: null,
		artists: [],
		hero,
		art: null,
		playingTrackId: null,
		rowMenu: null,
	};
}

/** A picked album's Hero facts. */
function hero(): GuiMusicHero {
	return {
		artist: 'Beastie Boys',
		collections: ['Hip-Hop'],
		albumCount: 1,
		artistTracks: null,
		album: {
			albumId: 'album-1',
			name: 'Licensed to Ill',
			year: '1986',
			tracks: null,
			total: null,
		},
		menu: [],
	};
}

/** The attached slice's frame: an empty queue, the given Panel focus, and the
 * given carried Library content. */
function frame(panel: GuiPanel, libraryMusic: GuiMusicFrame | null): GuiFrame {
	return {
		focus: { surface: 'queue', panel },
		queue: { rows: [], cursor: null, marks: [], active: null, hover: null, drag: null },
		summary: {
			remaining: 0,
			nextTitle: null,
			finishClock: null,
			rightColumn: 'length',
			filter: '',
			filterActive: false,
		},
		panel: { collapsed: false, queueFolded: false },
		overlay: null,
		playback: {
			status: 'idle',
			host: '',
			hostIsRemote: false,
			shown: false,
			playback: {
				active: false,
				activeIdx: null,
				positionTicks: 0,
				runtimeTicks: 0,
				paused: false,
			},
			elapsedClock: '0:00',
			remainingClock: '0:00',
			showControls: false,
			nowPlayingTitle: null,
			parentLine: null,
			episodeBadge: null,
			stillCaption: null,
			finishClock: null,
			availability: { stop: false, next: false, previous: false },
			statusIndicators: null,
			artwork: null,
			chapters: [],
			introMark: null,
			creditsMark: null,
			trickplay: null,
			trackChips: null,
			targetChoices: [],
			hover: null,
		},
		footer: {
			owner: { text: '', role: 'textPrimary', bold: false },
			ownerLive: false,
			playlist: { text: '', role: 'textPrimary', bold: false },
			autosave: null,
			dirty: null,
		},
		status: { prefixArmed: false, toast: null },
		library:
			libraryMusic === null
				? null
				: {
						revision: 1,
						content: {
							tabs: [{ position: 1, label: 'Music' }],
							currentTab: 1,
							music: libraryMusic,
							home: null,
							shows: null,
							emby: null,
							audiobooks: null,
							podcasts: null,
						},
					},
		theme: null,
	};
}

/** Seeds the frame store and the Library content the window holds, renders the
 * window at `size`, and returns the rendered view once the observer's
 * measurement has painted. */
async function mount(
	size: LayoutSize,
	slice: GuiFrame,
	carried: GuiLibraryContent | null = null,
) {
	await frames.listen();
	const handler = listenMock.mock.calls.at(-1)?.[1] as (
		event: { payload: GuiWindowFrame },
	) => void;
	handler({ payload: { slice } });
	// The frame store keeps the seeded slice; a stale carried value from
	// another test would outlive the reset, so set the store rather than the
	// frame.
	libraryContent.set(carried);
	ResizeObserverStub.size = size;
	const view = render(App);
	await tick();
	// The window and the Library list each measure through their own observer,
	// so every one fires: the window takes the container's size and the list
	// takes its own measured height.
	for (const instance of ResizeObserverStub.instances) {
		instance.fire();
	}
	await tick();
	return view;
}

test('prevents the default context menu', () => {
	render(App);
	const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });

	document.body.dispatchEvent(event);

	expect(event.defaultPrevented).toBe(true);
});

test('without Music content the Queue panel paints alone', async () => {
	const { container } = await mount({ width: 1600, height: 900 }, frame('queue', null));

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'wide',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	expect(container.querySelector('[data-library-panel]')).toBeNull();
});

// Row 4.8's negative case: the Library Status bar paints the frame's armed
// prefix and live toast only while the Library panel shows, so the Queue
// footer keeps painting both while it does not. The frame's `status` is one
// fact carried to both surfaces, never consumed by either.
test('the Queue footer keeps the prefix and the toast while the Library panel does not show', async () => {
	const slice = frame('queue', null);
	slice.status = { prefixArmed: true, toast: null };
	const { container } = await mount({ width: 1600, height: 900 }, slice);

	expect(container.querySelector('[data-library-panel]')).toBeNull();
	expect(container.querySelector('[data-footer-line]')?.textContent).toContain('PREFIX');

	// The next frame carries a toast: the footer paints it in the same line.
	const handler = listenMock.mock.calls.at(-1)?.[1] as (
		event: { payload: GuiWindowFrame },
	) => void;
	handler({
		payload: {
			slice: {
				...slice,
				status: { prefixArmed: false, toast: { text: 'Scanning', severity: 'neutral' } },
			},
		},
	});
	await tick();

	expect(container.querySelector('[data-library-panel]')).toBeNull();
	expect(container.querySelector('[data-footer-line]')?.textContent).toContain('Scanning');
});

// Row 4.8's positive case (design D10): while the Library panel shows, its
// Library Status bar paints the frame's armed prefix and live toast, and the
// Queue footer line paints neither — each fact paints once a frame.
test('the Queue footer drops the prefix and the toast while the Library panel shows', async () => {
	const slice = frame('queue', music());
	slice.status = { prefixArmed: true, toast: null };
	const { container } = await mount({ width: 1600, height: 900 }, slice);

	expect(container.querySelector('[data-library-panel]')).toBeTruthy();
	expect(container.querySelector('[data-library-status-prefix]')?.textContent).toContain('PREFIX');
	expect(container.querySelector('[data-footer-line]')?.textContent).not.toContain('PREFIX');

	// The next frame carries a toast: the Library Status bar paints it, and the
	// footer line stays clear of it.
	const handler = listenMock.mock.calls.at(-1)?.[1] as (
		event: { payload: GuiWindowFrame },
	) => void;
	handler({
		payload: {
			slice: {
				...slice,
				status: { prefixArmed: false, toast: { text: 'Scanning', severity: 'neutral' } },
			},
		},
	});
	await tick();

	expect(container.querySelector('[data-library-status-toast]')?.textContent).toContain('Scanning');
	expect(container.querySelector('[data-footer-line]')?.textContent).not.toContain('Scanning');
});

// The Library panel's one presence fact (design D2, D10): a frame that carries
// no Library section paints no panel and hides no Queue panel, whatever content
// the window kept from an earlier frame.
test('a frame without the Library section paints only the Queue panel', async () => {
	const { container } = await mount(
		{ width: 700, height: 900 },
		frame('library', null),
		{ tabs: [{ position: 1, label: 'Music' }], currentTab: 1, music: music(), home: null, shows: null, emby: null, audiobooks: null, podcasts: null },
	);

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'mini',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	expect(container.querySelector('[data-library-panel]')).toBeNull();
});

test('Mini below 480 px mounts only the Queue panel while Panel focus is queue', async () => {
	const { container } = await mount({ width: 700, height: 900 }, frame('queue', music()));

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'mini',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	expect(container.querySelector('[data-library-panel]')).toBeNull();
});

test('Mini mounts only the Library panel while Panel focus is library', async () => {
	const { container } = await mount({ width: 700, height: 900 }, frame('library', music()));

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'mini',
	);
	expect(container.querySelector('[data-panel-state]')).toBeNull();
	expect(
		container.querySelector('[data-library-panel]')?.getAttribute('data-library-arrangement'),
	).toBe('mini');
});

test('Narrow mounts both panels without a Hero pane', async () => {
	const { container } = await mount({ width: 1200, height: 900 }, frame('library', music()));

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'narrow',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	const panel = container.querySelector('[data-library-panel]');
	expect(panel?.getAttribute('data-library-arrangement')).toBe('narrow');
	expect(panel?.querySelector('[data-library-region="list"]')).toBeTruthy();
	expect(container.querySelector('[data-library-region="hero"]')).toBeNull();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

test('Narrow opens the Hero overlay once an album is picked', async () => {
	const { container } = await mount(
		{ width: 1200, height: 900 },
		frame('library', music(hero())),
	);

	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeTruthy();
});

test('Wide mounts the Hero pane beside the list', async () => {
	const { container } = await mount(
		{ width: 1600, height: 900 },
		frame('library', music(hero())),
	);

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'wide',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	const panel = container.querySelector('[data-library-panel]');
	expect(panel?.getAttribute('data-library-arrangement')).toBe('wide');
	expect(panel?.querySelector('[data-library-region="hero"]')).toBeTruthy();
	expect(panel?.querySelector('[data-library-region="list"]')).toBeTruthy();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

test('880 px wide but short of 600 px falls back to Narrow', async () => {
	const { container } = await mount({ width: 1280, height: 500 }, frame('library', music()));

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'narrow',
	);
	expect(container.querySelector('[data-library-region="hero"]')).toBeNull();
});

test('the collapsed rail leaves a wider Library column', async () => {
	const slice = frame('library', music());
	slice.panel = { collapsed: true, queueFolded: false };

	const { container } = await mount({ width: 1000, height: 900 }, slice);

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'wide',
	);
});

// Design D4's dividers: a drag reports the sized column's new width, the Queue
// column paints it, and the arrangement classifies that same width; a
// double-click returns the column to its default. jsdom has no PointerEvent, so
// the drag's coordinates ride a MouseEvent of the same type.
test('dragging the Queue divider resizes the column and a double-click resets it', async () => {
	const { container } = await mount(
		{ width: 1600, height: 900 },
		frame('library', music(hero())),
	);
	const column = container.querySelector('[data-queue-column]') as HTMLElement;
	const divider = container.querySelector('[data-divider="queue"]') as HTMLElement;

	expect(column.style.width).toBe('400px');

	fireEvent(divider, new MouseEvent('pointerdown', { clientX: 400, button: 0, bubbles: true }));
	fireEvent(window, new MouseEvent('pointermove', { clientX: 500 }));
	await tick();

	expect(column.style.width).toBe('500px');
	// The Library column narrowed by the drag: 1600 - 500 is still Wide.
	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'wide',
	);

	fireEvent(divider, new MouseEvent('dblclick', { bubbles: true }));
	await tick();

	expect(column.style.width).toBe('400px');
});

// add-gui-library-panel row 8.1: while a plain Emby library is the active
// destination (design D12), the window mounts the Library panel with the Emby
// painters — its list, its rail, and its Hero — as it does for the other
// destinations.
test('the Emby content mounts the Library panel with the Emby painters', async () => {
	const slice = frame('library', null);
	slice.library = { revision: 1, content: embyContent() };
	const { container } = await mount({ width: 1600, height: 900 }, slice);

	const panel = container.querySelector('[data-library-panel]');

	expect(panel).toBeTruthy();
	expect(panel?.querySelector('[data-library-emby-list]')).toBeTruthy();
	expect(panel?.querySelector('[data-library-region="rail"]')).toBeTruthy();
	expect(
		panel?.querySelector('[data-library-region="hero"] [data-library-emby-hero]'),
	).toBeTruthy();
	expect(panel?.querySelector('[data-library-list]')).toBeNull();
});

// add-gui-library-panel row 10.1: while a Podcasts library is the active
// destination (design D12), the window mounts the Library panel with the
// Podcasts painters — its pills, its date-gutter list, and its Hero — and
// none of the other destinations' painters.
test('the Podcasts content mounts the Library panel with the Podcasts painters', async () => {
	const slice = frame('library', null);
	slice.library = { revision: 1, content: podcastsContent() };
	const { container } = await mount({ width: 1600, height: 900 }, slice);

	const panel = container.querySelector('[data-library-panel]');

	expect(panel).toBeTruthy();
	expect(panel?.querySelector('[data-library-podcasts-list]')).toBeTruthy();
	expect(
		panel?.querySelector('[data-library-region="hero"] [data-library-podcasts-hero]'),
	).toBeTruthy();
	expect(panel?.querySelector('[data-library-pill="latest"]')).toBeTruthy();
	expect(panel?.querySelector('[data-library-list]')).toBeNull();
	expect(panel?.querySelector('[data-library-emby-list]')).toBeNull();
	expect(panel?.querySelector('[data-library-audiobooks-list]')).toBeNull();
});

/** The carried content while a Podcasts library is active: the Latest pill
 * and a show pill, one merged episode row, and the selected episode's Hero
 * (design D12, row 10.1). */
function podcastsContent(): GuiLibraryContent {
	return {
		tabs: [{ position: 2, label: 'Podcasts' }],
		currentTab: 2,
		music: null,
		home: null,
		shows: null,
		emby: null,
		audiobooks: null,
		podcasts: {
			libIdx: 2,
			pills: [
				{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false },
				{ pill: { kind: 'show', id: 'sh1' }, name: 'Show One', selected: false, collection: false },
			],
			loading: false,
			failed: false,
			rows: [
				{
					id: 'e1:ep1',
					title: 'Episode A',
					subtitle: 'Show One',
					date: '12 May',
					state: 'ordinary',
				},
			],
			selected: 'e1:ep1',
			hero: {
				title: 'Episode A',
				metaRows: ['Show One', '34:00'],
				durationRow: 1,
				progressRow: null,
				overview: null,
				artShape: 'square',
				art: null,
			},
		},
	};
}

/** The carried content while a plain Emby library is active: one name-sorted
 * row with its rail letter and its Hero (design D12, row 8.1). */
function embyContent(): GuiLibraryContent {
	return {
		tabs: [{ position: 3, label: 'Movies' }],
		currentTab: 3,
		music: null,
		home: null,
		shows: null,
		emby: {
			libIdx: 3,
			pills: [],
			collectionNames: [],
			feedGroup: false,
			loading: false,
			failed: false,
			rows: [
				{
					id: 'm1',
					title: 'Alien',
					subtitle: '1979',
					date: null,
					sortLetter: 'A',
					kind: 'media',
				},
			],
			selected: 'm1',
			hero: {
				title: 'Alien',
				metaRows: ['1979'],
				durationRow: 0,
				progressRow: null,
				overview: null,
				artShape: 'landscape',
				art: null,
			},
		},
		audiobooks: null,
		podcasts: null,
	};
}
