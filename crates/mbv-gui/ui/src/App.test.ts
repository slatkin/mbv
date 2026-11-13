// add-gui-library-panel row 4.2: the window arranges the Queue panel and the
// Library panel from the measured Library column width (design D4) — Mini
// below 480 px showing only the panel with Panel focus, Narrow from 480 px
// with both panels, and Wide from 880 px when the container is at least
// 600 px tall. The Tauri event and IPC APIs are mocked; the frame is the
// serde wire shape of bridge/frame.rs.

import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import {
	connected,
	focus,
	panel as panelStore,
	resetSectionsForTest,
	settings,
	status as statusStore,
} from '$lib/bridge/sections';
import { DEFAULT_DRAGGED_WIDTHS, draggedWidths } from '$lib/bridge/resize';
import { carryLibrary } from '$lib/bridge/test-library';
import type { LibrarySections } from '$lib/bridge/test-library';
import type {
	GuiSettingsFrame,
	GuiMusicHero,
	GuiPanel,
	LayoutSize,
} from '$lib/bridge/types';
import { initialSettingsUi, settingsUi } from '$lib/settings/state';
import { selectSettingsSection } from '$lib/settings/state';
import App from './App.svelte';

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
	resetSectionsForTest();
	settingsUi.set(initialSettingsUi());
	// The divider widths are session state (design D4): a drag one test holds
	// would outlive it.
	draggedWidths.set(DEFAULT_DRAGGED_WIDTHS);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

/** The Music content with an optional picked album for the Hero. */
function music(hero: GuiMusicHero | null = null): LibrarySections {
	return {
		list: {
			tabs: [{ position: 1, label: 'Music' }],
			currentTab: 1,
			destination: {
				kind: 'music',
				libIdx: 0,
				pills: [],
				collectionNames: [],
				loading: false,
				failure: null,
				artists: [],
			},
		},
		detail: { kind: 'music', hero, art: null, playingTrackId: null },
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

/** Seeds the window's section stores with an empty queue, the given Panel
 * focus, the given carried Music content, and any section overrides. */
function seed(
	panel: GuiPanel,
	libraryMusic: LibrarySections | null,
	overrides: {
		panel?: Partial<{ collapsed: boolean; queueFolded: boolean; uiResetEpoch: number }>;
		status?: { prefixArmed: boolean; toast: { text: string; severity: 'neutral' | 'success' | 'warning' | 'error' } | null };
		settings?: GuiSettingsFrame | null;
		library?: LibrarySections | null;
	} = {},
): void {
	connected.set(true);
	focus.set({ surface: panel, panel });
	panelStore.set({
		collapsed: false,
		queueFolded: false,
		uiResetEpoch: 0,
		...overrides.panel,
	});
	statusStore.set(overrides.status ?? { prefixArmed: false, toast: null });
	settings.set(overrides.settings ?? null);
	carryLibrary(overrides.library ?? libraryMusic);
}

/** Renders the window over the seeded section stores at `size`, and returns
 * the rendered view once the observer's measurement has painted. */
async function mount(size: LayoutSize) {
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
	seed('queue', null);
	const { container } = await mount({ width: 1600, height: 900 });

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
	seed('queue', null, { status: { prefixArmed: true, toast: null } });
	const { container } = await mount({ width: 1600, height: 900 });

	expect(container.querySelector('[data-library-panel]')).toBeNull();
	expect(container.querySelector('[data-footer-line]')?.textContent).toContain('PREFIX');

	// The next batch carries a toast: the footer paints it in the same line.
	statusStore.set({ prefixArmed: false, toast: { text: 'Scanning', severity: 'neutral' } });
	await tick();

	expect(container.querySelector('[data-library-panel]')).toBeNull();
	expect(container.querySelector('[data-footer-line]')?.textContent).toContain('Scanning');
});

// Row 4.8's positive case (design D10): while the Library panel shows, its
// Library Status bar paints the frame's armed prefix and live toast, and the
// Queue footer line paints neither — each fact paints once a frame.
test('the Queue footer drops the prefix and the toast while the Library panel shows', async () => {
	seed('queue', music(), { status: { prefixArmed: true, toast: null } });
	const { container } = await mount({ width: 1600, height: 900 });

	expect(container.querySelector('[data-library-panel]')).toBeTruthy();
	expect(container.querySelector('[data-library-status-prefix]')?.textContent).toContain('PREFIX');
	expect(container.querySelector('[data-footer-line]')?.textContent).not.toContain('PREFIX');

	// The next batch carries a toast: the Library Status bar paints it, and the
	// footer line stays clear of it.
	statusStore.set({ prefixArmed: false, toast: { text: 'Scanning', severity: 'neutral' } });
	await tick();

	expect(container.querySelector('[data-library-status-toast]')?.textContent).toContain('Scanning');
	expect(container.querySelector('[data-footer-line]')?.textContent).not.toContain('Scanning');
});

// The Library panel's one presence fact (design D2, D10): a frame that carries
// no Library section paints no panel and hides no Queue panel, whatever content
// the window kept from an earlier frame.
test('a frame without the Library section paints only the Queue panel', async () => {
	seed('library', null);
	const { container } = await mount({ width: 700, height: 900 });

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'mini',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	expect(container.querySelector('[data-library-panel]')).toBeNull();
});

test('Mini below 480 px mounts only the Queue panel while Panel focus is queue', async () => {
	seed('queue', music());
	const { container } = await mount({ width: 700, height: 900 });

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'mini',
	);
	expect(container.querySelector('[data-panel-state]')).toBeTruthy();
	expect(container.querySelector('[data-library-panel]')).toBeNull();
});

