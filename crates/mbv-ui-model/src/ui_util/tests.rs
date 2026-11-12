use crate::ui_util::{
    SUBTITLE_MODES, continue_tab_title, fmt_duration_gutter, fmt_duration_hms, fmt_duration_short,
    fmt_publish_date_short, fmt_wall_clock, is_home_icon_title, next_subtitle_mode,
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

/// `fmt_wall_clock` renders the offset's zero-padded 24-hour wall clock
/// with a padded hour and minute: the finish-clock format the design canvas
/// paints. Midnight reads `00:00` and the afternoon keeps its own hour.
#[test]
fn fmt_wall_clock_renders_a_zero_padded_twenty_four_hour_clock_in_the_offset() {
    // 2026-01-15 11:42:00 UTC, 2026-01-15 13:05:00 UTC, and midnight UTC.
    let (mid_morning, early_afternoon, midnight) = (1_768_477_320, 1_768_482_300, 1_768_435_200);
    assert_eq!(fmt_wall_clock(mid_morning, UtcOffset::UTC), "11:42");
    assert_eq!(fmt_wall_clock(early_afternoon, UtcOffset::UTC), "13:05");
    assert_eq!(fmt_wall_clock(midnight, UtcOffset::UTC), "00:00");
    // One hour ahead of UTC, the same instants read an hour later.
    let plus_one = UtcOffset::from_hms(1, 0, 0).expect("a +1:00:00 offset is valid");
    assert_eq!(fmt_wall_clock(mid_morning, plus_one), "12:42");
}

/// A seconds value beyond `time`'s range saturates to the empty string, as
/// the publish-date helpers do, so a caller's `Option` gate reads no clock.
#[test]
fn fmt_wall_clock_saturates_out_of_range_seconds_to_empty() {
    assert_eq!(fmt_wall_clock(i64::MAX, UtcOffset::UTC), "");
}

/// The mode list is the cycle's order (row 9.1, design D13): the GUI's pick
/// paints the modes `next_subtitle_mode` steps through, and the last mode
/// wraps back to the first. An empty mode still reads as `Always`, the step
/// after the default, and an undeclared mode steps to `Default`.
#[test]
fn next_subtitle_mode_walks_the_mode_list_in_order_and_wraps() {
    assert_eq!(
        SUBTITLE_MODES,
        [
            "Default",
            "Always",
            "Smart",
            "OnlyForced",
            "None",
            "HearingImpaired"
        ]
    );
    assert_eq!(next_subtitle_mode("Default"), "Always");
    assert_eq!(next_subtitle_mode("Always"), "Smart");
    assert_eq!(next_subtitle_mode("Smart"), "OnlyForced");
    assert_eq!(next_subtitle_mode("OnlyForced"), "None");
    assert_eq!(next_subtitle_mode("None"), "HearingImpaired");
    assert_eq!(next_subtitle_mode("HearingImpaired"), "Default");
    assert_eq!(next_subtitle_mode(""), "Always");
    assert_eq!(next_subtitle_mode("Nowhere"), "Default");
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
