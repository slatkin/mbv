// add-gui-library-panel row 7.1: the Shows Workspace's interaction state
// (design D12) — the episode cursor's movement and clamping, its
// re-resolution to the first episode when its id leaves the frame, the season
// step that sends the select-season command and stops at the pills' ends, and
// the cursor episode the Hero's actions name. The Tauri IPC is mocked; the
// asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiShowsDetail, GuiShowsEpisodeRow, GuiShowsHero } from '$lib/bridge/types';
import {
	initialShowsWorkspace,
	moveShowsEpisodeCursor,
	playShowsEpisode,
	resetShowsEpisodeCursor,
	showsWorkspace,
	stepShowsSeason,
} from './shows-workspace';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	showsWorkspace.set(initialShowsWorkspace());
});

/** A season's episodes. */
const EPISODES: GuiShowsEpisodeRow[] = [
	{ episodeId: 'ep1', number: 1, name: 'Pilot', duration: '58:00', state: 'ordinary' },
	{ episodeId: 'ep2', number: 2, name: 'Cat in the Bag', duration: '47:00', state: 'ordinary' },
];

/** The series Hero's Workspace over `episodes`, with `selectedSeason` pressed. */
function workspace(
	episodes: GuiShowsEpisodeRow[],
	selectedSeason: string,
): GuiShowsHero['workspace'] {
	return {
		seasons: [
			{ seasonId: 's1', name: 'Season 1', selected: selectedSeason === 's1' },
			{ seasonId: 's2', name: 'Season 2', selected: selectedSeason === 's2' },
		],
		selectedSeason,
		episodes,
	};
}

/** The Shows library index the episode commands name. */
const LIB_IDX = 2;

/** The selected series' detail part, with a series Hero. */
function shows(
	episodes: GuiShowsEpisodeRow[] = EPISODES,
	selectedSeason = 's1',
): GuiShowsDetail {
	const hero: GuiShowsHero = {
		title: 'Better Call Saul',
		metaRows: [[{ text: 'TV Series', kind: 'plain' }]],
		durationRow: null,
		progressRow: null,
		overview: null,
		artShape: 'landscape',
		art: null,
		workspace: workspace(episodes, selectedSeason),
	};
	return {
		workspaceLoading: false,
		selected: 'series1',
		hero,
	};
}

/** Put the cursor on `episodeId`. */
function cursorAt(episodeId: string): void {
	showsWorkspace.set({ ...initialShowsWorkspace(), cursor: episodeId });
}

test('a cursor move steps to the next episode', () => {
	cursorAt('ep1');

	moveShowsEpisodeCursor('down', shows());

	expect(get(showsWorkspace).cursor).toBe('ep2');
});

test('a cursor move past the last episode stays there', () => {
	cursorAt('ep2');

	moveShowsEpisodeCursor('down', shows());

	expect(get(showsWorkspace).cursor).toBe('ep2');
});

test('a cursor whose episode left the frame re-resolves to the first episode', () => {
	cursorAt('gone');

	moveShowsEpisodeCursor('home', shows());

	expect(get(showsWorkspace).cursor).toBe('ep1');
});

test('a cursor reset starts the Workspace over', () => {
	cursorAt('ep2');

	resetShowsEpisodeCursor();

	expect(get(showsWorkspace).cursor).toBeNull();
});

test('a season step sends the neighbouring season', () => {
	stepShowsSeason(1, LIB_IDX, shows());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsSelectSeason', libIdx: 2, seasonId: 's2' },
	});
});

test.each([
	['s1', -1],
	['s2', 1],
] as const)('a season step past the end from %s sends nothing', (selectedSeason, delta) => {
	stepShowsSeason(delta, LIB_IDX, shows(EPISODES, selectedSeason));

	expect(invokeMock).not.toHaveBeenCalled();
});

test('the play command names the cursor episode', () => {
	cursorAt('ep2');

	playShowsEpisode(LIB_IDX, shows());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsPlay', libIdx: 2, itemId: 'ep2' },
	});
});

test('an action sends nothing while the Workspace holds no episode', () => {
	cursorAt('ep1');

	playShowsEpisode(LIB_IDX, shows([]));

	expect(invokeMock).not.toHaveBeenCalled();
});
