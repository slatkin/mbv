use time::{OffsetDateTime, format_description};

const TIMESTAMP_FORMAT: &[format_description::FormatItem<'_>] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]"
);

pub(crate) fn format_ts(datetime: OffsetDateTime) -> String {
    datetime
        .format(TIMESTAMP_FORMAT)
        .expect("OffsetDateTime should support the fixed timestamp format")
}

pub(crate) fn now_local() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

#[cfg(test)]
mod tests {
    use super::format_ts;
    use time::macros::datetime;

    #[test]
    fn timestamp_formats_milliseconds_and_negative_offset() {
        assert_eq!(
            format_ts(datetime!(2026-09-28 14:03:12.345 -03:30)),
            "2026-09-28T14:03:12.345-03:30"
        );
    }
}
