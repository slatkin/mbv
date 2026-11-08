// add-gui-library-panel row 4.8: the Library Status bar (design D10) paints the
// canvas's fixed key hints and the location, and the frame's armed prefix or
// live toast replaces the hints while the location stays.

import { render } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import type { GuiLibraryTab, GuiMusicFrame, GuiMusicHero, GuiStatusBar } from '$lib/bridge/types';
import LibraryStatusBar from './LibraryStatusBar.svelte';

const TABS: GuiLibraryTab[] = [{ position: 1, label: 'Music' }];

const IDLE: GuiStatusBar = { prefixArmed: false, toast: null };

/** The picked album's Hero facts (design D5). */
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

/** The Music content with the picked album and one selected collection. */
function music(): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [{ groupId: 'g1', name: 'Hip-Hop', selected: true, collection: true }],
		loading: false,
		failure: null,
		artists: [],
		hero: hero(),
		art: null,
	};
}

function renderBar(status: GuiStatusBar = IDLE) {
	return render(LibraryStatusBar, {
		tabs: TABS,
		currentTab: 1,
		music: music(),
		status,
	});
}

test('the bar paints the canvas key hints and the location', () => {
	const { container } = renderBar();
	const hints = [...(container.querySelectorAll('[data-library-hint]') ?? [])].map((hint) =>
		hint.textContent?.trim(),
	);

	expect(hints).toEqual(['/ filter', '⏎ play', 'n next', 'a queue', 'tab pane', '1–7 library']);
	expect(
		container.querySelector('[data-library-status-location]')?.textContent?.trim(),
	).toBe('Music › Hip-Hop › Beastie Boys');
});

test('a live toast replaces the hints and keeps the location', () => {
	const { container } = renderBar({
		prefixArmed: false,
		toast: { text: "Scanning 'Movies'...", severity: 'neutral' },
	});

	expect(container.querySelector('[data-library-hint]')).toBeNull();
	expect(container.querySelector('[data-library-status-toast]')?.textContent).toContain(
		"Scanning 'Movies'...",
	);
	expect(container.querySelector('[data-library-status-location]')?.textContent).toContain(
		'Music › Hip-Hop › Beastie Boys',
	);
});

test('an armed prefix replaces the hints and keeps the location', () => {
	const { container } = renderBar({ prefixArmed: true, toast: null });

	expect(container.querySelector('[data-library-hint]')).toBeNull();
	expect(container.querySelector('[data-library-status-prefix]')?.textContent).toContain(
		'PREFIX',
	);
	expect(container.querySelector('[data-library-status-location]')?.textContent).toContain(
		'Music › Hip-Hop › Beastie Boys',
	);
});
