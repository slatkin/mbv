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

/** A duration as `H:MM:SS` at and over an hour, `M:SS` under one — a port
 * of `mbv_ui_model::ui_util::fmt_duration_short`, the TUI's shared duration
 * formatter (gui-change-driven-sections design D5). Seconds are whole; a
 * negative value is not reachable — the playhead clamps the position into
 * the runtime before it divides. */
export function formatDurationShort(secs: number): string {
	if (secs >= 3600) {
		return `${Math.floor(secs / 3600)}:${String(Math.floor((secs % 3600) / 60)).padStart(2, '0')}:${String(Math.floor(secs % 60)).padStart(2, '0')}`;
	}
	return `${Math.floor(secs / 60)}:${String(Math.floor(secs % 60)).padStart(2, '0')}`;
}

/** The wall clock at `unixSecs` as a zero-padded 24-hour `HH:MM`, in the
 * window's local zone — a port of `mbv_ui_model::ui_util::fmt_wall_clock`
 * (design D5). The webview always knows the local zone, so the window's
 * finish clocks need no offset from the core. */
export function formatWallClock(unixSecs: number): string {
	const at = new Date(unixSecs * 1000);
	const hours = String(at.getHours()).padStart(2, '0');
	const minutes = String(at.getMinutes()).padStart(2, '0');
	return `${hours}:${minutes}`;
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
