// The movement vocabulary the Library's key routers share (add-gui-library-panel
// design D3): the keys the Library keeps from the core's router and the list
// movement each cursor key names. `./keys` routes them for Music and Home and
// `./shows-keys` for Shows, so one table serves both and the two cannot drift.

import type { ListMove } from './list';

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
