// The GUI's two-panel arrangement (add-gui-library-panel design D4): the
// Library column sits beside the Queue panel, and the front end picks its
// arrangement from the measured Library column width — the window width minus
// the Queue panel's current width — with the canvas sizes as thresholds. The
// thresholds are front-end constants beside the queue panel's state constants
// (`panel.ts`); the core does not classify the size. The Queue column width the
// arrangement subtracts is the painted one, which the session's divider moves
// (`./resize.ts`), so a drag and the classification cannot disagree.

import type { GuiPanelToggles, LayoutSize } from './types';

/** The Queue panel's expanded width: the canvas's expanded artboard. */
export const QUEUE_PANEL_WIDTH_PX = 400;

/** The Queue panel's collapsed width: the `z` toggle's 72 px rail. */
export const QUEUE_RAIL_WIDTH_PX = 72;

/** The Library column width below which only the panel with Panel focus
 * shows: the 320 px list plus the pill row. */
export const LIBRARY_MINI_MAX_WIDTH_PX = 480;

/** The Library column width from which the Hero pane sits beside the list:
 * the canvas's 880 px, its 320 px list plus a 560 px Hero. The session's list
 * width and the divider come out of it, so the Hero pane flexes from just
 * under its artboard at the threshold. */
export const LIBRARY_WIDE_MIN_WIDTH_PX = 880;

/** The container height Wide also needs: the Hero pane's 600 px artboard. */
export const LIBRARY_WIDE_MIN_HEIGHT_PX = 600;

/** The arrangement of the Library column beside the Queue panel (design D4). */
export type GuiArrangement = 'mini' | 'narrow' | 'wide';

/**
 * The Queue panel's painted column width: the 72 px rail while the collapse
 * toggle holds, the given expanded width otherwise — the 400 px canvas
 * artboard until the session's divider moves it (design D4).
 */
export function queueColumnWidth(
	panel: GuiPanelToggles,
	expandedWidth: number = QUEUE_PANEL_WIDTH_PX,
): number {
	return panel.collapsed ? QUEUE_RAIL_WIDTH_PX : expandedWidth;
}

/** The measured Library column width: the window width minus the Queue
 * panel's painted column width (design D4). */
export function libraryColumnWidth(container: LayoutSize, queueWidth: number): number {
	return Math.max(0, container.width - queueWidth);
}

/**
 * The arrangement the Library column paints (design D4): Mini below 480 px,
 * Wide from 880 px when the container is at least 600 px tall, Narrow
 * between. A zero width reads as "not measured yet" and is Mini, which mounts
 * the panel that holds Panel focus — the Queue panel on a fresh window. The
 * caller passes the painted Queue column width, so the arrangement follows a
 * divider drag.
 */
export function libraryArrangement(container: LayoutSize, queueWidth: number): GuiArrangement {
	const width = libraryColumnWidth(container, queueWidth);
	if (width < LIBRARY_MINI_MAX_WIDTH_PX) {
		return 'mini';
	}
	if (width >= LIBRARY_WIDE_MIN_WIDTH_PX && container.height >= LIBRARY_WIDE_MIN_HEIGHT_PX) {
		return 'wide';
	}
	return 'narrow';
}