test('Mini mounts only the Library panel while Panel focus is library', async () => {
	seed('library', music());
	const { container } = await mount({ width: 700, height: 900 });

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'mini',
	);
	expect(container.querySelector('[data-panel-state]')).toBeNull();
	expect(
		container.querySelector('[data-library-panel]')?.getAttribute('data-library-arrangement'),
	).toBe('mini');
});

test('Narrow mounts both panels without a Hero pane', async () => {
	seed('library', music());
	const { container } = await mount({ width: 1200, height: 900 });

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
	seed('library', music(hero()));
	const { container } = await mount({ width: 1200, height: 900 });

	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeTruthy();
});

test('Wide mounts the Hero pane beside the list', async () => {
	seed('library', music(hero()));
	const { container } = await mount({ width: 1600, height: 900 });

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
	seed('library', music());
	const { container } = await mount({ width: 1280, height: 500 });

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'narrow',
	);
	expect(container.querySelector('[data-library-region="hero"]')).toBeNull();
});

test('the collapsed rail leaves a wider Library column', async () => {
	seed('library', music(), { panel: { collapsed: true, queueFolded: false } });
	const { container } = await mount({ width: 1000, height: 900 });

	expect(container.querySelector('[data-arrangement]')?.getAttribute('data-arrangement')).toBe(
		'wide',
	);
});

