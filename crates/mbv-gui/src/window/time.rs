//! The window's time helpers (D10 of `replace-tauri-with-blitz`): the port
//! of `lib/bridge/time.ts` (align-gui-with-design-canvas U1.5). The design
//! canvas paints U+2212, not the ASCII hyphen-minus, before every remaining
//! time. The durations and wall clocks are the TUI's shared formatters in
//! `mbv-ui-model`, which the TypeScript module ported; the window calls them
//! directly, so the two Clients cannot disagree on a clock's shape.

use std::sync::OnceLock;

use time::UtcOffset;

/// `value` as a remaining time: the canvas's U+2212 minus before the clock
/// text it already carries (a duration, a compact minute count, or a clock).
/// Callers pass the painted time as the frame carries it; the helper adds no
/// arithmetic and no unit.
#[must_use]
pub fn minus(value: &str) -> String {
    format!("\u{2212}{value}")
}

/// A duration as `H:MM:SS` at and over an hour, `M:SS` under one — the TUI's
/// shared duration formatter (`mbv_ui_model::ui_util::fmt_duration_short`),
/// which the TypeScript module ported. Seconds are whole; a negative value is
/// not reachable — the playhead clamps the position into the runtime before
/// it divides.
#[must_use]
pub fn format_duration_short(secs: i64) -> String {
    mbv_ui_model::ui_util::fmt_duration_short(secs)
}

/// The wall clock at `unix_secs` as a zero-padded 24-hour `HH:MM`, in the
/// window's local zone — the TUI's shared `fmt_wall_clock` (D10). The window
/// always knows its zone, so the finish clocks need no offset from the core.
#[must_use]
pub fn format_wall_clock(unix_secs: i64) -> String {
    mbv_ui_model::ui_util::fmt_wall_clock(unix_secs, local_offset())
}

/// The window's local UTC offset, resolved once per process: the lookup
/// fails once other threads exist, so the first caller's answer stands, as
/// the app log's resolver does. A failed lookup falls back to UTC.
#[must_use]
pub fn local_offset() -> UtcOffset {
    static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();
    *LOCAL_OFFSET.get_or_init(|| UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC))
}

/// A fraction `ticks / runtime`, clamped to `[0, 1]`; zero runtime is zero.
/// The progress shapes the panels paint (the played fill, the ring's arc,
/// the chapter and mark positions) all read the one rule.
#[must_use]
pub fn clamp_ratio(ticks: f64, runtime: f64) -> f64 {
    if runtime <= 0.0 {
        return 0.0;
    }
    (ticks / runtime).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests;
