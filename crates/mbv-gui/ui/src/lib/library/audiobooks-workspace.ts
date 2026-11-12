// The Audiobooks Hero's Workspace interaction state (add-gui-library-panel
// design D1, D12, row 9.1): the cursor over the selected book's chapters. It
// is the window content's own — the core never reads it, and the hover is the
// Workspace table's own paint — and the chapters themselves are the core's,
// carried by the frame. The chapter whose span holds the predicted position
// (design D5) is the window's derivation, read from the playhead stores while
// the Hero's book is the playing book.
//
// The cursor is a chapter number, never a row position: the frame carries the
// stable Service discriminator the TUI's chapter activation addresses, so a
// cursor whose chapter left the frame (a different book's detail, or the
// detail landing) resolves to the book's current chapter when one plays, and
// to the first chapter otherwise. The pure cursor model is
// `moveChapterCursor`; the store functions below send the commands the Hero's
// keys and controls name.

import { get, writable } from 'svelte/store';
import { playhead } from '$lib/bridge/playhead';
import { sendLibraryCommand } from '$lib/bridge/library';
import type {
	GuiAudiobooksChapterRow,
	GuiAudiobooksDetail,
	GuiAudiobooksWorkspace,
} from '$lib/bridge/types';
import type { ListMove } from './list';

/** The Workspace's interaction state (design D12). */
export interface AudiobooksWorkspaceState {
	/** The cursor's chapter number, or `null` while no chapter carries the
	 * cursor (a fresh book, and a book whose chapters have not landed). */
	cursor: number | null;
}

/** The state a freshly mounted Workspace starts with. */
export function initialAudiobooksWorkspace(): AudiobooksWorkspaceState {
	return { cursor: null };
}

/** The Workspace's interaction state. */
export const audiobooksWorkspace = writable<AudiobooksWorkspaceState>(
	initialAudiobooksWorkspace(),
);

/** The selected book's Workspace, or `null` while the book's detail has not
 * landed. */
function workspaceOf(detail: GuiAudiobooksDetail): GuiAudiobooksWorkspace | null {
	return detail.hero?.workspace ?? null;
}

/**
 * The chapter whose span holds the predicted position (design D5):
 * `startTicks <= position < endTicks` — the span's end is exclusive, so a
 * position at a chapter's end marks the next chapter, and an empty span
 * (`startTicks === endTicks`, an audio-part row's `0`/`0`) never holds one.
 * `null` while no row's span holds the position.
 */
export function currentChapterRow(
	rows: readonly GuiAudiobooksChapterRow[],
	positionTicks: number | null,
): GuiAudiobooksChapterRow | null {
	if (positionTicks === null) {
		return null;
	}
	return (
		rows.find(
			(row) => row.startTicks <= positionTicks && positionTicks < row.endTicks,
		) ?? null
	);
}

/**
 * The selected book's Hero's marked chapter: the row whose span holds the
 * predicted position while the Hero's book is the playing book (design D5).
 * The Hero's book is the frame's selection; the playing book is the frame's
 * `playingBookId`, `null` while the active slot is pending. Reads the
 * playhead's predicted position, so a reactive caller reads it through a
 * store subscription, not through this function.
 */
export function markedChapterOf(detail: GuiAudiobooksDetail): GuiAudiobooksChapterRow | null {
	if (detail.playingBookId === null || detail.playingBookId !== detail.selected) {
		return null;
	}
	return currentChapterRow(workspaceOf(detail)?.rows ?? [], get(playhead.position));
}

/**
 * The chapter the cursor rests on (design D12): the chapter the state's number
 * names while the Workspace still paints it, the marked chapter while the
 * Hero's book plays, and the Workspace's first chapter otherwise — the cursor
 * re-resolves when its chapter leaves the frame, so a book change never leaves
 * it resting on nothing.
 */
export function resolveAbsChapterCursor(
	workspace: GuiAudiobooksWorkspace | null,
	cursor: number | null,
	marked: GuiAudiobooksChapterRow | null,
): GuiAudiobooksChapterRow | null {
	const rows = workspace?.rows ?? [];
	return rows.find((row) => row.chapter === cursor) ?? marked ?? rows[0] ?? null;
}

