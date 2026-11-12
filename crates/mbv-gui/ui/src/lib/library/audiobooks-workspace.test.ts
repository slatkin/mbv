// add-gui-library-panel row 9.1: the Audiobooks Workspace's interaction state
// (design D12) — the chapter cursor's movement and clamping, its re-resolution
// to the marked chapter (or the first chapter) when its chapter leaves the
// frame, the reset when the selected book changes, the chapter command the
// cursor names, and the book commands the selected book names. The Tauri IPC
// is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs. The current chapter is the window's derivation from the
// playhead (gui-change-driven-sections design D5).

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import { playhead } from '$lib/bridge/playhead';
import { TEST_SECOND as SECOND, stampPlayheadForTest } from '$lib/bridge/test-clocks';
import type {
	GuiAudiobooksChapterRow,
	GuiAudiobooksDetail,
	GuiAudiobooksHero,
} from '$lib/bridge/types';
import {
	absCursorChapter,
	currentChapterRow,
	initialAudiobooksWorkspace,
	markedChapterOf,
	moveAbsChapterCursor,
	playAbsBook,
	playAbsChapter,
	queueAbsBook,
	resetAbsChapterCursor,
	audiobooksWorkspace,
} from './audiobooks-workspace';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	audiobooksWorkspace.set(initialAudiobooksWorkspace());
	playhead.reset();
});

/** A book's chapters on the merged timeline: the first spans 0–21:00, the
 * second 21:00–45:00 (design D5's `startTicks`/`endTicks`). */
const CHAPTERS: GuiAudiobooksChapterRow[] = [
	{ chapter: 1, title: 'First Contact', duration: '21:00', startTicks: 0, endTicks: 21 * 60 * SECOND },
	{
		chapter: 2,
		title: 'Fallen Angel',
		duration: '24:00',
		startTicks: 21 * 60 * SECOND,
		endTicks: 45 * 60 * SECOND,
	},
];

/** The selected book's Hero facts with a chapter Workspace. */
function hero(rows: GuiAudiobooksChapterRow[] = CHAPTERS): GuiAudiobooksHero {
	return {
		title: 'The Caves of Steel',
		metaRows: [[{ text: 'Isaac Asimov', kind: 'plain' }], [{ text: '9 h 51 m', kind: 'time' }], [{ text: '17%', kind: 'plain' }]],
		durationRow: 1,
		progressRow: 2,
		overview: null,
		artShape: 'portrait',
		art: null,
		workspace: { rows },
	};
}

/** The Audiobooks library index the book commands name. */
const LIB_IDX = 4;

/** A detail part whose selected book's Hero carries `workspace`, with the book
 * the playhead plays when one does. */
function audiobooks(
	rows: GuiAudiobooksChapterRow[] = CHAPTERS,
	selected: string | null = 'book1',
	playingBook: string | null = null,
): GuiAudiobooksDetail {
	return {
		workspaceLoading: false,
		selected,
		playingBookId: playingBook,
		hero: selected === null ? null : hero(rows),
	};
}

/** Put the cursor on `chapter`. */
function cursorAt(chapter: number): void {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: chapter });
}

test('a cursor move steps to the next chapter', () => {
	cursorAt(1);

	moveAbsChapterCursor('down', audiobooks());

	expect(get(audiobooksWorkspace).cursor).toBe(2);
});

test('a cursor move past the last chapter stays there', () => {
	cursorAt(2);

	moveAbsChapterCursor('down', audiobooks());

	expect(get(audiobooksWorkspace).cursor).toBe(2);
});

// The current chapter is the window's derivation now (design D5): the row
// whose span holds the predicted position, while the Hero's book is the
// playing book. The fixed test clocks hold the position at its anchor.
test('the marked chapter follows the predicted position while the Hero\'s book plays', () => {
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 30 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	expect(markedChapterOf(audiobooks(CHAPTERS, 'book1', 'book1'))?.chapter).toBe(2);

	// An earlier position marks the first chapter.
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 5 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	expect(markedChapterOf(audiobooks(CHAPTERS, 'book1', 'book1'))?.chapter).toBe(1);
});

