// The movement vocabulary and the pane routing the Library's key routers share
// (add-gui-library-panel design D3, row 7C.2): the keys the Library keeps from
// the core's router, the list movement each cursor key names, and the one key
// handler the Feeds, Podcasts, Emby, and Audiobooks destinations run over their
// own facts — the frame, the rows, the pill row, and the item commands. `./keys`
// routes the vocabulary for Music and Home and `./shows-keys` for Shows, so one
// table serves both and they cannot drift; `routePanelListKey` serves the four
// flat destinations, which copy nothing but their own extras — Emby's
// play-next, the Audiobooks chapter Workspace.

import type { KeyOutcome } from '$lib/bridge/input';
import type { ListMove } from './list';
import { openLibraryFilter } from './state';
import type { LibraryUiState } from './state';

/** The keys the Library keeps from the core's router (design D3, step 4): the
 * movements, Right and Left's open and close, and Enter. Each pane acts on the
 * ones it owns — the list's cursor, the Hero's tracklist or Workspace, or the
 * series list — and the others change nothing but still never reach the core.
 * `n` and `a` are the Library's too, and each destination's item key answers
 * them for every pane. */
export const LIST_AND_HERO_KEYS: ReadonlySet<string> = new Set([
	'Enter',
	'ArrowUp',
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'PageUp',
	'PageDown',
	'Home',
	'End',
]);

/** The list movement each cursor key names (design D3, D7). */
export const CURSOR_MOVES: Readonly<Record<string, ListMove>> = {
	ArrowUp: 'up',
	ArrowDown: 'down',
	PageUp: 'pageUp',
	PageDown: 'pageDown',
	Home: 'home',
	End: 'end',
};

/** Whether no modifier is held. */
export function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/** The pill-row facts a destination's keys read (design D6, D12): the pill
 * walk and the pill select the keys ask for. A destination with no pill row
 * leaves `pills` unset on its `PanelKeys`. */
export interface PillKeys<Frame, Pill> {
	/** Whether the frame paints the pill row: the Feeds and Podcasts rows
	 * always, the Emby one in feed-group mode only. */
	available(frame: Frame): boolean;
	/** The pill the pill row's focus holds, or `null` while no pill has it. */
	focused(frame: Frame): Pill | null;
	/** Ask for a pill to be selected: Space and Enter request it. */
	request(pill: Pill): void;
	/** Move the focused pill one pill along the row. */
	moveFocus(step: number): void;
}

/** The destination facts the shared panel key handler routes over (row 7C.2):
 * the carried frame, the list's cursor row, the item the play and queue
 * commands name, and the Hero a destination paints. Each destination's keys
 * file is this one table plus its extras. */
export interface PanelKeys<Frame, Item, Pill = never> {
	/** The carried content slice, or `null` while another destination holds
	 * the window: a key routed then answers `core`. */
	frame(): Frame | null;
	/** The pill row's facts; unset on a destination that paints none. */
	pills?: PillKeys<Frame, Pill>;
	/** Whether the frame paints a Hero pane. */
	hasHero(frame: Frame): boolean;
	/** The item the frame's Hero paints: the core's selected one. */
	selectedItemId(frame: Frame): string | null;
	/** The row the list's cursor rests on, or `null` while no painted row
	 * carries it. */
	cursorRow(): Item | null;
	/** The item id the play and queue commands name. */
	itemId(item: Item): string;
	/** Move the list's cursor one movement along. */
	moveListCursor(move: ListMove): void;
	/** Play the item Enter or a double-click names. */
	playItem(itemId: string): void;
	/** Add the item `a` names to the queue's tail. */
	queueItem(itemId: string): void;
	/** Play the item `n` names next; unset on a destination with no
	 * play-next, where `n` is consumed and does nothing. */
	playNextItem?(itemId: string): void;
	/** The Hero pane's own router, on a destination whose Hero carries a
	 * control — the Audiobooks chapter Workspace. Unset routes the Hero's
	 * default: the movements and Enter change nothing but are consumed, and
	 * `n` and `a` act on the selected item. */
	routeHero?(event: KeyboardEvent, frame: Frame): KeyOutcome;
}

/**
 * Route a key to a flat destination's keys (design D3, D12, row 7C.2): `/`
 * opens the list's filter, Space selects the focused pill and with no pill
 * focused stays the core's play and pause, the view pills walk and select,
 * the list's cursor moves and Enter plays the item it rests on, and the Hero
 * consumes the movements it has no cursor for — or routes them to the
 * destination's own Workspace. `a` queues the item the focused pane names,
 * and `n` plays it next where the destination has a play-next and is consumed
 * and does nothing where it has none. Every one of them is consumed, so no
 * Library key reaches the core's playback chords from any pane.
 */
