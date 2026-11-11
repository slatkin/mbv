// The Library panel's key routing (add-gui-library-panel design D3): the router
// the window content installs in `input.ts`, where it runs before the forwarder
// and decides whether the core ever sees a keydown. The order is the design's:
// an open core overlay or an armed prefix keeps the keyboard with the core, an
// open text field of the panel types, the window keys (Tab across the panes,
// `1`–`7` for the destination tabs) and the Library keys run in the front end
// while the Library panel shows, and every other key goes to the core.

import { get } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiFrame, GuiLibraryTab, GuiPanel, GuiSearchFrame } from '$lib/bridge/types';
import { CURSOR_MOVES, LIST_AND_HERO_KEYS } from './keys-shared';
import { stepPillFocus } from './list';
import {
	acceptLibraryFilter,
	activeLibraryPanes,
	closeLibraryFilter,
	closeLibraryHero,
	closeLibraryItemMenu,
	closeLibraryRowMenu,
	focusLibraryPane,
	focusLibraryPill,
	focusedItemId,
	focusedLibraryPill,
	heroOverlayOpen,
	libraryAudiobooks,
	libraryContent,
	libraryPodcasts,
	libraryEmby,
	libraryHome,
	libraryShows,
	libraryShown,
	libraryUi,
	moveLibraryCursor,
	moveLibraryPane,
	moveLibraryTrackCursor,
	openLibraryFilter,
	requestLibraryPillSelect,
	setLibraryCursorOpen,
} from './state';
import type { LibraryUiState } from './state';
import {
	acceptHomeFilter,
	closeHomeFilter,
	homeCursorRowId,
	moveHomeListCursor,
	playHomeRow,
	playHomeRowNext,
	queueHomeRow,
} from './home';
import { routeShowsPanelKey } from './shows-keys';
import { acceptShowsFilter, closeShowsFilter } from './shows';
import { routeEmbyPanelKey } from './emby-keys';
import { acceptEmbyFilter, closeEmbyFilter } from './emby';
import { routeAudiobooksPanelKey } from './audiobooks-keys';
import { acceptAudiobooksFilter, closeAudiobooksFilter } from './audiobooks';
import { routePodcastsPanelKey } from './podcasts-keys';
import { acceptPodcastsFilter, closePodcastsFilter } from './podcasts';
import { routeFeedsPanelKey } from './feeds-keys';
import { acceptFeedsFilter, closeFeedsFilter, libraryFeeds } from './feeds';
import {
	activateSearchHighlight,
	closeLibrarySearch,
	focusLibrarySearch,
	moveSearchHighlight,
	searchFieldFocused,
	searchFieldMounted,
	searchResultsOf,
} from './search';

/** The frame facts the routing reads. A slice `GuiFrame` satisfies it whole, so
 * the installer passes the frame it holds; a test builds only these fields. */
export type LibraryKeyFrame = Pick<
	GuiFrame,
	'focus' | 'overlay' | 'status' | 'library'
> & {
	summary: Pick<GuiFrame['summary'], 'filterActive'>;
};

/** Whether no modifier is held. */
function bare(event: KeyboardEvent): boolean {
	return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
}

/** The carried destination tabs (design D2): the tabs the top bar paints, and
 * the ones the digits open. */
function carriedTabs(): readonly GuiLibraryTab[] {
	return get(libraryContent)?.tabs ?? [];
}

/** The place of the destination tab a digit opens (design D3, step 3): `1`
 * opens the first tab after Home, so the digit is the tab's place in the
 * carried list rather than a tab position. */
function destinationPlace(event: KeyboardEvent): number | null {
	if (!bare(event) || event.key.length !== 1) {
		return null;
	}
	const place = Number(event.key);
	return place >= 1 && place <= 7 ? place : null;
}

