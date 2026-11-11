// The Search field's front-end state (openspec change add-gui-library-panel,
// design D12, row 12.1): the query text the field types, whether the field
// holds the keyboard, and the one result row the keys highlight. The results
// themselves are the core's — the frame carries them — so this state holds
// only the facts the keys and the field own.
//
// The typing sends the debounced `searchQuery` command: the debounce keeps a
// keystroke burst from sending one request per key. The scheduler is a seam,
// so the tests drive a pending send without a real timer.

import { get, writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiSearchFrame, GuiSearchRow } from '$lib/bridge/types';

/** The Search field's front-end state (design D12, row 12.1). */
export interface LibrarySearchState {
	/** Whether a mounted top bar paints the Search field (design D3, step 3):
	 * the key router arms and routes the field only on this fact, so a chord
	 * never arms a field whose element no window paints. The top bar's own
	 * mount sets it, and its teardown retracts it. */
	mounted: boolean;
	/** Whether the Search field holds the keyboard (design D3, step 2): the
	 * key router's gate. The field's own focus events confirm it. */
	focused: boolean;
	/** The field's text, as the keys typed it. */
	query: string;
	/** The highlighted result's row id, or `null` while none is. The id
	 * survives a new results frame that still carries it. */
	highlight: string | null;
}

/** The state a fresh session starts with. */
export function initialLibrarySearch(): LibrarySearchState {
	return { mounted: false, focused: false, query: '', highlight: null };
}

/** The Library panel's Search state. */
export const librarySearch = writable<LibrarySearchState>(initialLibrarySearch());

/** The query typing waits this long before the command sends (row 12.1). */
const SEARCH_DEBOUNCE_MS = 200;

/** A scheduled send's release: a pending send cancels before the next one
 * arms. */
interface ScheduledSearch {
	cancel(): void;
}

/** The debounce seam: schedules `send` to run after `delayMs` and returns the
 * release. The default is the real timer; the tests inject a recorder that
 * holds the pending send, so no test sleeps. */
export type SearchScheduler = (
	send: () => void,
	delayMs: number,
) => ScheduledSearch;

let scheduleSearchSend: SearchScheduler = (send, delayMs) => {
	const id = setTimeout(send, delayMs);
	return { cancel: () => clearTimeout(id) };
};

/** Install a debounce seam. */
export function setSearchScheduler(next: SearchScheduler): void {
	scheduleSearchSend = next;
}

/** The pending send, or `null` while none is. */
let pending: ScheduledSearch | null = null;

/** Drop the pending query send, if one is. */
function cancelPendingSend(): void {
	pending?.cancel();
	pending = null;
}

/** The carried results section the router acts on, as the frame painted it:
 * `null` while the search section is closed. */
export function searchResultsOf(frame: GuiSearchFrame | null): GuiSearchFrame {
	return frame ?? { query: '', loading: false, failed: false, results: [] };
}

/** A mounted top bar paints the Search field: the mount sets this fact, and
 * teardown retracts it with the focus fact — an arrangement change that
 * unmounts the bar under an armed field must not leave the keyboard armed
 * where no element holds it (design D3, step 3). */
export function mountSearchField(): void {
	librarySearch.update((s) => (s.mounted ? s : { ...s, mounted: true }));
}

/** Retract the mount fact with the focus fact. The query text stays: the core
 * keeps its live query, so a remounted field paints the same search. */
export function unmountSearchField(): void {
	librarySearch.update((s) =>
		s.mounted || s.focused ? { ...s, mounted: false, focused: false } : s,
	);
}

/** Whether a mounted top bar paints the Search field (design D3, step 3): the
 * key router's second gate, beside the keyboard hold. */
export function searchFieldMounted(): boolean {
	return get(librarySearch).mounted;
}

/** Focus the Search field (design D3, step 3): the `Ctrl+K` chord. The top
 * bar's input follows this fact into DOM focus, and its focus event confirms
 * the same fact back. */
export function focusLibrarySearch(): void {
	librarySearch.update((s) => ({ ...s, focused: true }));
}

/** Whether the Search field holds the keyboard (design D3, step 2): the key
 * router asks this before the panel's other keys route. */
export function searchFieldFocused(): boolean {
	return get(librarySearch).focused;
}

/** The field lost the keyboard: a press took the focus elsewhere (the
 * component's blur event). The Esc close lands on the same fact. */
export function blurSearchField(): void {
	librarySearch.update((s) => (s.focused ? { ...s, focused: false } : s));
}

/** The field's text changed (the component's input event): the fact a frame
 * carries is the core's, but the typing keeps the text locally and schedules
 * the debounced query send. An empty field clears at once — there is no
 * query left to wait for. */
export function setSearchQuery(query: string): void {
	cancelPendingSend();
	librarySearch.update((s) => ({ ...s, query }));
	if (query === '') {
		void sendLibraryCommand({ command: 'searchQuery', query: '' });
		return;
	}
	pending = scheduleSearchSend(() => {
		pending = null;
		void sendLibraryCommand({ command: 'searchQuery', query: get(librarySearch).query });
	}, SEARCH_DEBOUNCE_MS);
}

/** Close the Search field with Esc (design D3, step 2): the field clears and
 * gives the keyboard back, and the core closes the results section. */
export function closeLibrarySearch(): void {
	cancelPendingSend();
	librarySearch.set(initialLibrarySearch());
	void sendLibraryCommand({ command: 'searchClose' });
}

/** Hold the highlighted result's row id, or `null` while none is. An id the
 * state already holds writes nothing, so the top bar's keep-the-highlight
 * effect never loops on itself: the store notifies on every set, even of an
 * unchanged value. */
export function setSearchHighlight(highlight: string | null): void {
	if (get(librarySearch).highlight === highlight) {
		return;
	}
	librarySearch.update((s) => ({ ...s, highlight }));
}

/** Move the highlighted result `step` rows (design D12, row 12.1): the rows
 * compare by id, never object identity, so a new results frame keeps the held
 * highlight whenever its id survives. An arrow with no results or no held id
 * takes the end it walks from. */
export function moveSearchHighlight(step: number, search: GuiSearchFrame): void {
	const results = search.results;
	if (results.length === 0) {
		return;
	}
	const at = results.findIndex((row) => row.id === get(librarySearch).highlight);
	const next =
		at === -1
			? step > 0
				? 0
				: results.length - 1
			: Math.min(Math.max(at + step, 0), results.length - 1);
	const row = results[next];
	librarySearch.update((s) => ({ ...s, highlight: row?.id ?? null }));
}

/** Activate the found row `row` (design D12, row 12.1): Enter on the
 * highlight, or the row's press. The command carries the row's stable id and
 * item type as the frame painted them, and the core opens it. */
export function activateSearchRow(row: GuiSearchRow): void {
	cancelPendingSend();
	void sendLibraryCommand({
		command: 'searchActivate',
		id: row.id,
		itemType: row.itemType,
	});
}

/** Activate the highlighted result, if the carried results hold one (design
 * D12, row 12.1). No highlight sends nothing. */
export function activateSearchHighlight(search: GuiSearchFrame): void {
	const row = search.results.find((r) => r.id === get(librarySearch).highlight);
	if (row === undefined) {
		return;
	}
	activateSearchRow(row);
}