export function routePanelListKey<Frame, Item, Pill>(
	event: KeyboardEvent,
	ui: LibraryUiState,
	keys: PanelKeys<Frame, Item, Pill>,
): KeyOutcome {
	const frame = keys.frame();
	if (frame === null) {
		return 'core';
	}
	// `/` opens the list's filter (design D3, D7): the list narrows to the
	// titles that contain the text.
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	// Space selects the focused pill (design D6). With no pill focused — and
	// on a destination with no pill row — it is the core's: Space plays and
	// pauses.
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = keys.pills?.focused(frame) ?? null;
		if (pill === null) {
			return 'core';
		}
		keys.pills?.request(pill);
		return 'handled';
	}
	if (!bare(event)) {
		return 'core';
	}
	if (ui.pane === 'pills' && keys.pills !== undefined && keys.pills.available(frame)) {
		return orRemaining(routePillKey(event, frame, keys.pills), event, keys);
	}
	if (ui.pane === 'list') {
		return routeListKey(event, keys);
	}
	if (ui.pane === 'hero' && keys.hasHero(frame)) {
		const routeHero = keys.routeHero ?? ((heroEvent) => routeHeroKey(heroEvent, frame, keys));
		return orRemaining(routeHero(event, frame), event, keys);
	}
	// The Hero pane a frame without a Hero does not paint, and any pane that
	// left a key unclaimed, still keep the Library's keys from the core: the
	// movements and Enter change nothing, and `n` and `a` act on the list's
	// cursor row.
	return routeRemainingKey(event, keys);
}

/** The key's answer, or the remaining Library keys' answer when the pane left
 * it unclaimed: a pane that does not act on a key never passes it to the
 * core's playback chords (design D3, step 4). */
function orRemaining<Frame, Item, Pill>(
	outcome: KeyOutcome,
	event: KeyboardEvent,
	keys: PanelKeys<Frame, Item, Pill>,
): KeyOutcome {
	return outcome === 'core' ? routeRemainingKey(event, keys) : outcome;
}

/** The Library's own keys no pane claims (design D3, step 4): `n` and `a` act
 * on the list's cursor row, as the Music router's item key acts from every
 * pane, and the movements and Enter change nothing at all. Every one of them
 * is consumed, so a playback chord never runs. */
function routeRemainingKey<Frame, Item, Pill>(
	event: KeyboardEvent,
	keys: PanelKeys<Frame, Item, Pill>,
): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		const row = keys.cursorRow();
		routeItemKey(event, row === null ? null : keys.itemId(row), keys);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the pill row (design D12): Right and Left walk the pills,
 * the first of them with no pill focused taking the end it walks from, and
 * Enter selects the focused pill as Space does. */
function routePillKey<Frame, Pill>(
	event: KeyboardEvent,
	frame: Frame,
	pills: PillKeys<Frame, Pill>,
): KeyOutcome {
	if (event.key === 'ArrowRight') {
		pills.moveFocus(1);
		return 'handled';
	}
	if (event.key === 'ArrowLeft') {
		pills.moveFocus(-1);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const pill = pills.focused(frame);
		if (pill !== null) {
			pills.request(pill);
		}
		return 'handled';
	}
	return 'core';
}

/** Route a key to the list's cursor (design D12): the arrows, Page Up, Page
 * Down, Home, and End move the cursor, Enter plays the item it rests on, and
 * `n` and `a` play it next or queue it. */
function routeListKey<Frame, Item, Pill>(
	event: KeyboardEvent,
	keys: PanelKeys<Frame, Item, Pill>,
): KeyOutcome {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		keys.moveListCursor(move);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const row = keys.cursorRow();
		if (row !== null) {
			keys.playItem(keys.itemId(row));
		}
		return 'handled';
	}
	if (event.key === 'n' || event.key === 'a') {
		const row = keys.cursorRow();
		routeItemKey(event, row === null ? null : keys.itemId(row), keys);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route a key to the Hero (design D12): it carries no cursor of its own, so
 * the movements and Enter change nothing but are consumed, and `n` and `a`
 * act on the item the core selected — the one the Hero paints. A destination
 * whose Hero holds a control names its own router on `PanelKeys.routeHero`. */
function routeHeroKey<Frame, Item, Pill>(
	event: KeyboardEvent,
	frame: Frame,
	keys: PanelKeys<Frame, Item, Pill>,
): KeyOutcome {
	if (event.key === 'n' || event.key === 'a') {
		routeItemKey(event, keys.selectedItemId(frame), keys);
		return 'handled';
	}
	return LIST_AND_HERO_KEYS.has(event.key) ? 'handled' : 'core';
}

/** Route `n` and `a` to one item (design D12): the list's cursor item, or the
 * one the Hero paints. `n` plays next where the destination has a play-next
 * and sends nothing where it has none; `a` sends the queue command. No item
 * sends nothing. */
function routeItemKey<Frame, Item, Pill>(
	event: KeyboardEvent,
	itemId: string | null,
	keys: PanelKeys<Frame, Item, Pill>,
): void {
	if (itemId === null) {
		return;
	}
	if (event.key === 'n') {
		keys.playNextItem?.(itemId);
		return;
	}
	keys.queueItem(itemId);
}
