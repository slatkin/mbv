const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Parses a leading `YYYY-MM-DD` date out of `date_str` (an Emby date field,
/// which may carry a `T...` time/offset suffix that's ignored here) and
/// returns its `(year, month name, day)`, or `None` if it doesn't parse.
fn parse_ymd(date_str: &str) -> Option<(&str, &'static str, u32)> {
    let date_part = date_str.split('T').next().unwrap_or(date_str);
    let parts: Vec<&str> = date_part.splitn(3, '-').collect();
    let [y, m, d] = parts.as_slice() else {
        return None;
    };
    let day: u32 = d.parse().ok()?;
    let month_idx: usize = m.parse::<usize>().ok()?.checked_sub(1)?;
    Some((y, MONTHS.get(month_idx)?, day))
}

/// Formats an Emby `PremiereDate` value (e.g. `2015-06-19T00:00:00.0000000Z`)
/// as a release date like "19 Jun 2015".
pub(in crate::app::render) fn format_release_date(premiere_date: &str) -> String {
    parse_ymd(premiere_date)
        .map(|(y, month, d)| format!("{d} {} {y}", &month[..3]))
        .unwrap_or_else(|| premiere_date.to_string())
}
