// The front-end half of Reset UI State (add-gui-settings-screen design
// D10): a new `uiResetEpoch` drops every Library interaction store — the
// pane focus, the cursors, the filters, the open nodes, the pill focus, the
// menus, the Hero dismissals, and the scroll — while the core clears the
// kept collections and picks and the panel widths reset beside these.
// The content itself is the frame's carry, so nothing here mirrors or
// resets it; the next frame re-carries it.

import { homeUi, initialHomeUi } from './home';
import { showsUi, initialShowsUi } from './shows';
import { embyUi, initialEmbyUi } from './emby';
import { audiobooksUi, initialAudiobooksUi } from './audiobooks';
import { podcastsUi, initialPodcastsUi } from './podcasts';
import { feedsUi, initialFeedsUi } from './feeds';
import { librarySearch, initialLibrarySearch } from './search';
import { initialLibraryUi, libraryUi } from './state';
import {
	audiobooksScroll,
	embyScroll,
	feedsScroll,
	homeScroll,
	initialListScroll,
	musicScroll,
	podcastsScroll,
	showsScroll,
} from './scroll';

/** Reset every Library interaction store to its mounted value, the list scroll
 * stores included. */
export function resetLibraryInteraction(): void {
	libraryUi.set(initialLibraryUi());
	musicScroll.set(initialListScroll());
	librarySearch.set(initialLibrarySearch());
	homeUi.set(initialHomeUi());
	homeScroll.set(initialListScroll());
	showsUi.set(initialShowsUi());
	showsScroll.set(initialListScroll());
	embyUi.set(initialEmbyUi());
	embyScroll.set(initialListScroll());
	audiobooksUi.set(initialAudiobooksUi());
	audiobooksScroll.set(initialListScroll());
	podcastsUi.set(initialPodcastsUi());
	podcastsScroll.set(initialListScroll());
	feedsUi.set(initialFeedsUi());
	feedsScroll.set(initialListScroll());
}
