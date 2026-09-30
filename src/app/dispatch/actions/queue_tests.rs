use crate::app::App;
use rstest::rstest;

// ── remote_seek_ticks: asymmetric clamp (rewind only) ───────────────────

#[rstest]
#[case::remote_seek_rewind_clamps_at_zero(3, -5.0, 0)]
fn remote_seek(#[case] position: i64, #[case] delta: f64, #[case] expected: i64) {
    assert_eq!(App::remote_seek_ticks(position, delta), expected);
}

// ── next_subtitle_entry: shared cycling math (remote/local parity, #86) ─

#[test]
fn next_subtitle_entry_advances_from_off() {
    assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 0), 5);
}

#[test]
fn next_subtitle_entry_wraps_from_last_back_to_off() {
    assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 7), 0);
}

#[test]
fn next_subtitle_entry_empty_returns_current_unchanged() {
    assert_eq!(App::next_subtitle_entry(&[], 3), 3);
}

#[test]
fn next_subtitle_entry_matches_remote_sentinel_convention() {
    // Remote sessions use -1 as the "off" sentinel (vs. 0 for local
    // playback) -- same wraparound math, different sentinel value.
    assert_eq!(App::next_subtitle_entry(&[-1, 2, 4], -1), 2);
    assert_eq!(App::next_subtitle_entry(&[-1, 2, 4], 4), -1);
}

// ── cycle_sub: local branch (#86 unification + idle fallback) ───────────
