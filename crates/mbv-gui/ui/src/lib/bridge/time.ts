// The GUI's time-sign convention (align-gui-with-design-canvas U1.5): the
// design canvas paints U+2212, not the ASCII hyphen-minus, before every
// remaining time. The scrubber's right span paints through `minus`, so the
// glyph's one home stays one edit. (The collapsed rail paints a literal `-`
// until the collapsed-states work picks the convention up.)

/** `value` as a remaining time: the canvas's U+2212 minus before the clock
 * text it already carries (a duration, a compact minute count, or a clock).
 * Callers pass the painted time as the frame carries it; the helper adds no
 * arithmetic and no unit. */
export function minus(value: string): string {
	return `\u2212${value}`;
}

/** A fraction `ticks / runtime`, clamped to `[0, 1]`; zero runtime is zero.
 * The progress shapes the panels paint (the played fill, the ring's arc, the
 * chapter and mark positions) all read the one rule. */
export function clampRatio(ticks: number, runtime: number): number {
	if (runtime <= 0) {
		return 0;
	}
	return Math.min(1, Math.max(0, ticks / runtime));
}
