// The panel's four states (the `gui-client` spec's "the panel changes state
// with its own size"; design D10): the state is picked from the panel
// container's measured size plus the Client core's two toggles, never from the
// window size. The measurement is the same one `LayoutReport.containerSize`
// carries to the core, so the painted subtree and the core's capacity and hit
// map follow one size.
//
// The state is decided here and not by CSS `@container` rules because each
// state's acceptance renders from a frame and a container size, and jsdom
// evaluates no container query. The window still holds no interaction state
// (the spec: "The layout that the container size picks is not interaction
// state"): the size is a measurement and the toggles are the core's.

import type { GuiPanelToggles, GuiQueue, LayoutSize } from './types';

/** One of the panel's four canvas states (design D10). */
export type GuiPanelState =
	/** The canvas's expanded artboard: the playback block, the queue, and the
	 * footer line. */
	| 'expanded'
	/** The container is too short for the expanded artboard: the 96×54
	 * still, the title and its meta line, play/pause, next, and a thin
	 * progress line. */
	| 'compactStrip'
	/** The `q` toggle folded the queue to its one-line summary. */
	| 'expandedWithoutQueue'
	/** The `z` toggle collapsed the panel to its 72 px rail. */
	| 'rail';

/** The container height below which the panel paints the compact strip (the
 * spec's "shorter than 520 px"). */
export const COMPACT_STRIP_MAX_HEIGHT = 520;

/**
 * The state the panel paints: the collapsed toggle wins over every size, then
 * a container too short for the expanded artboard, then the queue fold. A
 * zero height reads as "not measured yet" and paints the expanded artboard,
 * so the first frame after a reload does not flash a strip the window is not.
 */
export function panelState(container: LayoutSize, panel: GuiPanelToggles): GuiPanelState {
	if (panel.collapsed) {
		return 'rail';
	}
	if (container.height > 0 && container.height < COMPACT_STRIP_MAX_HEIGHT) {
		return 'compactStrip';
	}
	if (panel.queueFolded) {
		return 'expandedWithoutQueue';
	}
	return 'expanded';
}

/**
 * The compact strip's next-item title: the first item row after the active
 * slot, or the first item row while no slot carries the playhead. `null` while
 * the active slot is outside the painted viewport — the frame carries the
 * viewport's rows, so a row the window cannot see cannot name the next item.
 */
export function nextItemTitle(queue: GuiQueue): string | null {
	let rest = queue.rows;
	if (queue.active !== null) {
		const index = queue.rows.findIndex(
			(row) => row.row === 'item' && row.slot === queue.active,
		);
		if (index < 0) {
			return null;
		}
		rest = queue.rows.slice(index + 1);
	}
	for (const row of rest) {
		if (row.row === 'item') {
			return row.primary;
		}
	}
	return null;
}