/** Move the pane focus (design D3, step 3, D12): Tab and Shift+Tab walk the
 * panes the active destination paints — the pill row, the list, and the Hero on
 * Music; the list and the Hero on Home; the mode pills, the list, and the Hero
 * on Shows, without the Hero the Upcoming mode does not paint. The ends of that
 * walk cross the panel boundary (design D4): Tab past the last pane and
 * Shift+Tab before the first give the Queue panel Panel focus, and Queue Panel
 * focus gives the Library panel's near end the keyboard, because Panel focus
 * lives in the core. */
function takePaneFocus(panel: GuiPanel, step: number): void {
	const panes = activeLibraryPanes();
	if (panel === 'queue') {
		sendLibraryCommand({ command: 'takeFocus' });
		focusLibraryPane(step > 0 ? panes[0] : panes[panes.length - 1]);
		return;
	}
	const { pane } = get(libraryUi);
	const back = step < 0;
	const at = panes.includes(pane) ? panes.indexOf(pane) : back ? panes.length : -1;
	if (back ? at <= 0 : at === panes.length - 1) {
		sendLibraryCommand({ command: 'takeQueueFocus' });
		return;
	}
	moveLibraryPane(step);
}

/** The pills of the carried content. */
function pills() {
	return get(libraryContent)?.music?.pills ?? [];
}

/** Move the focused pill one pill along the row (design D3, step 4): Right and
 * Left walk the pills, and the first of them with no pill focused takes the
 * end it walks from. */
function movePillFocus(step: number): void {
	const pill = stepPillFocus(
		pills(),
		get(libraryUi).focusedPill,
		(entry) => entry.groupId,
		step,
	);
	if (pill !== null) {
		focusLibraryPill(pill.groupId);
	}
}

/** Close the active destination's filter with Esc (design D3, step 2): the
 * open field is the one list's own, so its close and accept run the
 * destination's own resolve, in the same order `routePanelKey` hands keys
 * over. */
function closeActiveFilter(): void {
	if (libraryHome() !== null) {
		closeHomeFilter();
		return;
	}
	if (libraryShows() !== null) {
		closeShowsFilter();
		return;
	}
	if (libraryEmby() !== null) {
		closeEmbyFilter();
		return;
	}
	if (libraryAudiobooks() !== null) {
		closeAudiobooksFilter();
		return;
	}
	if (libraryPodcasts() !== null) {
		closePodcastsFilter();
		return;
	}
	if (libraryFeeds() !== null) {
		closeFeedsFilter();
		return;
	}
	closeLibraryFilter();
}

/** Accept the active destination's filter with Enter (design D3, step 2):
 * the field closes and the list keeps its text. */
function acceptActiveFilter(): void {
	if (libraryHome() !== null) {
		acceptHomeFilter();
		return;
	}
	if (libraryShows() !== null) {
		acceptShowsFilter();
		return;
	}
	if (libraryEmby() !== null) {
		acceptEmbyFilter();
		return;
	}
	if (libraryAudiobooks() !== null) {
		acceptAudiobooksFilter();
		return;
	}
	if (libraryPodcasts() !== null) {
		acceptPodcastsFilter();
		return;
	}
	if (libraryFeeds() !== null) {
		acceptFeedsFilter();
		return;
	}
	acceptLibraryFilter();
}

/** Route a key while the filter field holds the keyboard (design D3, step 2). */
function routeFilterKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === 'Escape') {
		closeActiveFilter();
		return 'handled';
	}
	if (event.key === 'Enter') {
		acceptActiveFilter();
		return 'handled';
	}
	// The field takes the rest: the browser types the key, and the core gets
	// none.
	return 'text';
}

/** Play the item the focused pane's cursor names (design D3's Enter): the list
 * row, or the Hero's tracklist track while that pane has the pane focus. With
 * no item under the cursor the key sends no command. Enter never picks: a mouse
 * press picks the item for the Hero, and Enter plays. */
function playFocusedItem(): void {
	const itemId = focusedItemId();
	if (itemId !== null) {
		void sendLibraryCommand({ command: 'play', itemId });
	}
}

