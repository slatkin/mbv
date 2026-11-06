use crate::ui_util::{
    continue_tab_title, fmt_duration_gutter, fmt_duration_hms, fmt_duration_short,
    fmt_publish_date_short, fmt_wall_clock, is_home_icon_title,
};
use time::UtcOffset;

/// The predicate recognises exactly the two labels `continue_tab_title`
/// emits, so the tab painter's Home exception can never fire on a library
/// name (2026-10-09 user rule).
#[test]
fn home_icon_title_matches_only_the_continue_tab_labels() {
    assert!(is_home_icon_title(continue_tab_title(true)));
    assert!(is_home_icon_title(continue_tab_title(false)));
    assert!(!is_home_icon_title("alpha"));
    assert!(!is_home_icon_title(""));
}

/// `fmt_wall_clock` renders the offset's 12-hour wall clock with a padded
/// minute and an upper-case period: the finish-clock format the design
/// canvas paints. Midnight and noon both read 12, in their own period.
#[test]
fn fmt_wall_clock_renders_a_twelve_hour_clock_in_the_offset() {
    // 2026-01-15 11:42:00 UTC, 2026-01-15 13:05:00 UTC, and midnight UTC.
    let (mid_morning, early_afternoon, midnight) = (1768477320, 1768482300, 1768435200);
    assert_eq!(fmt_wall_clock(mid_morning, UtcOffset::UTC), "11:42 AM");
    assert_eq!(fmt_wall_clock(early_afternoon, UtcOffset::UTC), "1:05 PM");
    assert_eq!(fmt_wall_clock(midnight, UtcOffset::UTC), "12:00 AM");
    // One hour ahead of UTC, the same instants read an hour later.
    let plus_one = UtcOffset::from_hms(1, 0, 0).expect("a +1:00:00 offset is valid");
    assert_eq!(fmt_wall_clock(mid_morning, plus_one), "12:42 PM");
}

/// A seconds value beyond `time`'s range saturates to the empty string, as
/// the publish-date helpers do, so a caller's `Option` gate reads no clock.
#[test]
fn fmt_wall_clock_saturates_out_of_range_seconds_to_empty() {
    assert_eq!(fmt_wall_clock(i64::MAX, UtcOffset::UTC), "");
}

// ── fmt_duration_short ───────────────────────────────────────────────────

#[test]
fn fmt_duration_short_minutes_and_seconds() {
    assert_eq!(fmt_duration_short(90), "1:30");
    assert_eq!(fmt_duration_short(3599), "59:59");
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
fn out_of_range_publish_dates_saturate_without_panicking() {
    assert_eq!(fmt_publish_date_short(u64::MAX), "1 Jan");
}
