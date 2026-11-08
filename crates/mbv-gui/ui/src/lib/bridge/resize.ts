// The two dividers (add-gui-library-panel design D4): the Queue panel's column
// and the Wide arrangement's list column are resized by dragging, and the
// session keeps both widths. Nothing persists — a window-content reload returns
// both dividers to their defaults — and a width the user dragged outlives tab
// and breakpoint changes, because the store is module state.
//
// Every painted width resolves here, so the columns the window paints and the
// width the arrangement classifies are one value: a drag can never paint a
// column the thresholds do not measure. The Queue panel keeps its canvas width
// as its minimum, and its maximum leaves the Library column its Mini
// threshold, so a drag removes no panel and the thresholds still decide Mini,
// Narrow, and Wide.

import { writable } from 'svelte/store';
import {
	LIBRARY_MINI_MAX_WIDTH_PX,
	QUEUE_PANEL_WIDTH_PX,
	libraryColumnWidth,
	queueColumnWidth,
} from './arrangement';
import type { GuiPanelToggles, LayoutSize } from './types';

/** The list column's default width: the canvas's 320 px index, 15 % wider. */
export const LIBRARY_LIST_DEFAULT_WIDTH_PX = 368;

/** The list column's narrowest: the canvas's 320 px index. */
export const LIBRARY_LIST_MIN_WIDTH_PX = 320;

/** The list column's widest: twice the canvas's index. */
export const LIBRARY_LIST_MAX_WIDTH_PX = 640;

/** The narrowest the Hero pane keeps beside the list, so neither pane squeezes
 * the other away: the list column's own minimum. */
export const HERO_MIN_WIDTH_PX = LIBRARY_LIST_MIN_WIDTH_PX;

/** A divider's painted width: the band between two columns takes this much of
 * the container, and the Library column the Queue column leaves is measured
 * without it, so the painted columns and the thresholds stay one geometry. */
export const DIVIDER_WIDTH_PX = 6;

/** The widths the user dragged this session, in pixels: `null` while the
 * divider holds its default. */
export interface DraggedWidths {
	queue: number | null;
	list: number | null;
}

/** The session's widths before any drag: both dividers at their defaults. */
export const DEFAULT_DRAGGED_WIDTHS: DraggedWidths = { queue: null, list: null };

/**
 * The widths the session holds (design D4): a drag writes one, a double-click
 * clears it, and the window-content life is the session's, so the widths
 * survive tab and breakpoint changes and no frame carries them.
 */
export const draggedWidths = writable<DraggedWidths>(DEFAULT_DRAGGED_WIDTHS);

/**
 * A Queue column width held between the panel's canvas width and the width
 * that leaves the Library column its Mini threshold: the Queue panel never
 * narrows below its artboard and never grows enough to remove the Library
 * column's two-panel states.
 */
export function clampQueueWidth(width: number, containerWidth: number): number {
	const max = Math.max(
		QUEUE_PANEL_WIDTH_PX,
		Math.round(containerWidth) - LIBRARY_MINI_MAX_WIDTH_PX - DIVIDER_WIDTH_PX,
	);
	return Math.min(Math.max(Math.round(width), QUEUE_PANEL_WIDTH_PX), max);
}

/**
 * A list column width held between the canvas index and the widest, and short
 * enough to leave the Hero pane its minimum. A Library column too narrow for
 * both panes gives the list its minimum.
 */
export function clampListWidth(width: number, libraryColumn: number): number {
	const max = Math.max(
		LIBRARY_LIST_MIN_WIDTH_PX,
		Math.min(
			LIBRARY_LIST_MAX_WIDTH_PX,
			Math.round(libraryColumn) - HERO_MIN_WIDTH_PX - DIVIDER_WIDTH_PX,
		),
	);
	return Math.min(Math.max(Math.round(width), LIBRARY_LIST_MIN_WIDTH_PX), max);
}

/** The widths the window paints, and the Library column the Queue column
 * leaves: the one resolution the columns and the arrangement both read. */
export interface PanelWidths {
	/** The Queue panel's painted column width: the rail while collapsed. */
	queue: number;
	/** The list column's painted width, which only Wide paints. */
	list: number;
	/** The Library column's width: the container minus the Queue column and
	 * the divider between them. */
	libraryColumn: number;
}

/**
 * The painted widths from the measured container, the core's toggles, and the
 * session's drags (design D4). The stored widths are clamped again here, so a
 * width dragged on a wider window narrows with the window instead of
 * squeezing a pane away.
 */
export function panelWidths(
	container: LayoutSize,
	panel: GuiPanelToggles,
	dragged: DraggedWidths,
): PanelWidths {
	const queue = queueColumnWidth(
		panel,
		clampQueueWidth(dragged.queue ?? QUEUE_PANEL_WIDTH_PX, container.width),
	);
	// The collapsed rail paints no divider, so its 6 px stay in the column.
	const divider = panel.collapsed ? 0 : DIVIDER_WIDTH_PX;
	const libraryColumn = libraryColumnWidth(container, queue + divider);
	return {
		queue,
		list: clampListWidth(dragged.list ?? LIBRARY_LIST_DEFAULT_WIDTH_PX, libraryColumn),
		libraryColumn,
	};
}

/** A drag on the Queue panel's divider: the panel's new column width. */
export function dragQueueWidth(width: number, containerWidth: number): void {
	draggedWidths.update((widths) => ({
		...widths,
		queue: clampQueueWidth(width, containerWidth),
	}));
}

/**
 * A drag on the list column's divider: the column's new width, held inside the
 * Library column the Queue panel leaves.
 */
export function dragListWidth(width: number, libraryColumn: number): void {
	draggedWidths.update((widths) => ({
		...widths,
		list: clampListWidth(width, libraryColumn),
	}));
}

/** A double-click on the Queue panel's divider: the canvas width again. */
export function resetQueueWidth(): void {
	draggedWidths.update((widths) => ({ ...widths, queue: null }));
}

/** A double-click on the list column's divider: the default width again. */
export function resetListWidth(): void {
	draggedWidths.update((widths) => ({ ...widths, list: null }));
}
