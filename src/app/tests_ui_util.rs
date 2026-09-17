use super::ui_util::{fmt_duration_hms, fmt_duration_short};

// ── fmt_duration_short ───────────────────────────────────────────────────

#[test]
fn fmt_duration_short_zero() {
    assert_eq!(fmt_duration_short(0), "0:00");
}

#[test]
fn fmt_duration_short_seconds_only() {
    assert_eq!(fmt_duration_short(45), "0:45");
}

#[test]
fn fmt_duration_short_minutes_and_seconds() {
    assert_eq!(fmt_duration_short(90), "1:30");
    assert_eq!(fmt_duration_short(3599), "59:59");
}

#[test]
fn fmt_duration_short_first_component_unpadded() {
    assert_eq!(fmt_duration_short(65), "1:05");
    assert_eq!(fmt_duration_short(605), "10:05");
}

#[test]
fn fmt_duration_short_hours() {
    assert_eq!(fmt_duration_short(3600), "1:00:00");
    assert_eq!(fmt_duration_short(3661), "1:01:01");
    assert_eq!(fmt_duration_short(7384), "2:03:04");
    assert_eq!(fmt_duration_short(7322), "2:02:02");
}

// ── fmt_duration_hms ─────────────────────────────────────────────────────

#[test]
fn fmt_duration_hms_padded_minutes_under_an_hour() {
    assert_eq!(fmt_duration_hms(0), "00:00");
    assert_eq!(fmt_duration_hms(45), "00:45");
    assert_eq!(fmt_duration_hms(185), "03:05");
    assert_eq!(fmt_duration_hms(3599), "59:59");
}

#[test]
fn fmt_duration_hms_hours() {
    assert_eq!(fmt_duration_hms(3600), "01:00:00");
    assert_eq!(fmt_duration_hms(3661), "01:01:01");
    assert_eq!(fmt_duration_hms(45296), "12:34:56");
}
