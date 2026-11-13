//! Tests of the window's time helpers (D16): the contracts of `time.test.ts`
//! that the window owns — the canvas minus sign, and the wall clock's
//! zero-padding in the local zone. The duration table is the TUI's shared
//! formatter's contract, which `mbv-ui-model`'s own tests own; the window
//! calls it directly, so a second table here would test the same function
//! twice.

use time::{Date, Month, Time};

use super::{format_wall_clock, local_offset, minus};

/// The local-zone epoch of the given wall-clock components, as a fixture
/// builder: the assertion holds in every zone, as the TypeScript fixture's
/// does.
fn local_unix_secs(year: i32, month: Month, day: u8, hour: u8, minute: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).expect("the fixture date is valid");
    let time = Time::from_hms(hour, minute, 0).expect("the fixture time is valid");
    time::PrimitiveDateTime::new(date, time)
        .assume_offset(local_offset())
        .unix_timestamp()
}

/// A remaining time carries the canvas minus sign, and no ASCII hyphen.
#[test]
fn a_remaining_time_carries_the_canvas_minus_sign() {
    assert_eq!(minus("22:14"), "\u{2212}22:14");
    assert!(!minus("22m").contains('-'));
}

/// The wall clock zero-pads both fields in the local zone.
#[test]
fn a_wall_clock_zero_pads_to_hh_mm() {
    let early = local_unix_secs(2026, Month::November, 12, 7, 5);
    let night = local_unix_secs(2026, Month::November, 12, 21, 43);

    assert_eq!(format_wall_clock(early), "07:05");
    assert_eq!(format_wall_clock(night), "21:43");
}
