// add-tauri-gui-client row 6.6: the panel-state rules (design D10) that the
// component tests do not pin — the precedence of the two toggles over the
// container size, the 520 px boundary the compact strip starts below, and the
// compact strip's next-item reading of the frame's viewport rows.

import { expect, test } from 'vitest';
import { COMPACT_STRIP_MAX_HEIGHT, nextItemTitle, panelState } from './panel';
import type { GuiPanelToggles, GuiQueue, GuiQueueRow } from './types';

const TALL = { width: 640, height: 900 };
const SHORT = { width: 640, height: COMPACT_STRIP_MAX_HEIGHT - 1 };

function toggles(panel: Partial<GuiPanelToggles> = {}): GuiPanelToggles {
	return { collapsed: false, queueFolded: false, ...panel };
}

/** One item row of a frame's viewport. */
function item(slot: number): GuiQueueRow {
	return {
		row: 'item',
		slot,
		primary: `Track ${slot}`,
		secondary: null,
		trailing: null,
		duration: '3:12',
		endsAt: null,
		kind: 'media',
		state: { state: 'ordinary' },
		striped: false,
	};
}

function queue(rows: GuiQueueRow[], active: number | null): GuiQueue {
	return { rows, cursor: null, marks: [], active, hover: null, drag: null };
}

test.each([
	{
		name: 'the collapse toggle wins over a short container',
		container: SHORT,
		panel: toggles({ collapsed: true, queueFolded: true }),
		expected: 'rail',
	},
	{
		name: 'a container one pixel short of 520 paints the strip',
		container: SHORT,
		panel: toggles(),
		expected: 'compactStrip',
	},
	{
		name: 'a 520 px container keeps the expanded artboard',
		container: { width: 640, height: COMPACT_STRIP_MAX_HEIGHT },
		panel: toggles(),
		expected: 'expanded',
	},
	{
		name: 'the queue fold paints the expanded panel without its queue',
		container: TALL,
		panel: toggles({ queueFolded: true }),
		expected: 'expandedWithoutQueue',
	},
	{
		name: 'an unmeasured container paints the expanded artboard',
		container: { width: 0, height: 0 },
		panel: toggles(),
		expected: 'expanded',
	},
])('the panel state: $name', ({ container, panel, expected }) => {
	expect(panelState(container, panel)).toBe(expected);
});

test.each([
	{
		name: 'the row after the active slot',
		rows: [item(1), item(2), item(3)],
		active: 1,
		expected: 'Track 2',
	},
	{
		name: 'the first item row while no slot plays',
		rows: [item(1), item(2)],
		active: null,
		expected: 'Track 1',
	},
	{
		name: 'nothing while the active slot is outside the viewport',
		rows: [item(2), item(3)],
		active: 1,
		expected: null,
	},
	{
		name: 'nothing after the last row',
		rows: [item(1), item(2)],
		active: 2,
		expected: null,
	},
])('the strip next item: $name', ({ rows, active, expected }) => {
	expect(nextItemTitle(queue(rows, active))).toBe(expected);
});
