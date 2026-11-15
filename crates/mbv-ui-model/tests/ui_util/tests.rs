use mbv_ui_model::ui_util::row_date;

/// A provider timestamp inside `i64` but outside `time`'s supported range
/// formats as the empty string, so the row-date helper reports no date and
/// the list reserves no gutter for it.
#[test]
fn an_unformattable_timestamp_yields_no_date() {
    assert_eq!(row_date(Some(10_000_000_000_000)), None);
}
