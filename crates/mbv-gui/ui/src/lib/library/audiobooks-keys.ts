// The Audiobooks destination's key routing (add-gui-library-panel design D3,
// D12, row 9.1): the Library keys while an Audiobooks library is active.
// `./keys` stays the panel's one routing site and hands an Audiobooks key here
// after the window keys and the open-field checks, so this module never
// installs a router. The pane focus decides the control a key acts on: the
// book list, or the Hero's chapter Workspace.
//
// Everything else matches the Emby router: the movements and Enter never reach
// the core, Esc's overlay and menu handling runs before this, Audiobooks paints
// no filter and no pill row in Phase 2, and Space stays the core's play and
// pause. A book has no play-next: `n` is consumed and does nothing, and `a`
// sends the queue command.

import type { KeyOutcome } from '$lib/bridge/input';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryAudiobooks } from './state';
import {
	audiobooksCursorRow,
	moveAudiobooksListCursor,
	playAudiobooksRow,
	queueAudiobooksRow,
} from './audiobooks';
import type { AudiobooksItemRow } from './audiobooks-list';
import {
	moveAbsChapterCursor,
	playAbsChapter,
	queueAbsBook,
} from '$lib/library/audiobooks-workspace';

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/**
 * Route a key to the Audiobooks keys (row 9.1): the book list's cursor moves
 * and Enter plays the book it rests on — which the Owner replaces the queue
 * with at the book's saved position — and the Hero's Workspace cursor moves
 * over the selected book's chapters. `a` queues the book the focused pane
 * names, and `n` is consumed and does nothing, because books have no
 * play-next. Every one of them is consumed, so no Library key reaches the
 * core's playback chords from any pane.
 */
export function routeAudiobooksPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	const audiobooks = libraryAudiobooks();
	if (audiobooks === null) {
		return 'core';
	}
	// Audiobooks paints no filter in Phase 2 (row 9.1): `/` keeps its core
	// effect, as it does on Home, Shows, and Emby.
	if (event.key === '/') {
		return 'core';
	}
	// No pill row rides on the destination, so no Space is a pill select:
	// Space stays the core's play and pause.
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		return 'core';
	}
	if (!bare(event)) {
		return 'core';
	}
	if (ui.pane === 'list') {
		return routeListKey(event);
	}
	if (ui.pane === 'hero' && audiobooks.hero !== null) {
		return orRemaining(routeWorkspaceKey(event, audiobooks), event);
	}
	// The Hero pane a frame without a Hero does not paint, and any pane that
	// left a key unclaimed, still keep the Library's keys from the core: the
	// movements and Enter change nothing, `n` changes nothing, and `a` queues
	// the list's cursor book.
	return routeRemainingKey(event);
}

/** The key's answer, or the remaining Library keys' answer when the pane left
 * it unclaimed: a pane that does not act on a key never passes it to the
 * core's playback chords (design D3, step 4). */
function orRemaining(outcome: KeyOutcome, event: KeyboardEvent): KeyOutcome {
	return outcome === 'core' ? routeRemainingKey(event) : outcome;
}

/** The Library's own keys no Audiobooks pane claims (design D3, step 4): `a`
 * queues the list's cursor book, as the Emby router's item key acts from every
 * pane, and `n` is consumed and does nothing — books have no play-next. The
 * movements and Enter change nothing at all. Every one of them is consumed, so
 * a playback chord never runs from an Audiobooks key. */
function routeRemainingKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeBookKey(event, audiobooksCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the book list's cursor (row 9.1): the arrows, Page Up, Page
 * Down, Home, and End move the cursor, Enter plays the book it rests on, and
 * `a` queues it. `n` is consumed and does nothing. */
function routeListKey(event: KeyboardEvent): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveAudiobooksListCursor(move);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const row = audiobooksCursorRow();
		if (row !== null) {
			playAudiobooksRow(row.row.id);
		}
		return 'handled';
	}
	if (event.key === 'n' || event.key === 'a') {
		routeBookKey(event, audiobooksCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route `n` and `a` to one book (row 9.1): the list's cursor book. `n`
 * queues nothing, because a book has no play-next; `a` sends the queue
 * command. A cursor on no book sends nothing. */
function routeBookKey(event: KeyboardEvent, row: AudiobooksItemRow | null): void {
	if (event.key !== 'a' || row === null) {
		return;
	}
	queueAudiobooksRow(row.row.id);
}

/** Route a key to the Hero's chapter Workspace (row 9.1): the movement keys
 * move the chapter cursor, Enter plays or seeks from the chapter, and `a`
 * queues the selected book. `n` is consumed and does nothing, because books
 * have no play-next. */
function routeWorkspaceKey(event: KeyboardEvent, audiobooks: NonNullable<ReturnType<typeof libraryAudiobooks>>): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveAbsChapterCursor(move, audiobooks);
		return 'handled';
	}
	if (event.key === 'Enter') {
		playAbsChapter(audiobooks);
		return 'handled';
	}
	if (event.key === 'a') {
		queueAbsBook(audiobooks);
		return 'handled';
	}
	if (event.key === 'n') {
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}
