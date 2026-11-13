// The window's playhead (gui-change-driven-sections design D5): the position
// prediction from the `playhead` anchor section. The core sends one anchor —
// the position, runtime, paused state, and active slot at its send — on a
// changed fact or a sample that drifted past the tolerance; between anchors
// the window advances the position itself: the predicted position is
// `anchor.positionTicks + (now − receipt stamp)` while playing, held while
// paused. Everything that follows the position or the wall clock derives
// here: the scrubber's played fraction and clocks, the transport's finish
// time, the queue's finish clock (with its ` +1d` suffix), and each queue
// row's end.
//
// The core and the window share no clock, so the window stamps each anchor
// with `performance.now()` when it receives it; the IPC delay is the only
// error, and the next anchor corrects it. The clocks and the schedulers are
// injectable, so the tests drive time without a real wait.

import { derived, get, writable } from 'svelte/store';
import type { Readable } from 'svelte/store';
import { clampRatio, formatDurationShort, formatWallClock } from './time';
import type { GuiPlayhead } from './types';

/** One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`). */
const TICKS_PER_SECOND = 10_000_000;

/** One millisecond in Emby ticks: the elapsed monotonic time the prediction
 * reads is milliseconds, the position is ticks. */
const TICKS_PER_MS = TICKS_PER_SECOND / 1000;

/** One wall-clock minute, in milliseconds. */
const MINUTE_MS = 60_000;

/** The clocks and schedulers the playhead reads. The defaults are the
 * window's own; the tests install recorders, so no test waits. */
export interface PlayheadClocks {
	/** Monotonic milliseconds: `performance.now()` in the window. */
	nowMs(): number;
	/** The wall clock now. The local zone the finish clocks paint comes from
	 * it — the webview always knows its zone. */
	wallNow(): Date;
	/** Schedule one animation-frame step; returns its handle. */
	scheduleFrame(step: () => void): number;
	/** Cancel a scheduled frame step by its handle. */
	cancelFrame(handle: number): void;
	/** Schedule one timeout; returns its handle. */
	scheduleTimeout(step: () => void, delayMs: number): number;
	/** Cancel a scheduled timeout by its handle. */
	cancelTimeout(handle: number): void;
}

/** The window's own clocks. */
const realClocks: PlayheadClocks = {
	nowMs: () => performance.now(),
	wallNow: () => new Date(),
	scheduleFrame: (step) => requestAnimationFrame(step),
	cancelFrame: (handle) => cancelAnimationFrame(handle),
	scheduleTimeout: (step, delayMs) => window.setTimeout(step, delayMs),
	cancelTimeout: (handle) => window.clearTimeout(handle),
};

/** The wall-clock facts the finish derivations read (design D5). */
export interface PlayheadFinish {
	/** The wall clock now, in unix seconds: the ` +1d` compare's today. */
	nowSecs: number;
	/** The active item's finish epoch, in unix seconds: the clock now plus
	 * the item's remaining time. The queue finish and each row end add their
	 * offset to it. */
	finishSecs: number;
}

/** The playhead stores one window keeps (design D5). The window's instance
 * is `playhead`; the tests build their own over injected clocks with
 * `createPlayhead`. */
