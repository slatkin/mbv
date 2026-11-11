// The Podcasts destination's key routing (add-gui-library-panel design D3,
// D12, row 10.1): the Library keys while a Podcasts library is active.
// `./keys` stays the panel's one routing site and hands a Podcasts key here
// after the window keys and the open-field checks, so this module never
// installs a router. The pane focus decides the control a key acts on: the
// episode pills, the list, or the Hero — which carries no Workspace and no
// cursor.
//
// The routing itself is the shared panel key handler's (`./keys-shared`,
// row 7C.2): the movements and Enter never reach the core, Esc's overlay and
// menu handling runs before this, `/` opens the list's filter, and Space
// with no focused pill stays the core's play and pause. This module keeps
// only the Podcasts facts it routes over. There is no play-next on the
// destination: `n` is consumed and does nothing, and `a` sends the queue
// command.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiPodcastPill, GuiPodcastsFrame } from '$lib/bridge/types';
import { routePanelListKey } from './keys-shared';
import type { PanelKeys } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryPodcasts } from './state';
import {
	focusedPodcastsPill,
	movePodcastsListCursor,
	movePodcastsPillFocus,
	playPodcastsRow,
	queuePodcastsRow,
	requestPodcastsPillSelect,
	podcastsCursorRow,
} from './podcasts';
import type { PodcastsItemRow } from './podcasts-list';

/** The Podcasts facts the shared panel key handler routes over (row 7C.2):
 * the episode pills are always painted, the item the play and queue commands
 * name is the episode the cursor row carries, and there is no play-next. */
const PODCASTS_KEYS: PanelKeys<GuiPodcastsFrame, PodcastsItemRow, GuiPodcastPill> = {
	frame: libraryPodcasts,
	pills: {
		available: () => true,
		focused: focusedPodcastsPill,
		request: requestPodcastsPillSelect,
		moveFocus: movePodcastsPillFocus,
	},
	hasHero: (podcasts) => podcasts.hero !== null,
	selectedItemId: (podcasts) => podcasts.selected,
	cursorRow: podcastsCursorRow,
	itemId: (row) => row.row.id,
	moveListCursor: movePodcastsListCursor,
	playItem: playPodcastsRow,
	queueItem: queuePodcastsRow,
};

/** Route a key to the Podcasts keys (row 10.1): the shared handler over the
 * Podcasts facts. */
export function routePodcastsPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	return routePanelListKey(event, ui, PODCASTS_KEYS);
}
