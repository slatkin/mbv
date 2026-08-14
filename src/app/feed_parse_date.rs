use std::borrow::Cow;

use time::format_description::well_known::{Iso8601, Rfc2822};

/// Parse an RSS `pubDate` (RFC 2822) or Atom `published`/`updated`
/// (ISO 8601) timestamp into unix seconds UTC; anything else yields None
/// (missing dates sort last in the Feeds tab "All" group).
pub(super) fn parse_pub_date_secs(text: &str) -> Option<u64> {
    let t = text.trim();
    // ISO dates are Y-M-D based (at least two dashes); RFC 2822 dates are
    // month-name based. A plain `contains('T')` is not enough — "GMT"
    // zones and clock times contain "T"s too.
    let dt = if t.matches('-').count() >= 2 {
        // time's Iso8601 accepts "Z" but not lowercase "z"; normalize.
        let t: Cow<'_, str> = match t.strip_suffix('z') {
            Some(rest) => Cow::Owned(format!("{rest}Z")),
            None => Cow::Borrowed(t),
        };
        time::OffsetDateTime::parse(&t, &Iso8601::DEFAULT).ok()
    } else {
        time::OffsetDateTime::parse(t, &Rfc2822).ok()
    }?;
    if dt.year() < 1970 {
        return None;
    }
    Some(dt.unix_timestamp() as u64)
}
