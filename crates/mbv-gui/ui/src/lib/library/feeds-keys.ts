// The Feeds destination's key routing (add-gui-library-panel design D3,
// D12, row 11.1): the Library keys while the Feeds destination is active.
// `./keys` stays the panel's one routing site and hands a Feeds key here
// after the window keys and the open-field checks, so this module never
// installs a router. The pane focus decides the control a key acts on: the
// view pills, the list, or the Hero — which carries no Workspace and no
// cursor.
//
// The routing itself is the shared panel key handler's (`./keys-shared`,
// row 7C.2): the movements and Enter never reach the core, Esc's overlay and
// menu handling runs before this, `/` opens the list's filter, and Space
// with no focused pill stays the core's play and pause. This module keeps
// only the Feeds facts it routes over. There is no play-next on the
// destination: `n` is consumed and does nothing, and `a` sends the queue
// command.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiFeedPill, GuiFeedsList } from '$lib/bridge/types';
import { routePanelListKey } from './keys-shared';
import type { PanelKeys } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryFeedsDetail, libraryFeedsList } from './parts';
import {
	feedsCursorRow,
	focusedFeedsPill,
	moveFeedsListCursor,
	moveFeedsPillFocus,
	playFeedsRow,
	queueFeedsRow,
	requestFeedsPillSelect,
} from './feeds';
import type { FeedsItemRow } from './feeds-list';

/** The Feeds facts the shared panel key handler routes over (row 7C.2): the
 * view pills are always painted, the item the play and queue commands name
 * is the entry row itself, and there is no play-next. */
const FEEDS_KEYS: PanelKeys<GuiFeedsList, FeedsItemRow, GuiFeedPill> = {
	frame: libraryFeedsList,
	pills: {
		available: () => true,
		focused: focusedFeedsPill,
		request: requestFeedsPillSelect,
		moveFocus: moveFeedsPillFocus,
	},
	hasHero: () => (libraryFeedsDetail()?.hero ?? null) !== null,
	selectedItemId: () => libraryFeedsDetail()?.selected ?? null,
	cursorRow: feedsCursorRow,
	itemId: (row) => row.id,
	moveListCursor: moveFeedsListCursor,
	playItem: playFeedsRow,
	queueItem: queueFeedsRow,
};

/** Route a key to the Feeds keys (row 11.1): the shared handler over the
 * Feeds facts. */
export function routeFeedsPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	return routePanelListKey(event, ui, FEEDS_KEYS);
}
