// add-gui-library-panel row 9.1: the Audiobooks Hero (design D12) — the
// portrait cover-box, the title, the meta rows with their duration and
// progress roles, and the chapters as the Workspace with the playing chapter
// marked. The actions send the Audiobooks commands with the frame's selected
// book, and a chapter's double-click and hover play send that chapter. The
// Tauri IPC is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
	GuiAudiobooksChapterRow,
	GuiAudiobooksFrame,
	GuiAudiobooksHero,
} from '$lib/bridge/types';
import { initialAudiobooksWorkspace, audiobooksWorkspace } from '$lib/library/audiobooks-workspace';
import LibraryAudiobooksHero from './LibraryAudiobooksHero.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	audiobooksWorkspace.set(initialAudiobooksWorkspace());
});

/** The library index the Audiobooks commands name. */
const LIB_IDX = 4;

/** A book's chapters: one untouched, one playing at the playhead. */
const CHAPTERS: GuiAudiobooksChapterRow[] = [
	{ chapter: 1, title: 'First Contact', duration: '21:00', current: false },
	{ chapter: 2, title: 'Fallen Angel', duration: '24:00', current: true },
];

/**
 * The selected book's Hero facts: the author, the duration, and the progress
 * rows with their roles, and the chapter Workspace (design D12, row 9.1).
 * `factOverrides` and `frameOverrides` re-shape one test's payload.
 */
function mountFrame(heroFacts: GuiAudiobooksHero | null, frameLoading = false): HTMLElement {
	const frame: GuiAudiobooksFrame = {
		libIdx: LIB_IDX,
		loading: false,
		failed: false,
		workspaceLoading: frameLoading,
		rows: [
			{
				id: 'book1',
				title: 'The Caves of Steel',
				subtitle: 'Isaac Asimov',
				duration: '9:51:00',
				sortLetter: 'A',
			},
		],
		selected: 'book1',
		hero: heroFacts,
	};
	return render(LibraryAudiobooksHero, { audiobooks: frame }).container;
}

/** The selected book's Hero facts over the shared chapter Workspace. */
function hero(facts: Partial<GuiAudiobooksHero> = {}): GuiAudiobooksHero {
	return {
		title: 'The Caves of Steel',
		metaRows: [[{ text: 'Isaac Asimov', kind: 'plain' }], [{ text: '9 h 51 m', kind: 'time' }], [{ text: '17%', kind: 'plain' }]],
		durationRow: 1,
		progressRow: 2,
		overview: null,
		artShape: 'portrait',
		art: null,
		workspace: { rows: CHAPTERS },
		...facts,
	};
}

/** One chapter row of the painted Workspace. */
function chapterRow(container: HTMLElement, chapter: number): HTMLElement {
	return container.querySelector(`[data-library-abs-chapter="${chapter}"]`) as HTMLElement;
}

// The cover art is the book artwork policy's portrait: the Hero places its
// artwork box at the portrait arm's size.
test('the portrait Hero places its artwork box at the portrait size', () => {
	const container = mountFrame(hero());
	const box = container.querySelector('[data-library-abs-hero-art]') as HTMLElement;

	expect(box.style.width).toBe('300px');
	expect(box.style.height).toBe('450px');
});

test('the Hero paints its title, its plain facts under it, and its duration and progress beside the art', () => {
	const container = mountFrame(hero());

	expect(container.querySelector('[data-library-abs-hero-title]')?.textContent).toBe(
		'The Caves of Steel',
	);
	expect(container.querySelector('[data-library-abs-hero-facts]')?.textContent).toBe(
		'Isaac Asimov',
	);
	const meta = [...container.querySelectorAll('[data-library-abs-hero-role]')];
	expect(meta.map((row) => row.textContent)).toEqual(['9 h 51 m', '17%']);
	expect(meta.map((row) => row.getAttribute('data-library-abs-hero-role'))).toEqual([
		'duration',
		'progress',
	]);
});

