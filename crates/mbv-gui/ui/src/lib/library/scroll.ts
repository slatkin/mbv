// The Library lists' scroll stores (gui-change-driven-sections design D9). A
// list's native scroll offset and its measured viewport height live here, in a
// store of their own, apart from the interaction state in `library/state.ts`.
// A scroll step then moves only this store: the rows a list derives from the
// content, the open nodes, the filter, and the filter state keep their
// identity, and the list repaints only its visible window.

import { writable } from 'svelte/store';

/** One list's scroll position: the native offset in pixels, and the viewport
 * height the list measured (`0` until it measures). */
export interface ListScroll {
	scrollTop: number;
	viewportHeight: number;
}

/** The state a list's scroll store starts with: unscrolled and unmeasured. */
export function initialListScroll(): ListScroll {
	return { scrollTop: 0, viewportHeight: 0 };
}

/** The Music list's scroll store (the `LibraryList` canvas tree). */
export const musicScroll = writable<ListScroll>(initialListScroll());