export interface Playhead {
	/** Stamp the anchor the batch carried: it becomes the prediction's base,
	 * stamped with the clock at receipt. */
	stamp(anchor: GuiPlayhead): void;
	/** The stamped anchor, `null` before the first batch and after `reset`. */
	readonly anchor: Readable<GuiPlayhead | null>;
	/** The monotonic clock the prediction reads, moved by a
	 * `requestAnimationFrame` loop only while the anchor is active and not
	 * paused (design D5). */
	readonly now: Readable<number>;
	/** The wall clock's minute counter: one `setTimeout` re-arms to the next
	 * minute boundary, so the finish times move while playback is paused,
	 * with no update from the core. */
	readonly minute: Readable<number>;
	/** The predicted position, clamped into the runtime, in ticks. */
	readonly position: Readable<number>;
	/** The active item's runtime, in ticks; `0` while unknown. */
	readonly runtime: Readable<number>;
	/** The played fraction of the active item, clamped to `[0, 1]`. */
	readonly fraction: Readable<number>;
	/** The elapsed time, as the TUI's shared duration formatter paints it. */
	readonly elapsed: Readable<string>;
	/** The time left in the active item, in the same format. The panels
	 * paint the minus sign before it. */
	readonly remaining: Readable<string>;
	/** The wall-clock facts the finish derivations read, `null` while no
	 * anchor holds or the runtime is unknown. */
	readonly finish: Readable<PlayheadFinish | null>;
	/** Drop the anchor and stop the clocks. Production never calls it: a
	 * window keeps its playhead for its life, and a test resets between
	 * cases. */
	reset(): void;
	/** Swap the clocks for test clocks. Pending schedulers are dropped with
	 * the anchor first, so the next `stamp` arms over the new clocks
	 * (tests only). */
	useClocks(clocks: PlayheadClocks): void;
}

/** Create the playhead stores over `clocks`. */
export function createPlayhead(clocks: PlayheadClocks): Playhead {
	// The clocks are a field, not a closure capture: `useClocks` swaps them
	// and the running loops read the swap.
	let activeClocks = clocks;
	const anchorStore = writable<GuiPlayhead | null>(null);
	const nowStore = writable(0);
	const minuteStore = writable(0);
	let stampMs = 0;
	let frameHandle: number | undefined;
	let minuteHandle: number | undefined;

	/** Start the animation-frame loop while the anchor is active and not
	 * paused; stop it otherwise (design D5). */
	function pump(): void {
		const anchor = get(anchorStore);
		const shouldRun = anchor !== null && anchor.active && !anchor.paused;
		if (shouldRun && frameHandle === undefined) {
			const step = () => {
				nowStore.set(activeClocks.nowMs());
				frameHandle = activeClocks.scheduleFrame(step);
			};
			frameHandle = activeClocks.scheduleFrame(step);
		} else if (!shouldRun && frameHandle !== undefined) {
			activeClocks.cancelFrame(frameHandle);
			frameHandle = undefined;
		}
	}

	/** Arm the one-minute timeout, once: the firing re-arms itself. */
	function scheduleMinute(): void {
		if (minuteHandle !== undefined) {
			return;
		}
		const delay = MINUTE_MS - (activeClocks.wallNow().getTime() % MINUTE_MS) + 1;
		minuteHandle = activeClocks.scheduleTimeout(() => {
			minuteHandle = undefined;
			minuteStore.update((count) => count + 1);
			scheduleMinute();
		}, delay);
	}

	const anchor: Readable<GuiPlayhead | null> = anchorStore;
	const now: Readable<number> = nowStore;
	const minute: Readable<number> = minuteStore;

	/** The predicted position: the anchor's position advanced by the elapsed
	 * monotonic time while playing, held while paused, clamped into the
	 * runtime. */
	const position = derived([anchor, now], ([value, at]) => {
		if (value === null) {
			return 0;
		}
		const elapsed = value.paused ? 0 : Math.max(0, at - stampMs);
		const predicted = value.positionTicks + elapsed * TICKS_PER_MS;
		return Math.min(Math.max(predicted, 0), Math.max(value.runtimeTicks, 0));
	});

	const runtime = derived(anchor, (value) => value?.runtimeTicks ?? 0);

	const fraction = derived([position, runtime], ([at, total]) => clampRatio(at, total));

	const elapsed = derived(position, (at) => formatDurationShort(Math.floor(at / TICKS_PER_SECOND)));

	const remaining = derived([position, runtime], ([at, total]) =>
		formatDurationShort(Math.floor(Math.max(0, total - at) / TICKS_PER_SECOND)),
	);

	// The finish facts recompute with the minute store, so a paused window's
	// times move with the wall clock, and with the position, whose remaining
	// second moves them while playing. The cached value holds the store to
	// one notification per changed fact: the subscribers paint clocks at
	// minute precision, so a per-frame recompute must not re-render them.
	let lastFinish: PlayheadFinish | null = null;
	const finish = derived([anchor, position, minute], ([value, at]) => {
		if (value === null || value.runtimeTicks <= 0) {
			lastFinish = null;
			return null;
		}
		const remainingSecs = Math.floor(Math.max(0, value.runtimeTicks - at) / TICKS_PER_SECOND);
		const nowSecs = Math.floor(activeClocks.wallNow().getTime() / 1000);
		const next = { nowSecs, finishSecs: nowSecs + remainingSecs };
		if (
			lastFinish === null ||
			next.nowSecs !== lastFinish.nowSecs ||
			next.finishSecs !== lastFinish.finishSecs
		) {
			lastFinish = next;
		}
		return lastFinish;
	});

	return {
		stamp(value) {
			stampMs = activeClocks.nowMs();
			anchorStore.set(value);
			// The prediction starts at the stamp, not at the previous `now`
			// value: a stale one would jump the position by the gap.
			nowStore.set(stampMs);
			pump();
			scheduleMinute();
		},
		anchor,
		now,
		minute,
		position,
		runtime,
		fraction,
		elapsed,
		remaining,
		finish,
		reset() {
			if (frameHandle !== undefined) {
				activeClocks.cancelFrame(frameHandle);
				frameHandle = undefined;
			}
			if (minuteHandle !== undefined) {
				activeClocks.cancelTimeout(minuteHandle);
				minuteHandle = undefined;
			}
			anchorStore.set(null);
		},
		useClocks(next) {
			activeClocks = next;
		},
	};
}

