// add-tauri-gui-client row 6.6: the panel-state rules (design D10) that the
// component tests do not pin — the precedence of the two toggles over the
// container size, the 520 px boundary the compact strip starts below, and the
// unmeasured container's first paint.

import { expect, test } from 'vitest';
import { COMPACT_STRIP_MAX_HEIGHT, panelState } from './panel';
import type { GuiPanelToggles } from './types';

const TALL = { width: 640, height: 900 };
const SHORT = { width: 640, height: COMPACT_STRIP_MAX_HEIGHT - 1 };

function toggles(panel: Partial<GuiPanelToggles> = {}): GuiPanelToggles {
	return { collapsed: false, queueFolded: false, uiResetEpoch: 0, ...panel };
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