// The book's facts carry no overview: the Hero keeps the TUI's facts.
test('the Hero paints no overview', () => {
	const container = mountFrame(hero());

	expect(container.querySelector('[data-library-abs-hero-overview]')).toBeNull();
});

test('the Workspace paints the chapter rows once the chapters have landed', () => {
	const container = mountFrame(hero());

	expect(container.querySelectorAll('[data-library-abs-chapter]').length).toBe(2);
	expect(
		chapterRow(container, 2).querySelector('[data-library-abs-chapter-name]')?.textContent?.trim(),
	).toBe('Fallen Angel');
	expect(
		chapterRow(container, 2).querySelector('[data-library-abs-chapter-duration]')?.textContent,
	).toBe('24:00');
});

// The chapter that plays carries the tracklist's play glyph: a highlight marks
// focus only, so the row keeps its own fill.
test('the Workspace marks the playing chapter with the play glyph', () => {
	const container = mountFrame(hero());

	expect(chapterRow(container, 2).getAttribute('data-library-abs-chapter-playing')).toBe('');
	expect(chapterRow(container, 2).querySelector('[data-library-abs-chapter-glyph]')).not.toBeNull();
	expect(chapterRow(container, 1).getAttribute('data-library-abs-chapter-playing')).toBeNull();
	expect(chapterRow(container, 1).querySelector('[data-library-abs-chapter-glyph]')).toBeNull();
});

// A book whose detail has not landed paints the loading line instead of rows.
test('a book whose detail has not landed paints the loading line', () => {
	const container = mountFrame(hero({ workspace: null }), true);

	expect(container.querySelector('[data-library-abs-chapters-loading]')?.textContent).toContain(
		'Loading chapters',
	);
	expect(container.querySelector('[data-library-abs-chapter]')).toBeNull();
});

test('a Hero that paints none paints nothing', () => {
	const container = mountFrame(null);

	expect(container.querySelector('[data-library-abs-hero]')).toBeNull();
});

test('a double-click on a chapter sends its play-chapter command', async () => {
	const container = mountFrame(hero());

	await fireEvent.dblClick(
		chapterRow(container, 2).querySelector('[data-library-abs-chapter-name]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlayChapter', libIdx: LIB_IDX, bookId: 'book1', chapter: 2 },
	});
});

test('a hovered chapter row plays its chapter', async () => {
	const container = mountFrame(hero());
	const row = chapterRow(container, 2);
	await fireEvent.pointerEnter(row);

	await fireEvent.click(
		row.querySelector('[data-library-abs-chapter-action="play"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlayChapter', libIdx: LIB_IDX, bookId: 'book1', chapter: 2 },
	});
});

test('a press on a chapter moves the cursor onto its row', async () => {
	const container = mountFrame(hero());

	await fireEvent.click(
		chapterRow(container, 1).querySelector('[data-library-abs-chapter-name]') as HTMLElement,
	);

	expect(get(audiobooksWorkspace).cursor).toBe(1);
});

test('Play sends the play command for the selected book', async () => {
	const container = mountFrame(hero());

	await fireEvent.click(container.querySelector('[data-library-abs-hero-play]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlay', libIdx: LIB_IDX, itemId: 'book1' },
	});
});

test('Add to queue sends the enqueue command for the selected book', async () => {
	const container = mountFrame(hero());

	await fireEvent.click(container.querySelector('[data-library-abs-hero-add]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absQueue', libIdx: LIB_IDX, itemId: 'book1' },
	});
});

// The Hero paints the frame's header band, as Music and Shows do: the kicker
// over the title, above the artwork (the frame is one look for every Hero).
test('the Hero paints the frame header above its artwork', () => {
	const container = mountFrame(hero());

	expect(container.querySelector('[data-library-abs-hero-kicker]')?.textContent).toBe('audiobook');
	expect(container.querySelector('[data-library-abs-hero-title]')?.tagName).toBe('H1');
});