/** The window's playhead. `sections.ts` stamps it on each carried `playhead`
 * section; the panels read the derived stores. */
export const playhead: Playhead = createPlayhead(realClocks);

/** Swap the window playhead's clocks for test clocks. Pending schedulers are
 * dropped with the anchor, so a test stamps a fresh playhead over the new
 * clocks (tests only). */
export function setPlayheadClocksForTest(clocks: PlayheadClocks): void {
	playhead.reset();
	playhead.useClocks(clocks);
}

/** The queue's finish clock (design D5): the wall clock at `tailSecs` past
 * the active item's end, with the ` +1d` suffix when the finish lands past
 * the local day of the clock now — the rule `queue_finish_clock` runs in
 * Rust for the TUI (`src/app/queue_summary_projection.rs`). `null` while the
 * window has no finish facts or the summary carries no tail. */
export function queueFinishText(
	finish: PlayheadFinish | null,
	tailSecs: number | null,
): string | null {
	if (finish === null || tailSecs === null) {
		return null;
	}
	const queueSecs = finish.finishSecs + tailSecs;
	const clock = formatWallClock(queueSecs);
	if (localDay(queueSecs) > localDay(finish.nowSecs)) {
		return `${clock} +1d`;
	}
	return clock;
}

/** One row's "Ends at" clock (design D5): the wall clock at the row's
 * offset past the active item's end. The row ends paint the bare clock, as
 * the transport's finish clock does. */
export function rowEndText(
	finish: PlayheadFinish | null,
	endsAfterSecs: number | null,
): string | null {
	if (finish === null || endsAfterSecs === null) {
		return null;
	}
	return formatWallClock(finish.finishSecs + endsAfterSecs);
}

/** The local day `unixSecs` falls on, as a comparable number: year, month,
 * and day in the window's zone. */
function localDay(unixSecs: number): number {
	const at = new Date(unixSecs * 1000);
	return at.getFullYear() * 10_000 + (at.getMonth() + 1) * 100 + at.getDate();
}
