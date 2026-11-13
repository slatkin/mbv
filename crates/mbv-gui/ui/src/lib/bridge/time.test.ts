// The GUI's time-sign convention (align-gui-with-design-canvas U1.5) and the
// playhead's shared formatters (gui-change-driven-sections design D5): the
// remaining-time glyph is the canvas's U+2212, the durations paint the TUI's
// shared `M:SS` / `H:MM:SS` forms, and the wall clocks paint zero-padded
// `HH:MM` in the window's local zone.

import { expect, test } from 'vitest';
import { formatDurationShort, formatWallClock, minus } from './time';

test('a remaining time carries the canvas minus sign', () => {
	expect(minus('22:14')).toBe('\u221222:14');
	expect(minus('22m')).not.toContain('-');
});

// One table for the two forms `fmt_duration_short` paints: `M:SS` under an
// hour, `H:MM:SS` at and over one (design D5's Rust port).
test.each([
	['0:00', 0],
	['45:05', 45 * 60 + 5],
	['59:59', 3599],
	['1:01:01', 3661],
	['12:34:56', 12 * 3600 + 34 * 60 + 56],
])('a duration paints the TUI\'s shared form as %s from %s seconds', (expected, secs) => {
	expect(formatDurationShort(secs)).toBe(expected);
});

// The wall clock zero-pads both fields in the local zone: the fixture builds
// its epoch from local components, so the assertion holds in every zone.
test('a wall clock zero-pads to HH:MM', () => {
	const early = new Date(2026, 10, 12, 7, 5);
	const night = new Date(2026, 10, 12, 21, 43);

	expect(formatWallClock(Math.floor(early.getTime() / 1000))).toBe('07:05');
	expect(formatWallClock(Math.floor(night.getTime() / 1000))).toBe('21:43');
});
