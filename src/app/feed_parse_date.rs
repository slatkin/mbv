/// Parse an RSS `pubDate` (RFC 2822) or Atom `published`/`updated`
/// (ISO 8601) timestamp into unix seconds UTC; anything else yields None
/// (missing dates sort last in the Feeds tab "All" group).
pub(super) fn parse_pub_date_secs(text: &str) -> Option<u64> {
    let t = text.trim();
    let t = t
        .strip_suffix('Z')
        .or_else(|| t.strip_suffix('z'))
        .unwrap_or(t);
    // ISO dates are Y-M-D based (at least two dashes); RFC 2822 dates are
    // month-name based. A plain `contains('T')` is not enough — "GMT"
    // zones and clock times contain "T"s too.
    if t.matches('-').count() >= 2 {
        parse_iso_date(t)
    } else {
        parse_rfc2822_date(t)
    }
}

/// RFC 2822: "Sat, 09 Aug 2026 12:00:00 +0000" (weekday optional).
fn parse_rfc2822_date(text: &str) -> Option<u64> {
    let rest = text.split_once(',').map(|(_, r)| r).unwrap_or(text).trim();
    let mut parts = rest.split_whitespace();
    let day: u32 = parts.next()?.parse().ok()?;
    let month: u32 = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i64 = parts.next()?.parse().ok()?;
    let mut clock = parts.next()?.split(':');
    let hour: u32 = clock.next()?.parse().ok()?;
    let minute: u32 = clock.next()?.parse().ok()?;
    let second: u32 = clock.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    if year < 1970 || month > 12 || day == 0 || day > 31 || hour > 23 || minute > 59 || second > 60
    {
        return None;
    }
    let offset_secs = parts.next().map(parse_zone_offset).unwrap_or(0);
    Some(
        (days_from_civil(year, month, day) * 86_400
            + hour as i64 * 3600
            + minute as i64 * 60
            + second as i64
            - offset_secs) as u64,
    )
}

/// ISO 8601: "2026-08-09T12:00:00Z" / "…+02:00" / "…+0200", optional
/// fractional seconds.
fn parse_iso_date(text: &str) -> Option<u64> {
    let (date_part, time_part) = text.split_once('T')?;
    let mut dp = date_part.split('-');
    let year: i64 = dp.next()?.parse().ok()?;
    let month: u32 = dp.next()?.parse().ok()?;
    let day: u32 = dp.next()?.parse().ok()?;
    if year < 1970 || month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    let (time, offset_secs) = match time_part.find(['+', '-']) {
        Some(idx) => {
            let (rest, offset) = time_part.split_at(idx);
            (rest, parse_iso_offset(offset)?)
        }
        None => (time_part, 0),
    };
    let mut clock = time.split('.').next()?.split(':');
    let hour: u32 = clock.next()?.parse().ok()?;
    let minute: u32 = clock.next()?.parse().ok()?;
    let second: u32 = clock.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(
        (days_from_civil(year, month, day) * 86_400
            + hour as i64 * 3600
            + minute as i64 * 60
            + second as i64
            - offset_secs) as u64,
    )
}

/// "+02:00" / "-0500" style UTC offsets in seconds (east positive).
fn parse_iso_offset(s: &str) -> Option<i64> {
    let (sign, rest) = s.split_at(1);
    let (hour, minute): (i64, i64) = if let Some((h, m)) = rest.split_once(':') {
        (h.parse().ok()?, m.parse().ok()?)
    } else if rest.len() >= 4 {
        (rest[..2].parse().ok()?, rest[2..4].parse().ok()?)
    } else {
        (rest.parse().ok()?, 0)
    };
    let secs = hour * 3600 + minute * 60;
    if sign == "-" {
        Some(-secs)
    } else {
        Some(secs)
    }
}

/// RFC 2822 numeric ("+0500" / "-0700") and named zones ("GMT", "UTC",
/// "EST", …); unknown names are treated as UTC.
fn parse_zone_offset(s: &str) -> i64 {
    if let Some(hour) = s.strip_prefix('+') {
        let h: i64 = hour[..2].parse().unwrap_or(0);
        let m: i64 = hour.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
        return h * 3600 + m * 60;
    }
    if let Some(hour) = s.strip_prefix('-') {
        let h: i64 = hour[..2].parse().unwrap_or(0);
        let m: i64 = hour.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
        return -(h * 3600 + m * 60);
    }
    match s {
        "GMT" | "UTC" | "UT" | "Z" => 0,
        "EST" => -5 * 3600,
        "EDT" => -4 * 3600,
        "CST" => -6 * 3600,
        "CDT" => -5 * 3600,
        "MST" => -7 * 3600,
        "MDT" => -6 * 3600,
        "PST" => -8 * 3600,
        "PDT" => -7 * 3600,
        _ => 0,
    }
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// civil algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((month + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719_468
}