test('a different playing book marks no chapter', () => {
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 30 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	expect(markedChapterOf(audiobooks(CHAPTERS, 'book1', 'book2'))).toBeNull();
	expect(markedChapterOf(audiobooks(CHAPTERS, 'book1', null))).toBeNull();
});

// The span's end is exclusive: a position at a chapter's end marks the next
// chapter, and an empty span (`startTicks === endTicks`, an audio-part row's
// `0`/`0`) never holds one.
test('a position at a chapter\'s end marks the next chapter', () => {
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 21 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	expect(markedChapterOf(audiobooks(CHAPTERS, 'book1', 'book1'))?.chapter).toBe(2);
});

test('an empty span never holds a position', () => {
	const parts: GuiAudiobooksChapterRow[] = [
		{ chapter: 1, title: 'Part 1', duration: '21:00', startTicks: 0, endTicks: 0 },
		{ chapter: 2, title: 'Part 2', duration: '24:00', startTicks: 0, endTicks: 0 },
	];

	expect(currentChapterRow(parts, 0)).toBeNull();
	expect(currentChapterRow(parts, 10 * SECOND)).toBeNull();
});

test('a cursor keeps its chapter while the frame paints it', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 1 });
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 30 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	expect(absCursorChapter(audiobooks(CHAPTERS, 'book1', 'book1'))?.chapter).toBe(1);
});

test('a cursor whose chapter left the frame resolves to the marked chapter', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 7 });
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 30 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	expect(absCursorChapter(audiobooks(CHAPTERS, 'book1', 'book1'))?.chapter).toBe(2);
});

test('a cursor with no marked chapter resolves to the first chapter', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 7 });

	expect(absCursorChapter(audiobooks())?.chapter).toBe(1);
});

test('a cursor over a book with no chapters resolves to none', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 1 });

	expect(absCursorChapter(audiobooks([]))).toBeNull();
});

test('a cursor whose chapter left the frame moves on from the marked chapter', () => {
	cursorAt(7);
	stampPlayheadForTest({
		active: true,
		activeSlot: 1,
		positionTicks: 30 * 60 * SECOND,
		runtimeTicks: 45 * 60 * SECOND,
		paused: false,
	});

	moveAbsChapterCursor('down', audiobooks(CHAPTERS, 'book1', 'book1'));

	expect(get(audiobooksWorkspace).cursor).toBe(2);
});

test('a cursor reset starts the Workspace over', () => {
	cursorAt(2);

	resetAbsChapterCursor();

	expect(get(audiobooksWorkspace).cursor).toBeNull();
});

test('the chapter command names the cursor chapter and the selected book', () => {
	cursorAt(2);

	playAbsChapter(LIB_IDX, audiobooks());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlayChapter', libIdx: 4, bookId: 'book1', chapter: 2 },
	});
});

test('the chapter command sends nothing while no chapter carries the cursor', () => {
	audiobooksWorkspace.set(initialAudiobooksWorkspace());

	playAbsChapter(LIB_IDX, audiobooks([]));

	expect(invokeMock).not.toHaveBeenCalled();
});

test('Play sends the play command for the selected book', () => {
	playAbsBook(LIB_IDX, audiobooks());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlay', libIdx: 4, itemId: 'book1' },
	});
});

test('Add to queue sends the enqueue command for the selected book', () => {
	queueAbsBook(LIB_IDX, audiobooks());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absQueue', libIdx: 4, itemId: 'book1' },
	});
});

test('a book command sends nothing while the frame selects no book', () => {
	playAbsBook(LIB_IDX, audiobooks(CHAPTERS, null));
	queueAbsBook(LIB_IDX, audiobooks(CHAPTERS, null));

	expect(invokeMock).not.toHaveBeenCalled();
});