/** The chapter the cursor rests on in the carried frame, or `null` while the
 * Workspace holds no chapter. Enter on the Workspace acts on this chapter. */
export function absCursorChapter(detail: GuiAudiobooksDetail): GuiAudiobooksChapterRow | null {
	return resolveAbsChapterCursor(
		workspaceOf(detail),
		get(audiobooksWorkspace).cursor,
		markedChapterOf(detail),
	);
}

/**
 * The chapter number a movement leaves (design D12): the next, the previous,
 * or a page away chapter, clamped to the book's ends. With no cursor on a
 * painted chapter, Up and End start from the last chapter and every other
 * movement from the first, so one key press always lands the cursor. The
 * Workspace carries no measured viewport in this unit, so a page move spans
 * one row.
 */
export function moveChapterCursor(
	rows: readonly GuiAudiobooksChapterRow[],
	cursor: number | null,
	move: ListMove,
): number | null {
	const last = rows.length - 1;
	if (last < 0) {
		return null;
	}
	const at = rows.findIndex((row) => row.chapter === cursor);
	if (at < 0) {
		return (move === 'up' || move === 'end' ? rows[last] : rows[0]).chapter;
	}
	const to = {
		up: at - 1,
		down: at + 1,
		pageUp: at - 1,
		pageDown: at + 1,
		home: 0,
		end: last,
	}[move];
	return rows[Math.min(Math.max(to, 0), last)].chapter;
}

/** Move the Workspace's cursor one movement along (design D12): the movement
 * keys while the Hero pane has the pane focus. */
export function moveAbsChapterCursor(move: ListMove, detail: GuiAudiobooksDetail): void {
	const workspace = workspaceOf(detail);
	const current = resolveAbsChapterCursor(
		workspace,
		get(audiobooksWorkspace).cursor,
		markedChapterOf(detail),
	);
	const cursor = moveChapterCursor(workspace?.rows ?? [], current?.chapter ?? null, move);
	audiobooksWorkspace.update((state) => (state.cursor === cursor ? state : { ...state, cursor }));
}

/** Play the book the frame selected from its saved position (design D12): the
 * Hero's Play. The queue is replaced through the shared admission rule, so a
 * book that admits nothing changes nothing. */
export function playAbsBook(libIdx: number, detail: GuiAudiobooksDetail): void {
	actOnBook(libIdx, detail, 'absPlay');
}

/** Add the book the frame selected to the queue (design D12). */
export function queueAbsBook(libIdx: number, detail: GuiAudiobooksDetail): void {
	actOnBook(libIdx, detail, 'absQueue');
}

/** Send one book command for the frame's selected book, and nothing while the
 * frame selects none: the selection names no book. */
function actOnBook(
	libIdx: number,
	detail: GuiAudiobooksDetail,
	command: 'absPlay' | 'absQueue',
): void {
	const itemId = detail.selected;
	if (itemId === null) {
		return;
	}
	void sendLibraryCommand({ command, libIdx, itemId });
}

/** Play or seek from the cursor's chapter (design D12): Enter on the
 * Workspace, the row's double-click, and the row's hover play control. A
 * Workspace with no chapter, or a frame that selects no book, sends nothing. */
export function playAbsChapter(libIdx: number, detail: GuiAudiobooksDetail): void {
	const bookId = detail.selected;
	const chapter = absCursorChapter(detail);
	if (bookId === null || chapter === null) {
		return;
	}
	void sendLibraryCommand({
		command: 'absPlayChapter',
		libIdx,
		bookId,
		chapter: chapter.chapter,
	});
}

/** Start the cursor over when the selected book changes (design D12): the
 * grooves are the new book's chapters, so the previous book's cursor names
 * nothing. */
export function resetAbsChapterCursor(): void {
	audiobooksWorkspace.set(initialAudiobooksWorkspace());
}

/** Settle the cursor on the row a press names (design D12): a press moves the
 * cursor onto the row it hit, as the Music list's track press does. */
export function pressAbsChapter(chapter: number): void {
	audiobooksWorkspace.update((state) =>
		state.cursor === chapter ? state : { ...state, cursor: chapter },
	);
}

