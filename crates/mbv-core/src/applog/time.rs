use std::sync::OnceLock;

use time::{OffsetDateTime, UtcOffset, format_description};

const TIMESTAMP_FORMAT: &[format_description::FormatItem<'_>] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]"
);

pub(crate) fn format_ts(datetime: OffsetDateTime) -> String {
    datetime
        .format(TIMESTAMP_FORMAT)
        .expect("OffsetDateTime should support the fixed timestamp format")
}

static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();

/// Resolves the local UTC offset once. Must run before other threads spawn:
/// the lookup fails once other threads exist. Falls
/// back to UTC when the lookup fails.
pub(crate) fn init_local_offset() {
    LOCAL_OFFSET.get_or_init(|| UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC));
}

pub(crate) fn now_local() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(LOCAL_OFFSET.get().copied().unwrap_or(UtcOffset::UTC))
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