/** Route a movement or open key to the list's cursor (design D3, D7): the
 * arrows, Page Up, Page Down, Home, and End move the cursor, Right opens the
 * cursor's node, Left closes it, and Enter plays the item under it. */
function routeListKey(event: KeyboardEvent): boolean {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveLibraryCursor(move);
		return true;
	}
	if (event.key === 'ArrowRight') {
		setLibraryCursorOpen(true);
		return true;
	}
	if (event.key === 'ArrowLeft') {
		setLibraryCursorOpen(false);
		return true;
	}
	if (event.key === 'Enter') {
		playFocusedItem();
		return true;
	}
	return false;
}

/** Route a movement or Enter key to the Hero's tracklist cursor (design D3):
 * the movement keys move the tracklist's cursor, and Enter plays the track it
 * rests on. */
function routeTracklistKey(event: KeyboardEvent): boolean {
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveLibraryTrackCursor(move);
		return true;
	}
	if (event.key === 'Enter') {
		playFocusedItem();
		return true;
	}
	return false;
}

/** Route `n` and `a` to the item the focused pane's cursor names (design D3,
 * step 4, D8): play next and add to queue. The Library takes both keys while
 * it has Library Panel focus, so the core's `a` chord never runs from here;
 * with no item under the cursor the key sends no command. */
function routeItemKey(event: KeyboardEvent): void {
	const itemId = focusedItemId();
	if (itemId === null) {
		return;
	}
	if (event.key === 'n') {
		void sendLibraryCommand({ command: 'playNext', itemId });
		return;
	}
	void sendLibraryCommand({ command: 'addToQueue', itemId });
}

/** Close the front end's open menu or overlay with Esc (design D3): the right-click
 * item menu and the picked item's menu first, then the Narrow arrangement's Hero
 * overlay, then the carried Search section. Whether one paints decides the
 * answer, so an Esc with nothing open may not leave the core's Esc unclaimed. */
function routeEscapeKey(ui: LibraryUiState, search: GuiSearchFrame | null): KeyOutcome {
	if (ui.rowMenu !== null) {
		closeLibraryRowMenu();
		return 'handled';
	}
	if (ui.itemMenuOpen) {
		closeLibraryItemMenu();
		return 'handled';
	}
	if (heroOverlayOpen(ui)) {
		closeLibraryHero();
		return 'handled';
	}
	// The carried Search section (design D12, row 12.1): the dropdown paints
	// while the frame carries it, so Esc closes it even after a press took the
	// keyboard elsewhere — it is the only close the open results have.
	if (search !== null) {
		closeLibrarySearch();
		return 'handled';
	}
	return 'core';
}

/** Route a key to the Library keys (design D3, step 4), with the pane the
 * window content focused deciding which control the key acts on, and the
 * carried Search section the Esc close reads (row 12.1). On Home the
 * Home keys route instead (design D12), on Shows the Shows keys (row 7.1), on
 * a plain Emby library the Emby keys (row 8.1), and on Audiobooks the
 * Audiobooks keys (row 9.1). */
