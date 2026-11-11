// The plain Emby-library destination's key routing (add-gui-library-panel
// design D3, D12, row 8.1): the Library keys while an Emby library is active.
// `./keys` stays the panel's one routing site and hands an Emby key here after
// the window keys and the open-field checks, so this module never installs a
// router. The pane focus decides the control a key acts on: the feed-group
// pills, the list, or the Hero — which carries no Workspace and no cursor.
//
// The routing itself is the shared panel key handler's (`./keys-shared`,
// row 7C.2): the movements and Enter never reach the core, Esc's overlay and
// menu handling runs before this, `/` opens the list's filter, and Space
// with no focused pill — and in plain mode, which paints none — stays the
// core's play and pause. This module keeps only the Emby facts it routes
// over: the feed-group pills that exist in feed-group mode only, and `n`'s
// play-next.

import type { KeyOutcome } from '$lib/bridge/input';
import type { GuiEmbyFeedGroup, GuiEmbyFrame } from '$lib/bridge/types';
import { routePanelListKey } from './keys-shared';
import type { PanelKeys } from './keys-shared';
import type { LibraryUiState } from './state';
import { libraryEmby } from './state';
import {
	embyCursorRow,
	focusedEmbyPill,
	moveEmbyListCursor,
	moveEmbyPillFocus,
	playEmbyRow,
	playEmbyRowNext,
	queueEmbyRow,
	requestEmbyPillSelect,
} from './emby';
import type { EmbyItemRow } from './emby-list';

/** The Emby facts the shared panel key handler routes over (row 7C.2): the
 * feed-group pills are painted in feed-group mode only, the item the play
 * and queue commands name is the row's item, and `n` plays it next. */
const EMBY_KEYS: PanelKeys<GuiEmbyFrame, EmbyItemRow, GuiEmbyFeedGroup> = {
	frame: libraryEmby,
	pills: {
		available: (emby) => emby.feedGroup,
		focused: focusedEmbyPill,
		request: requestEmbyPillSelect,
		moveFocus: moveEmbyPillFocus,
	},
	hasHero: (emby) => emby.hero !== null,
	selectedItemId: (emby) => emby.selected,
	cursorRow: embyCursorRow,
	itemId: (row) => row.row.id,
	moveListCursor: moveEmbyListCursor,
	playItem: playEmbyRow,
	queueItem: queueEmbyRow,
	playNextItem: playEmbyRowNext,
};

/** Route a key to the Emby library's keys (row 8.1): the shared handler over
 * the Emby facts. Enter on the cursor row plays it — a folder or a boxset
 * too, through the core's folder-play rule — and `n` and `a` act on the item
 * the focused pane names. */
export function routeEmbyPanelKey(event: KeyboardEvent, ui: LibraryUiState): KeyOutcome {
	return routePanelListKey(event, ui, EMBY_KEYS);
}
