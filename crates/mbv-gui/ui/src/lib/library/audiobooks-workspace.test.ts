// add-gui-library-panel row 9.1: the Audiobooks Workspace's interaction state
// (design D12) — the chapter cursor's movement and clamping, its re-resolution
// to the current chapter (or the first chapter) when its chapter leaves the
// frame, the reset when the selected book changes, the chapter command the
// cursor names, and the book commands the selected book names. The Tauri IPC
// is mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
	GuiAudiobooksChapterRow,
	GuiAudiobooksFrame,
	GuiAudiobooksHero,
} from '$lib/bridge/types';
import {
	absCursorChapter,
	initialAudiobooksWorkspace,
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
});

/** A book's chapters: one untouched, one playing at the playhead. */
const CHAPTERS: GuiAudiobooksChapterRow[] = [
	{ chapter: 1, title: 'First Contact', duration: '21:00', current: false },
	{ chapter: 2, title: 'Fallen Angel', duration: '24:00', current: true },
];

/** The selected book's Hero facts with a chapter Workspace. */
function hero(rows: GuiAudiobooksChapterRow[] = CHAPTERS): GuiAudiobooksHero {
	return {
		title: 'The Caves of Steel',
		metaRows: ['Isaac Asimov', '9 h 51 m', '17%'],
		durationRow: 1,
		progressRow: 2,
		overview: null,
		artShape: 'portrait',
		art: null,
		workspace: { rows },
	};
}

/** A frame whose selected book's Hero carries `workspace`. */
function audiobooks(
	rows: GuiAudiobooksChapterRow[] = CHAPTERS,
	selected: string | null = 'book1',
): GuiAudiobooksFrame {
	return {
		libIdx: 4,
		loading: false,
		failed: false,
		workspaceLoading: false,
		rows: [{ id: 'book1', title: 'The Caves of Steel', subtitle: 'Isaac Asimov', duration: '9:51:00', sortLetter: 'A' }],
		selected,
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

// A cursor whose chapter left the frame re-resolves to the playing chapter,
// without it to the first chapter, and it keeps its chapter while the frame
// paints it.
test('a cursor whose chapter left the frame resolves to the playing chapter', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 7 });

	expect(absCursorChapter(audiobooks())?.chapter).toBe(2);
});

test('a cursor with no playing chapter resolves to the first chapter', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 7 });
	const idle = CHAPTERS.map((row) => ({ ...row, current: false }));

	expect(absCursorChapter(audiobooks(idle))?.chapter).toBe(1);
});

test('a cursor keeps its chapter while the frame paints it', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 1 });

	expect(absCursorChapter(audiobooks())?.chapter).toBe(1);
});

test('a cursor over a book with no chapters resolves to none', () => {
	audiobooksWorkspace.set({ ...initialAudiobooksWorkspace(), cursor: 1 });

	expect(absCursorChapter(audiobooks([]))).toBeNull();
});

test('a cursor whose chapter left the frame moves on from the playing chapter', () => {
	cursorAt(7);

	moveAbsChapterCursor('down', audiobooks());

	expect(get(audiobooksWorkspace).cursor).toBe(2);
});

test('a cursor reset starts the Workspace over', () => {
	cursorAt(2);

	resetAbsChapterCursor();

	expect(get(audiobooksWorkspace).cursor).toBeNull();
});

test('the chapter command names the cursor chapter and the selected book', () => {
	cursorAt(2);

	playAbsChapter(audiobooks());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlayChapter', libIdx: 4, bookId: 'book1', chapter: 2 },
	});
});

test('the chapter command sends nothing while no chapter carries the cursor', () => {
	audiobooksWorkspace.set(initialAudiobooksWorkspace());

	playAbsChapter(audiobooks([]));

	expect(invokeMock).not.toHaveBeenCalled();
});

test('Play sends the play command for the selected book', () => {
	playAbsBook(audiobooks());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlay', libIdx: 4, itemId: 'book1' },
	});
});

test('Add to queue sends the enqueue command for the selected book', () => {
	queueAbsBook(audiobooks());

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absQueue', libIdx: 4, itemId: 'book1' },
	});
});

test('a book command sends nothing while the frame selects no book', () => {
	playAbsBook(audiobooks(CHAPTERS, null));
	queueAbsBook(audiobooks(CHAPTERS, null));

	expect(invokeMock).not.toHaveBeenCalled();
});
