// The Podcasts destination's key routing (add-gui-library-panel design D3,
// D12, row 10.1): the Library keys while a Podcasts library is active.
// `./keys` stays the panel's one routing site and hands a Podcasts key here
// after the window keys and the open-field checks, so this module never
// installs a router. The pane focus decides the control a key acts on: the
// episode pills, the list, or the Hero — which carries no Workspace and no
// cursor.
//
// Everything else matches the Shows and Emby routers: the movements and Enter
// never reach the core, Esc's overlay and menu handling runs before this, `/`
// opens the list's filter, and Space with no focused pill stays the core's
// play and pause. There is no play-next on the destination: `n` is consumed
// wherever it lands and sends no command, and `a` sends the queue command.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiPodcastsFrame } from '$lib/bridge/types';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryPodcasts, openLibraryFilter } from './state';
import {
	focusedPodcastsPill,
	movePodcastsListCursor,
	movePodcastsPillFocus,
	playPodcastsRow,
	queuePodcastsRow,
	requestPodcastsPillSelect,
	podcastsCursorRow,
} from './podcasts';

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/**
 * Route a key to the Podcasts keys (row 10.1): the episode pills walk and
 * select, the list's cursor moves and Enter plays the episode it rests on —
 * the queue is replaced under the Audiobookshelf admission rule — and the
 * Hero consumes the movements it has no cursor for. `a` sends the queue
 * command for the item the focused pane names, and `n` is consumed and does
 * nothing, because podcast episodes have no play-next. Every one of them is
 * consumed, so no Library key reaches the core's playback chords from any
 * pane.
 */
export function routePodcastsPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	const podcasts = libraryPodcasts();
	if (podcasts === null) {
		return 'core';
	}
	// `/` opens the list's filter (design D3, D7): the episode list narrows to
	// the titles that contain the text.
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	// Space selects the focused pill (design D6). With no pill focused, it is
	// the core's: Space plays and pauses.
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = focusedPodcastsPill(podcasts);
		if (pill === null) {
			return 'core';
		}
		requestPodcastsPillSelect(pill);
		return 'handled';
	}
	if (!bare(event)) {
		return 'core';
	}
	if (ui.pane === 'pills') {
		return orRemaining(routePillKey(event, podcasts), event);
	}
	if (ui.pane === 'list') {
		return routeListKey(event);
	}
	if (ui.pane === 'hero' && podcasts.hero !== null) {
		return orRemaining(routeHeroKey(event, podcasts), event);
	}
	// The Hero pane a frame without a Hero does not paint, and any pane that
	// left a key unclaimed, still keep the Library's keys from the core: the
	// movements and Enter change nothing, `n` changes nothing, and `a` queues
	// the list's cursor episode.
	return routeRemainingKey(event);
}

/** The key's answer, or the remaining Library keys' answer when the pane left
 * it unclaimed: a pane that does not act on a key never passes it to the
 * core's playback chords (design D3, step 4). */
function orRemaining(outcome: KeyOutcome, event: KeyboardEvent): KeyOutcome {
	return outcome === 'core' ? routeRemainingKey(event) : outcome;
}

/** The Library's own keys no Podcasts pane claims (design D3, step 4): `a`
 * queues the list's cursor episode, as the Shows router's item key acts from
 * every pane, and `n` is consumed and does nothing — podcast episodes have no
 * play-next. The movements and Enter change nothing at all. Every one of
 * them is consumed, so a playback chord never runs from a Podcasts key. */
function routeRemainingKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeListItemKey(event, podcastsCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the episode pills (row 10.1): Right and Left walk the pills,
 * the first of them with no pill focused taking the end it walks from, and
 * Enter selects the focused pill as Space does. */
function routePillKey(event: KeyboardEvent, podcasts: GuiPodcastsFrame): KeyOutcome {
	if (event.key === 'ArrowRight') {
		movePodcastsPillFocus(1);
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		movePodcastsPillFocus(-1);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const pill = focusedPodcastsPill(podcasts);
		if (pill !== null) {
			requestPodcastsPillSelect(pill);
		}
		return 'handled';
	}
	return 'core';
}

/** Route a key to the list's cursor (row 10.1): the arrows, Page Up, Page
 * Down, Home, and End move the cursor, Enter plays the episode it rests on,
 * and `a` queues it. `n` is consumed and does nothing. */
function routeListKey(event: KeyboardEvent): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		movePodcastsListCursor(move);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const row = podcastsCursorRow();
		if (row !== null) {
			playPodcastsRow(row.row.id);
		}
		return 'handled';
	}
	if (event.key === 'n' || event.key === 'a') {
		routeListItemKey(event, podcastsCursorRow());
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route `n` and `a` to one episode (row 10.1): `a` sends the queue command,
 * and `n` sends no command, because podcast episodes have no play-next. A
 * cursor on no row sends nothing. */
function routeListItemKey(event: KeyboardEvent, row: { row: { id: string } } | null): void {
	if (event.key !== 'a' || row === null) {
		return;
	}
	queuePodcastsRow(row.row.id);
}

/** Route a key to the Hero (row 10.1): it carries no Workspace and no cursor,
 * so the movements and Enter change nothing but are consumed, and `a` queues
 * the episode the core selected — the one the Hero paints. `n` is consumed
 * and does nothing. */
function routeHeroKey(event: KeyboardEvent, podcasts: GuiPodcastsFrame): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeItemKey(event, podcasts.selected);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route `a` to one episode this bridge fact names (row 10.1): the list's
 * cursor episode, or the frame's selected one. `n` is consumed and sends no
 * command. */
function routeItemKey(event: KeyboardEvent, itemId: string | null): void {
	if (event.key !== 'a' || itemId === null) {
		return;
	}
	queuePodcastsRow(itemId);
}
