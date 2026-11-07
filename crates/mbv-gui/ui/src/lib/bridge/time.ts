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