function routePanelKey(
	event: KeyboardEvent,
	ui: LibraryUiState,
	search: GuiSearchFrame | null,
): KeyOutcome {
	if (event.key === 'Escape' && bare(event)) {
		return routeEscapeKey(ui, search);
	}
	if (libraryHome() !== null) {
		return routeHomePanelKey(event);
	}
	if (libraryShows() !== null) {
		return routeShowsPanelKey(event, ui);
	}
	if (libraryEmby() !== null) {
		return routeEmbyPanelKey(event, ui);
	}
	if (libraryAudiobooks() !== null) {
		return routeAudiobooksPanelKey(event, ui);
	}
	if (libraryPodcasts() !== null) {
		return routePodcastsPanelKey(event, ui);
	}
	if (libraryFeeds() !== null) {
		return routeFeedsPanelKey(event, ui);
	}
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	// Space selects the focused pill (design D6). With
	// no pill focused, it is the core's: Space plays and pauses (design D3,
	// step 5).
	if (event.key === ' ' && !event.ctrlKey && !event.metaKey) {
		const pill = focusedLibraryPill(pills(), ui);
		if (pill === null) {
			return 'core';
		}
		requestLibraryPillSelect(pill.groupId);
		return 'handled';
	}
	if (ui.pane === 'pills' && bare(event)) {
		if (event.key === 'ArrowRight') {
			movePillFocus(1);
			return 'handled';
		}
		if (event.key === 'ArrowLeft') {
			movePillFocus(-1);
			return 'handled';
		}
		// Enter presses the focused pill, as Space does (design D3, step 4, D6).
		if (event.key === 'Enter') {
			const pill = focusedLibraryPill(pills(), ui);
			if (pill !== null) {
				requestLibraryPillSelect(pill.groupId);
				return 'handled';
			}
		}
	}
	if (ui.pane === 'list' && bare(event) && routeListKey(event)) {
		return 'handled';
	}
	if (ui.pane === 'hero' && bare(event) && routeTracklistKey(event)) {
		return 'handled';
	}
	if (bare(event) && (event.key === 'n' || event.key === 'a')) {
		routeItemKey(event);
		return 'handled';
	}
	if (bare(event) && LIST_AND_HERO_KEYS.has(event.key)) {
		return 'handled';
	}
	// Nothing else of the Library's answers this key: the core's router does.
	return 'core';
}

/** Route a key while the Search field holds the keyboard (design D3, step 2,
 * row 12.1): Esc closes the field and the results, the arrows move the
 * highlighted result, Enter activates it, and every other key is the field's
 * typing — the `text` answer lets the browser type it and never forwards it
 * to the core. With no results held, the arrows and Enter still never reach
 * the core: they would move the caret and submit nothing, and the core gets
 * none. */
function routeSearchKey(event: KeyboardEvent, search: GuiSearchFrame): KeyOutcome {
	if (event.key === 'Escape') {
		closeLibrarySearch();
		return 'handled';
	}
	if (event.key === 'ArrowDown') {
		moveSearchHighlight(1, search);
		return 'handled';
	}
	if (event.key === 'ArrowUp') {
		moveSearchHighlight(-1, search);
		return 'handled';
	}
	if (event.key === 'Enter') {
		activateSearchHighlight(search);
		return 'handled';
	}
	return 'text';
}

/** Route a key to the Home keys (design D12): the Home cursor owns the
 * movement keys wherever the pane focus sits, Enter plays the row it rests
 * on, and `n` and `a` enqueue from it. There is no filter and no pill row on
 * Home, so `/` and Space are not taken: they pass to the core's router. The
 * tree's Right and Left have no node to open, so they change nothing but
 * still never reach the core. */
function routeHomePanelKey(event: KeyboardEvent): KeyOutcome {
	if (event.key === '/' && bare(event)) {
		openLibraryFilter();
		return 'handled';
	}
	if (event.key === ' ') {
		// No pill row on Home (row 6.2): Space keeps its core effect, as it
		// does under Queue focus.
		return 'core';
	}
	if (!bare(event)) {
		return 'core';
	}
	const move = CURSOR_MOVES[event.key];
	if (move !== undefined) {
		moveHomeListCursor(move);
		return 'handled';
	}
	if (event.key === 'Enter') {
		const row = homeCursorRowId();
		if (row !== null) {
			playHomeRow(row);
		}
		return 'handled';
	}
	if (event.key === 'n') {
		const row = homeCursorRowId();
		if (row !== null) {
			playHomeRowNext(row);
		}
		return 'handled';
	}
	if (event.key === 'a') {
		const row = homeCursorRowId();
		if (row !== null) {
			queueHomeRow(row);
		}
		return 'handled';
	}
	if (LIST_AND_HERO_KEYS.has(event.key)) {
		// Right and Left have no tree node to open on Home; the other keys are
		// answered above. They still never reach the core (design D3, step 4).
		return 'handled';
	}
	return 'core';
}

