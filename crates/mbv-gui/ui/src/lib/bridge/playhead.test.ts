// gui-change-driven-sections task 4.3: the playhead's prediction (design D5)
// — the position advances by the injected elapsed time while playing and
// holds while paused, and while paused the finish times move with the
// injected wall clock. The clocks and the schedulers are recorders, so no
// test waits: a test advances the injected clocks and fires the recorded
// frame step and minute timeout.

import { get } from 'svelte/store';
import { expect, test } from 'vitest';
import type { Playhead, PlayheadClocks } from './playhead';
import { createPlayhead } from './playhead';
import type { GuiPlayhead } from './types';

/** One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`). */
const SECOND = 10_000_000;

/** The injected clocks and the handles a test fires. */
interface FakeClocks {
	clocks: PlayheadClocks;
	/** Move the monotonic clock, the prediction's elapsed source. */
	advanceMs(ms: number): void;
	/** Move the wall clock, the finish times' now source. */
	advanceWall(ms: number): void;
	/** Run the pending animation-frame step, one frame. */
	fireFrame(): void;
	/** Run the pending minute timeout, one wall-clock minute. */
	fireMinute(): void;
}

/** A recorder pair of clocks: nothing schedules for real, and the tests fire
 * what the playhead scheduled. */
function fakeClocks(): FakeClocks {
	let nowMs = 1_000;
	let wall = new Date(2026, 0, 1, 20, 0, 0);
	let frameStep: (() => void) | undefined;
	let minuteStep: (() => void) | undefined;
	const clocks: PlayheadClocks = {
		nowMs: () => nowMs,
		wallNow: () => wall,
		scheduleFrame: (step) => {
			frameStep = step;
			return 1;
		},
		cancelFrame: () => {
			frameStep = undefined;
		},
		scheduleTimeout: (step) => {
			minuteStep = step;
			return 2;
		},
		cancelTimeout: () => {
			minuteStep = undefined;
		},
	};
	return {
		clocks,
		advanceMs: (ms) => {
			nowMs += ms;
		},
		advanceWall: (ms) => {
			wall = new Date(wall.getTime() + ms);
		},
		fireFrame: () => frameStep?.(),
		fireMinute: () => minuteStep?.(),
	};
}

/** An anchor playing `positionTicks` of a 200 s item in slot 7. */
function playing(positionTicks: number): GuiPlayhead {
	return {
		active: true,
		activeSlot: 7,
		positionTicks,
		runtimeTicks: 200 * SECOND,
		paused: false,
	};
}

/** A playhead over the injected clocks, with a stamped anchor. */
function stamped(anchor: GuiPlayhead): { playhead: Playhead; fake: FakeClocks } {
	const fake = fakeClocks();
	const playhead = createPlayhead(fake.clocks);
	playhead.stamp(anchor);
	return { playhead, fake };
}

test('the predicted position advances by the injected elapsed time while playing', () => {
	const { playhead, fake } = stamped(playing(60 * SECOND));

	fake.advanceMs(5_000);
	fake.fireFrame();

	expect(get(playhead.position)).toBe(65 * SECOND);
});

test('the predicted position holds while paused', () => {
	const { playhead, fake } = stamped(playing(60 * SECOND));
	playhead.stamp({ ...playing(60 * SECOND), paused: true });

	fake.advanceMs(10_000);
	fake.fireFrame();

	expect(get(playhead.position)).toBe(60 * SECOND);
});

// While paused the playhead sends no update, so the finish times move only
// with the window's wall clock: the minute store fires, the finish facts
// recompute, and the clock now is two minutes later.
test('while paused the finish times move with the injected wall clock', () => {
	const { playhead, fake } = stamped({ ...playing(60 * SECOND), paused: true });
	const before = get(playhead.finish);
	expect(before).not.toBeNull();

	fake.advanceWall(2 * 60_000);
	fake.fireMinute();

	const after = get(playhead.finish);
	expect(after?.finishSecs).toBe(before!.finishSecs + 120);
});
