// The playhead's test clocks (gui-change-driven-sections design D5): the
// window playhead runs on injectable clocks and schedulers, so a test
// installs fixed ones and stamps a deterministic anchor — no test waits, and
// no pending timer outlives a case. Tests only; production code imports
// nothing from here.

import { playhead, setPlayheadClocksForTest } from './playhead';
import type { GuiPlayhead } from './types';

/** One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`). */
export const TEST_SECOND = 10_000_000;

/** The wall clock the test clocks report: 20:57:40 local, so a 200 s item at
 * its 60th second finishes at exactly 21:00, the same local day. */
export const TEST_WALL = new Date(2026, 0, 1, 20, 57, 40);

/** The mutable clock state a test can move between assertions. */
export interface TestClocks {
	/** The monotonic clock, in milliseconds. */
	nowMs: number;
	/** The wall clock. */
	wall: Date;
}

/** Install fixed clocks on the window playhead and stamp `anchor`: the
 * prediction starts exactly at the anchor's position, and the finish clocks
 * read `TEST_WALL`. Returns the clock state for a test that advances it. */
export function stampPlayheadForTest(anchor: GuiPlayhead): TestClocks {
	const state: TestClocks = { nowMs: 0, wall: TEST_WALL };
	setPlayheadClocksForTest({
		nowMs: () => state.nowMs,
		wallNow: () => state.wall,
		// The schedulers record nothing: the prediction stays at its anchor
		// until the test mutates the state, so no frame loop or minute timer
		// runs under the tests.
		scheduleFrame: () => 0,
		cancelFrame: () => {},
		scheduleTimeout: () => 0,
		cancelTimeout: () => {},
	});
	playhead.stamp(anchor);
	return state;
}
