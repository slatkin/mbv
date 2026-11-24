//! One authority parser for the `http(s)` feed URLs the crate reads: the
//! host `FeedPlatform::of_url` rules on and `normalize_feed_url` rewrites
//! against. One parser, so the two sites cannot disagree on a URL again
//! (issue #1040).

/// The host and the path with query of an `http(s)` URL with a path, or
/// `None` when the URL has no such scheme or no path.
///
/// The authority ends at the first `/`; a `?` or `#` before that `/` means
/// the URL has no path and answers `None`, so a query or fragment cannot
/// smuggle an `@`-form host into the authority. The host is the authority's
/// part after any userinfo and before any port, so
/// `https://user:pass@www.youtube.com/feed` reads `www.youtube.com` and
/// `https://youtube.com@evil.example/feed` reads `evil.example`.
pub(crate) fn host_and_path(url: &str) -> Option<(&str, &str)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let slash = rest.find('/')?;
    if rest[..slash].contains(['?', '#']) {
        return None;
    }
    let authority = &rest[..slash];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split_once(':').map_or(host, |(host, _)| host);
    Some((host, &rest[slash..]))
}

/// The path and the query of `path_and_query`: the string splits at its first
/// `?`, and a path with no query keeps an empty one.
pub(crate) fn split_query(path_and_query: &str) -> (&str, &str) {
    path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""))
}
