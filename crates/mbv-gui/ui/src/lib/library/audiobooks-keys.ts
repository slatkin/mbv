// The Audiobooks destination's key routing (add-gui-library-panel design D3,
// D12, row 9.1): the Library keys while an Audiobooks library is active.
// `./keys` stays the panel's one routing site and hands an Audiobooks key here
// after the window keys and the open-field checks, so this module never
// installs a router. The pane focus decides the control a key acts on: the
// book list, or the Hero's chapter Workspace.
//
// The routing itself is the shared panel key handler's (`./keys-shared`,
// row 7C.2): the movements and Enter never reach the core, Esc's overlay and
// menu handling runs before this, `/` opens the list's filter, and Space
// stays the core's play and pause — Audiobooks paints no pill row. This
// module keeps only the Audiobooks facts it routes over: the Hero's own
// chapter-Workspace router, and `a`'s queue command. A book has no
// play-next: `n` is consumed and does nothing.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiAudiobooksFrame } from '$lib/bridge/types';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS, routePanelListKey } from './keys-shared';
import type { PanelKeys } from './keys-shared';
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

/** The Audiobooks facts the shared panel key handler routes over (row 7C.2):
 * no pill row rides on the destination, the item the play and queue commands
 * name is the book the cursor row carries, the Hero routes its own chapter
 * Workspace, and there is no play-next. */
const AUDIOBOOKS_KEYS: PanelKeys<GuiAudiobooksFrame, AudiobooksItemRow> = {
	frame: libraryAudiobooks,
	hasHero: (audiobooks) => audiobooks.hero !== null,
	selectedItemId: (audiobooks) => audiobooks.selected,
	cursorRow: audiobooksCursorRow,
	itemId: (row) => row.row.id,
	moveListCursor: moveAudiobooksListCursor,
	playItem: playAudiobooksRow,
	queueItem: queueAudiobooksRow,
	routeHero: routeWorkspaceKey,
};

/** Route a key to the Audiobooks keys (row 9.1): the shared handler over the
 * Audiobooks facts. Enter on the book list's cursor plays the book it rests
 * on — which the Owner replaces the queue with at the book's saved
 * position. */
export function routeAudiobooksPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	return routePanelListKey(event, ui, AUDIOBOOKS_KEYS);
}

/** Route a key to the Hero's chapter Workspace (row 9.1): the movement keys
 * move the chapter cursor, Enter plays or seeks from the chapter, and `a`
 * queues the selected book. `n` is consumed and does nothing, because books
 * have no play-next. */
function routeWorkspaceKey(event: KeyboardEvent, audiobooks: GuiAudiobooksFrame): KeyOutcome {
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