/**
 * Route one keydown (design D3): the front end does its part, and the answer is
 * what the input forwarder must do with the key.
 */
export function routeLibraryKey(
	event: KeyboardEvent,
	frame: LibraryKeyFrame | null,
): KeyOutcome {
	// The attach states carry no slice, and no Library panel means no Library
	// keys: the core takes every one of them. The panel's presence is the
	// window's mount fact (design D2, D10), so a key never acts on a panel that
	// paints nothing.
	// The panel's presence is one fact for the window and the router (design
	// D2, D10): a frame that carries no Library section takes no Library key,
	// even while the window still holds the content of an earlier frame. The
	// content names the active destination's section (design D12).
	if (!frame || !libraryShown(frame.library, get(libraryContent))) {
		return 'core';
	}
	// (1) An open core overlay or an armed prefix keeps the keyboard with the
	// core: both are in the frame, and the core's own router answers them.
	if (frame.overlay !== null || frame.status.prefixArmed) {
		return 'core';
	}
	// The Queue filter's text entry is the core's, so while it holds the
	// keyboard every key is the core's to type (`text_entry_focused` in
	// `src/app/input/gui.rs`).
	if (frame.summary.filterActive) {
		return 'core';
	}
	// The `Ctrl+K` chord (design D3, step 3, row 12.1): the Search field takes
	// the keyboard wherever the Library panel shows, before the open-field
	// checks — even while the filter holds it, where the DOM focus move closes
	// the filter the way any other focus move does. The chord arms the field
	// only while a top bar mounts it: in Mini with Queue focus the panel paints
	// no top bar, and arming a field that holds no element leaves the keyboard
	// dead on its fact, so the chord keeps its core effect there instead.
	if (
		event.ctrlKey &&
		!event.metaKey &&
		!event.altKey &&
		!event.shiftKey &&
		(event.key === 'k' || event.key === 'K') &&
		searchFieldMounted()
	) {
		focusLibrarySearch();
		return 'handled';
	}
	// (2) An open text field of the panel takes the keys. The Search field
	// routes first: its hold on the keyboard is DOM focus, which no panel key
	// runs while it types (the key-leak rule), and the field's typing is the
	// `text` answer the forwarder lets through (design D3, step 2, row 12.1).
	// Both facts must hold: a stale armed fact an unmounted bar retracted must
	// never route a key under it (the same key-leak gate).
	if (searchFieldMounted() && searchFieldFocused()) {
		return routeSearchKey(event, searchResultsOf(frame.library?.search ?? null));
	}
	const ui = get(libraryUi);
	if (ui.filterOpen) {
		return routeFilterKey(event);
	}
	// (3) The window keys run whenever the Library panel shows.
	if (event.key === 'Tab' && !event.ctrlKey && !event.metaKey && !event.altKey) {
		takePaneFocus(frame.focus.panel, event.shiftKey ? -1 : 1);
		return 'handled';
	}
	const place = destinationPlace(event);
	if (place !== null) {
		// A digit names a carried tab, never a raw position: it opens the tab
		// the top bar paints at that place, and a digit past the carried tabs
		// opens nothing (design D3, step 3).
		const tab = carriedTabs()[place - 1];
		if (tab !== undefined) {
			sendLibraryCommand({ command: 'openDestination', position: tab.position });
		}
		return 'handled';
	}
	// (4) The Library keys run while the Library panel has Panel focus. The
	// carried Search section rides with them: its Esc close needs it.
	if (frame.focus.panel === 'library') {
		return routePanelKey(event, ui, frame.library?.search ?? null);
	}
	// (5) Every other key goes to the core's keyboard router.
	return 'core';
}
