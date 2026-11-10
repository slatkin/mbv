// add-gui-library-panel design D4: the two dividers' contract. This module
// owns the widths the arrangement classifies — the Queue panel's column and the
// Wide list column — so the defaults, the clamps, the reset, and the one
// resolution the paint and the thresholds share are stated here. The
// arrangement's own thresholds are `arrangement.ts`'s.

import { get } from 'svelte/store';
import { beforeEach, expect, test } from 'vitest';
import { libraryArrangement } from './arrangement';
import {
	DEFAULT_DRAGGED_WIDTHS,
	DIVIDER_WIDTH_PX,
	LIBRARY_LIST_DEFAULT_WIDTH_PX,
	LIBRARY_LIST_MIN_WIDTH_PX,
	PANEL_WIDTHS_STORAGE_KEY,
	dragListWidth,
	dragQueueWidth,
	draggedWidths,
	loadDraggedWidths,
	panelWidths,
	resetListWidth,
	resetQueueWidth,
} from './resize';

/** The core's toggles: the expanded panel with its queue painted. */
const expanded = { collapsed: false, queueFolded: false };

/** The collapsed panel: the 72 px rail, whatever the dragged width is. */
const collapsed = { collapsed: true, queueFolded: false };

const container = { width: 1600, height: 900 };

beforeEach(() => {
	draggedWidths.set(DEFAULT_DRAGGED_WIDTHS);
});

test('both dividers start at their defaults', () => {
	const widths = panelWidths(container, expanded, get(draggedWidths));

	expect(widths.queue).toBe(400);
	// The request made the list column 15 % wider than the canvas's index.
	expect(LIBRARY_LIST_DEFAULT_WIDTH_PX).toBe(Math.round(LIBRARY_LIST_MIN_WIDTH_PX * 1.15));
	expect(widths.list).toBe(LIBRARY_LIST_DEFAULT_WIDTH_PX);
	// The divider between the columns takes its own width.
	expect(widths.libraryColumn).toBe(1200 - DIVIDER_WIDTH_PX);
});

test('the Queue column stays between its canvas width and the Library Mini threshold', () => {
	dragQueueWidth(100, 1600);
	expect(get(draggedWidths).queue).toBe(400);

	dragQueueWidth(1400, 1600);
	expect(get(draggedWidths).queue).toBe(1120 - DIVIDER_WIDTH_PX);

	dragQueueWidth(500, 1600);
	expect(panelWidths(container, expanded, get(draggedWidths)).queue).toBe(500);
});

test('the list column stays between the canvas index and the room the Hero leaves its minimum', () => {
	dragListWidth(100, 1200);
	expect(get(draggedWidths).list).toBe(LIBRARY_LIST_MIN_WIDTH_PX);

	// 1200 px less the Hero's 320 px minimum and the 6 px divider.
	dragListWidth(900, 1200);
	expect(get(draggedWidths).list).toBe(874);

	dragListWidth(500, 1200);
	expect(panelWidths(container, expanded, get(draggedWidths)).list).toBe(500);
});

test('the collapsed rail keeps its width and hands the rest to the Library column', () => {
	dragQueueWidth(500, 1600);
	const widths = panelWidths(container, collapsed, get(draggedWidths));

	expect(widths.queue).toBe(72);
	expect(widths.libraryColumn).toBe(1528);
});

test('a stored width is clamped again when the window narrows', () => {
	dragQueueWidth(1100, 1600);
	const widths = panelWidths({ width: 1200, height: 900 }, expanded, get(draggedWidths));

	expect(widths.queue).toBe(720 - DIVIDER_WIDTH_PX);
	expect(widths.libraryColumn).toBe(480);
});

// The no-flapping contract (design D4): the arrangement classifies the width
// the columns paint, so a drag that shrinks the Library column crosses the
// threshold instead of leaving the paint and the classification apart.
test('the arrangement classifies the width a drag paints', () => {
	dragQueueWidth(1120 - DIVIDER_WIDTH_PX, 1600);
	const widths = panelWidths(container, expanded, get(draggedWidths));

	expect(libraryArrangement(container, widths.queue)).toBe('narrow');
});

test('a double-click returns each divider to its default', () => {
	dragQueueWidth(500, 1600);
	dragListWidth(500, 1200);
	resetQueueWidth();
	resetListWidth();

	const widths = panelWidths(container, expanded, get(draggedWidths));
	expect(widths.queue).toBe(400);
	expect(widths.list).toBe(LIBRARY_LIST_DEFAULT_WIDTH_PX);
});

// The sticky widths (design D4): a drag is saved, a relaunch reads it back, and
// a corrupt saved value falls back to the defaults.
test('a dragged width is saved and read back on the next start', () => {
	localStorage.clear();

	dragListWidth(420, 1400);

	expect(JSON.parse(localStorage.getItem(PANEL_WIDTHS_STORAGE_KEY) ?? 'null')).toEqual({
		queue: null,
		list: 420,
	});
	expect(loadDraggedWidths()).toEqual({ queue: null, list: 420 });
	resetListWidth();
});

test('a corrupt saved value loads the defaults', () => {
	const corrupt = { getItem: () => '{not json', setItem: () => {} } as unknown as Storage;

	expect(loadDraggedWidths(corrupt)).toEqual(DEFAULT_DRAGGED_WIDTHS);
});
