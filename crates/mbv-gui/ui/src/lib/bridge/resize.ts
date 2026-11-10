// The two dividers (add-gui-library-panel design D4): the Queue panel's column
// and the Wide arrangement's list column are resized by dragging. The widths
// persist across launches: every drag writes them to the webview's local
// storage, and the next start reads them back. A width the user dragged also
// outlives tab and breakpoint changes, because the store is module state.
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

/** The narrowest the Hero pane keeps beside the list, so neither pane squeezes
 * the other away: the list column's own minimum. */
export const HERO_MIN_WIDTH_PX = LIBRARY_LIST_MIN_WIDTH_PX;

/** The Hero pane's widest share of the Library column: 60 %, so a wide window
 * gives the freed width to the list column instead of stretching the Hero
 * across it. */
export const HERO_MAX_FRACTION = 0.6;

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

/** The local-storage key the dragged widths persist under (design D4). */
export const PANEL_WIDTHS_STORAGE_KEY = 'mbv.gui.panelWidths';

/** A saved width when it is a finite number, else `null` (the default). */
function savedWidth(value: unknown): number | null {
	return typeof value === 'number' && Number.isFinite(value) ? Math.round(value) : null;
}

/**
 * The widths a previous launch saved (design D4). A missing, unreadable, or
 * corrupt value yields the defaults, so stored data never blocks the window.
 */
export function loadDraggedWidths(storage: Storage = localStorage): DraggedWidths {
	try {
		const raw = storage.getItem(PANEL_WIDTHS_STORAGE_KEY);
		if (raw === null) {
			return { ...DEFAULT_DRAGGED_WIDTHS };
		}
		const saved = JSON.parse(raw) as Partial<Record<keyof DraggedWidths, unknown>>;
		return { queue: savedWidth(saved.queue), list: savedWidth(saved.list) };
	} catch {
		return { ...DEFAULT_DRAGGED_WIDTHS };
	}
}

/** Write the widths to local storage; a refused write only loses the save. */
function saveDraggedWidths(widths: DraggedWidths, storage: Storage = localStorage): void {
	try {
		storage.setItem(PANEL_WIDTHS_STORAGE_KEY, JSON.stringify(widths));
	} catch {
		// Storage is full or disabled: the session still holds the widths.
	}
}

/**
 * The widths the session holds (design D4): a drag writes one, a double-click
 * clears it, and every change is saved, so the widths survive a relaunch as
 * well as tab and breakpoint changes.
 */
export const draggedWidths = writable<DraggedWidths>(loadDraggedWidths());
draggedWidths.subscribe((widths) => saveDraggedWidths(widths));

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
 * A list column width held between the Hero cap's floor and the room the Hero
 * pane leaves beside it at its minimum. The Hero pane never exceeds its 60 %
 * share of the Library column, so the list column's floor is the width that
 * cap leaves: a drag left of the cap boundary stops there, the way it stops at
 * the canvas index. A Library column too narrow for both panes gives the list
 * its minimum.
 */
export function clampListWidth(width: number, libraryColumn: number): number {
	const heroMax = Math.round(libraryColumn * HERO_MAX_FRACTION);
	const min = Math.max(
		LIBRARY_LIST_MIN_WIDTH_PX,
		Math.round(libraryColumn) - heroMax - DIVIDER_WIDTH_PX,
	);
	const max = Math.max(
		min,
		Math.round(libraryColumn) - HERO_MIN_WIDTH_PX - DIVIDER_WIDTH_PX,
	);
	return Math.min(Math.max(Math.round(width), min), max);
}

/** The Wide Hero pane's painted width: its 60 % share of the Library column,
 * or the room the list column leaves, whichever is narrower. The list column
 * paints the remainder, so the three widths always fill the column. */
export function heroPaneWidth(libraryColumn: number, listWidth: number): number {
	const cap = Math.round(libraryColumn * HERO_MAX_FRACTION);
	return Math.min(cap, Math.round(libraryColumn) - listWidth - DIVIDER_WIDTH_PX);
}

/** The widths the window paints, and the Library column the Queue column
 * leaves: the one resolution the columns and the arrangement both read. */
export interface PanelWidths {
	/** The Queue panel's painted column width: the rail while collapsed. */
	queue: number;
	/** The list column's painted width, which only Wide paints. */
	list: number;
	/** The Wide Hero pane's painted width: at most its 60 % share of the
	 * Library column. */
	hero: number;
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
	const draggedList = clampListWidth(dragged.list ?? LIBRARY_LIST_DEFAULT_WIDTH_PX, libraryColumn);
	const hero = heroPaneWidth(libraryColumn, draggedList);
	return {
		queue,
		// The painted list width: the dragged width, or the share the Hero
		// pane's cap leaves when the cap binds.
		list: libraryColumn - hero - DIVIDER_WIDTH_PX,
		hero,
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
