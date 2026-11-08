// add-gui-library-panel row 4.2: a press inside the Library panel sends the
// take-focus command (design D4), so the core sets the focus stack and Panel
// focus together before the press acts on the row. Row 4.7 mounts the Hero in
// the arrangement's two places — the Wide pane beside the list and the Narrow
// overlay over it — with the same painters, and the overlay closes on a press
// on its backdrop (design D3). The Tauri IPC is mocked; the asserted payload is
// the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicFrame, GuiMusicHero, GuiStatusBar } from '$lib/bridge/types';
import { initialLibraryUi, libraryUi } from '$lib/library/state';
import LibraryPanel from './LibraryPanel.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	libraryUi.set(initialLibraryUi());
});

/** The frame's Status bar facts: nothing armed and no live toast. */
const idle: GuiStatusBar = { prefixArmed: false, toast: null };

/** The carried Music content with no album picked. */
function music(): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [],
		collectionNames: [],
		loading: false,
		failure: null,
		artists: [],
		hero: null,
		art: null,
	};
}

/** The picked album's Hero facts: the album block a pick fills (design D5). */
function hero(): GuiMusicHero {
	return {
		artist: 'Beastie Boys',
		collections: ['Hip-Hop'],
		albumCount: 1,
		artistTracks: null,
		album: {
			albumId: 'al1',
			name: 'Licensed To Ill',
			year: '1986',
			tracks: null,
			total: null,
		},
		menu: [],
	};
}

test('a press inside the panel sends the take-focus command', () => {
	const { container } = render(LibraryPanel, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: music(),
		status: idle,
		arrangement: 'wide',
	});
	const panel = container.querySelector('[data-library-panel]');

	expect(panel).toBeTruthy();
	fireEvent.pointerDown(panel as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
});

test('the panel paints the top bar and the pill row above the list region', () => {
	const { container } = render(LibraryPanel, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: music(),
		status: idle,
		arrangement: 'wide',
	});
	const panel = container.querySelector('[data-library-panel]');
	const regions = [...(panel?.querySelectorAll('[data-library-region]') ?? [])].map((el) =>
		el.getAttribute('data-library-region'),
	);

	expect(regions.slice(0, 3)).toEqual(['topBar', 'pills', 'hero']);
	expect(regions).toContain('list');
});

// Review of row 4.5, finding P1: a pick fills `music.hero`, so the Narrow
// Hero's place mounted over the whole list — filter, rows, and rail — with an
// opaque fill that no key dismissed. Row 4.7 paints the Hero into it and gives
// it the dismissal the review required: a press on the backdrop closes it.
test('the Narrow overlay paints the Hero and closes on a press on its backdrop', async () => {
	const { container } = render(LibraryPanel, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: { ...music(), hero: hero() },
		status: idle,
		arrangement: 'narrow',
	});
	const overlay = container.querySelector('[data-library-region="heroOverlay"]');

	expect(overlay?.querySelector('[data-library-hero]')).toBeTruthy();
	expect((overlay as HTMLElement).style.background).not.toBe('');

	await fireEvent.click(
		container.querySelector('[data-library-hero-backdrop]') as HTMLElement,
	);

	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});

test('the Wide Hero pane paints the Hero beside the list', () => {
	const { container } = render(LibraryPanel, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: { ...music(), hero: hero() },
		status: idle,
		arrangement: 'wide',
	});

	expect(
		container.querySelector('[data-library-region="hero"] [data-library-hero]'),
	).toBeTruthy();
	expect(container.querySelector('[data-library-region="heroOverlay"]')).toBeNull();
});
