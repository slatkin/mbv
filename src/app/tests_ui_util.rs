use super::ui_util::fmt_duration;

// ── fmt_duration ─────────────────────────────────────────────────────────

#[test]
fn fmt_duration_zero() {
    assert_eq!(fmt_duration(0), "0:00");
}

#[test]
fn fmt_duration_seconds_only() {
    assert_eq!(fmt_duration(45), "0:45");
}

#[test]
fn fmt_duration_minutes_and_seconds() {
    assert_eq!(fmt_duration(90), "1:30");
    assert_eq!(fmt_duration(3599), "59:59");
}

#[test]
fn fmt_duration_hours() {
    assert_eq!(fmt_duration(3600), "1:00:00");
    assert_eq!(fmt_duration(3661), "1:01:01");
    assert_eq!(fmt_duration(7384), "2:03:04");
}
