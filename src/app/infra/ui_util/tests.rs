use crate::app::infra::ui_util::{
    fmt_duration_gutter, fmt_duration_hms, fmt_duration_short, fmt_publish_date,
    fmt_publish_date_short,
};

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

#[test]
fn gutter_duration_uses_minutes_precision_and_fits_six_columns() {
    for (seconds, expected) in [(0, "0:00"), (90, "1:30"), (3599, "59:59")] {
        let formatted = fmt_duration_gutter(seconds);
        assert_eq!(formatted, expected);
        assert!(unicode_width::UnicodeWidthStr::width(formatted.as_str()) <= 6);
    }
    for (seconds, expected) in [(3600, "1:00"), (3661, "1:01"), (360_000, "100:00")] {
        let formatted = fmt_duration_gutter(seconds);
        assert_eq!(formatted, expected);
        assert!(unicode_width::UnicodeWidthStr::width(formatted.as_str()) <= 6);
    }
    assert!(fmt_duration_gutter(i64::MAX).len() <= 6);
}

/// The row gutter's format: day and abbreviated month, so the fixed
/// six-column gutter stays narrow (the hero's meta row keeps the year).
#[test]
fn short_publish_date_is_the_row_gutter_format() {
    // 2026-09-17T00:00:00Z.
    let secs = 1_789_603_200;
    assert_eq!(fmt_publish_date_short(secs), "17 Sep");
    assert_eq!(fmt_publish_date(secs), "17 Sep 2026");
    // A single-digit day stays unpadded: the gutter right-aligns it.
    assert_eq!(fmt_publish_date_short(secs - 14 * 86_400), "3 Sep");
}

/// A nonsense timestamp saturates to the epoch rather than panicking:
/// the formatter's `i64` conversion falls back to 0. The gutter is never
/// left blank or half-painted, and the row keeps its column.
#[test]
fn out_of_range_publish_dates_saturate_without_panicking() {
    assert_eq!(fmt_publish_date_short(u64::MAX), "1 Jan");
}