// Design D4's dividers: a drag reports the sized column's new width, the Queue
// column paints it, and the arrangement classifies that same width; a
// double-click returns the column to its default. jsdom has no PointerEvent, so
// the drag's coordinates ride a MouseEvent of the same type.
test('dragging the Queue divider resizes the column and a double-click resets it', async () => {
	seed('library', music(hero()));
	const { container } = await mount({ width: 1600, height: 900 });
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

// add-gui-settings-screen row 6.2: while the frame carries the Settings
// screen, the window mounts the screen in place of the Queue and Library
// panels, the sidebar lists the frame's sections in order, and Mini shows
// only the pane that holds the keyboard — the content once a section opens.

/** A Settings frame section over the Queue section (design D9). */
function settingsFrame(): GuiSettingsFrame {
	return {
		section: 'queue',
		sections: ['services', 'playback', 'queue', 'actions', 'keys', 'sessions', 'playlists'],
		content: {
			kind: 'rows',
			rows: [
				{
					key: 'consumeAudio',
					label: 'Consume audio',
					kind: 'toggle',
					value: 'on',
					flag: true,
					choices: null,
					choicesKind: null,
					values: null,
				},
			],
		},
	};
}

test('the open Settings screen mounts no Queue or Library panel', async () => {
	seed('library', music(), { settings: settingsFrame() });
	const { container } = await mount({ width: 1600, height: 900 });

	expect(container.querySelector('[data-settings-screen]')).toBeTruthy();
	expect(container.querySelector('[data-panel-state]')).toBeNull();
	expect(container.querySelector('[data-library-panel]')).toBeNull();
	expect(container.querySelector('[data-settings-status]')?.textContent).toContain('Queue');
});

test('the Settings sidebar lists the frame sections in order', async () => {
	seed('queue', null, { settings: settingsFrame() });
	const { container } = await mount({ width: 1600, height: 900 });

	const sections = [...container.querySelectorAll('[data-settings-section]')].map(
		(el) => el.getAttribute('data-settings-section'),
	);

	expect(sections).toEqual([
		'services',
		'playback',
		'queue',
		'actions',
		'keys',
		'sessions',
		'playlists',
	]);
});

// The spec's *Narrow window* requirement: below the Mini threshold only the
// pane that holds the keyboard paints — the sidebar first, and the content
// once the user opens a section.
test('in Mini, opening a section shows only its content', async () => {
	seed('queue', null, { settings: settingsFrame() });
	const { container } = await mount({ width: 700, height: 900 });

	expect(container.querySelector('[data-settings-sidebar-pane]')).toBeTruthy();
	expect(container.querySelector('[data-settings-content]')).toBeNull();

	selectSettingsSection('playback');
	await tick();

	expect(container.querySelector('[data-settings-sidebar-pane]')).toBeNull();
	expect(container.querySelector('[data-settings-content]')).toBeTruthy();
});

// add-gui-library-panel row 8.1: while a plain Emby library is the active
// destination (design D12), the window mounts the Library panel with the Emby
// painters — its list, its rail, and its Hero — as it does for the other
// destinations.
test('the Emby content mounts the Library panel with the Emby painters', async () => {
	seed('library', null, { library: embyContent() });
	const { container } = await mount({ width: 1600, height: 900 });

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
	seed('library', null, { library: podcastsContent() });
	const { container } = await mount({ width: 1600, height: 900 });

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

// add-gui-library-panel row 11.1: while the Feeds destination is the active
// destination (design D12), the window mounts the Library panel with the
// Feeds painters — its pills, its date-gutter list, and its Hero — and none
// of the other destinations' painters.
test('the Feeds content mounts the Library panel with the Feeds painters', async () => {
	seed('library', null, { library: feedsContent() });
	const { container } = await mount({ width: 1600, height: 900 });

	const panel = container.querySelector('[data-library-panel]');

	expect(panel).toBeTruthy();
	expect(panel?.querySelector('[data-library-feeds-list]')).toBeTruthy();
	expect(
		panel?.querySelector('[data-library-region="hero"] [data-library-feeds-hero]'),
	).toBeTruthy();
	expect(panel?.querySelector('[data-library-feeds-pill="latest"]')).toBeTruthy();
	expect(panel?.querySelector('[data-library-list]')).toBeNull();
	expect(panel?.querySelector('[data-library-podcasts-list]')).toBeNull();
	expect(panel?.querySelector('[data-library-emby-list]')).toBeNull();
});

/** The carried content while the Feeds destination is active: the Latest
 * and one subscription pill, one merged entry row, and the selected entry's
 * Hero (design D12, row 11.1). */
function feedsContent(): LibrarySections {
	return {
		list: {
			tabs: [{ position: 5, label: 'Feeds' }],
			currentTab: 5,
			destination: {
				kind: 'feeds',
				pills: [
					{ pill: { kind: 'latest' }, name: 'Latest', selected: true, collection: false },
					{
						pill: { kind: 'subscription', id: 'https://sub.one' },
						name: 'Sub One',
						selected: false,
						collection: false,
					},
				],
				loading: false,
				failed: false,
				rows: [
					{
						id: 's1::e1',
						title: 'Entry A',
						subtitle: 'Sub One',
						date: '12 May',
						state: 'ordinary',
					},
				],
			},
		},
		detail: {
			kind: 'feeds',
			selected: 's1::e1',
			hero: {
				title: 'Entry A',
				metaRows: [[{ text: 'Sub One', kind: 'plain' }], [{ text: '12:30', kind: 'time' }]],
				durationRow: 1,
				progressRow: null,
				artShape: 'square',
			},
		},
	};
}

/** The carried content while a Podcasts library is active: the Latest pill
 * and a show pill, one merged episode row, and the selected episode's Hero
 * (design D12, row 10.1). */
function podcastsContent(): LibrarySections {
	return {
		list: {
			tabs: [{ position: 2, label: 'Podcasts' }],
			currentTab: 2,
			destination: {
				kind: 'podcasts',
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
			},
		},
		detail: {
			kind: 'podcasts',
			selected: 'e1:ep1',
			hero: {
				title: 'Episode A',
				metaRows: [[{ text: 'Show One', kind: 'plain' }], [{ text: '34:00', kind: 'time' }]],
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
function embyContent(): LibrarySections {
	return {
		list: {
			tabs: [{ position: 3, label: 'Movies' }],
			currentTab: 3,
			destination: {
				kind: 'emby',
				libIdx: 3,
				libraryName: 'Movies',
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
			},
		},
		detail: {
			kind: 'emby',
			selected: 'm1',
			hero: {
				kind: 'movie',
				title: 'Alien',
				metaRows: [[{ text: '1979', kind: 'date' }]],
				durationRow: 0,
				progressRow: null,
				overview: null,
				artShape: 'landscape',
				art: null,
				credits: [],
				imdbUrl: null,
			},
		},
	};
}
