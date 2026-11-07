// The panel's time formats (design D4): the scrubber's clocks, the rail's
// remaining time, and the compact strip's clock all read the same TUI
// formatter, so the three panel states cannot disagree about a duration.

/** Emby ticks in one second (`mbv_emby_model::TICKS_PER_SECOND`). */
export const TICKS_PER_SECOND = 10_000_000;

/** `fmt_duration_short`: `M:SS` under an hour, `H:MM:SS` at or over it. */
export function fmtDurationShort(ticks: number): string {
	const s = Math.trunc(ticks / TICKS_PER_SECOND);
	if (s >= 3600) {
		const mm = String(Math.trunc((s % 3600) / 60)).padStart(2, '0');
		const ss = String(s % 60).padStart(2, '0');
		return `${Math.trunc(s / 3600)}:${mm}:${ss}`;
	}
	return `${Math.trunc(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
