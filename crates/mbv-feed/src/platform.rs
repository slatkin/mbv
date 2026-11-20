//! The known service that serves a Feed subscription, read from its URL alone.

/// A known service that serves a feed.
///
/// The platform comes from the subscription URL alone, so a stored copy cannot
/// go stale. A feed on a host that no rule names is generic and has no
/// platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedPlatform {
    /// A `YouTube` channel feed.
    YouTube,
    /// A `SoundCloud` user feed.
    SoundCloud,
    /// A `Spreaker` show feed.
    Spreaker,
    /// An `Audioboom` channel feed.
    Audioboom,
    /// A `Patreon` creator feed.
    Patreon,
    /// A `Substack` publication feed.
    Substack,
    /// A `Spotify` for Creators (`anchor.fm`) podcast feed.
    Spotify,
    /// An `Odysee` channel feed.
    Odysee,
    /// A `PeerTube` instance, channel, or playlist feed.
    PeerTube,
    /// An `Internet Archive` collection feed.
    InternetArchive,
}

/// The host domains of the nine platforms that serve feeds from a shared host,
/// paired with their platform. `PeerTube` is absent: every instance has its own
/// host, so it needs the path rule.
const HOST_DOMAINS: [(&str, FeedPlatform); 9] = [
    ("youtube.com", FeedPlatform::YouTube),
    ("soundcloud.com", FeedPlatform::SoundCloud),
    ("spreaker.com", FeedPlatform::Spreaker),
    ("audioboom.com", FeedPlatform::Audioboom),
    ("patreon.com", FeedPlatform::Patreon),
    ("substack.com", FeedPlatform::Substack),
    ("anchor.fm", FeedPlatform::Spotify),
    ("odysee.com", FeedPlatform::Odysee),
    ("archive.org", FeedPlatform::InternetArchive),
];

/// The paths `PeerTube` serves feeds on (`server/core/controllers/feeds/`).
const PEER_TUBE_PATHS: [&str; 5] = [
    "/feeds/videos.xml",
    "/feeds/videos.rss",
    "/feeds/videos.atom",
    "/feeds/videos.json",
    "/feeds/podcast/videos.xml",
];

/// The query parameters that narrow a `PeerTube` feed to a channel or a playlist
/// (`server/core/middlewares/validators/feeds.ts`). An instance-wide feed has
/// none of them.
const PEER_TUBE_FILTERS: [&str; 5] = [
    "accountId",
    "accountName",
    "videoChannelId",
    "videoChannelName",
    "playlistId",
];

impl FeedPlatform {
    /// The platform `url` shows, or `None` for a generic feed.
    ///
    /// The host rule runs first: a host matches a domain when it equals the
    /// domain or ends with `.` plus the domain, so `feeds.soundcloud.com` is
    /// `SoundCloud` and `notyoutube.com` is not `YouTube`. `PeerTube` has no
    /// shared host, so its rule reads the feed path and a filter parameter
    /// instead.
    #[must_use]
    pub fn of_url(url: &str) -> Option<Self> {
        let (host, path_and_query) = host_and_path(url)?;
        let host = host.to_ascii_lowercase();
        if let Some(platform) = host_platform(&host) {
            return Some(platform);
        }
        peer_tube_feed(path_and_query).then_some(Self::PeerTube)
    }
}

/// The host and the path with query of an `http(s)` URL, or `None` when the URL
/// has no such scheme or no path.
///
/// The host is the authority's part after any userinfo and before any port, so
/// `https://user:pass@www.youtube.com/feed` reads `www.youtube.com` and
/// `https://youtube.com@evil.example/feed` reads `evil.example`.
fn host_and_path(url: &str) -> Option<(&str, &str)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let slash = rest.find('/')?;
    let authority = &rest[..slash];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split_once(':').map_or(host, |(host, _)| host);
    Some((host, &rest[slash..]))
}

/// The platform that owns `host` by its domain, or `None`.
fn host_platform(host: &str) -> Option<FeedPlatform> {
    HOST_DOMAINS
        .into_iter()
        .find_map(|(domain, platform)| host_matches(host, domain).then_some(platform))
}

/// Whether `host` is `domain` itself or a subdomain of it.
fn host_matches(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

/// Whether `path_and_query` is a `PeerTube` feed narrowed to a channel or a
/// playlist.
fn peer_tube_feed(path_and_query: &str) -> bool {
    let (path, query) = path_and_query
        .split_once('?')
        .map_or((path_and_query, ""), |(path, query)| (path, query));
    PEER_TUBE_PATHS.contains(&path) && query.split('&').any(has_peer_tube_filter)
}

/// Whether `param` is named by one of the `PeerTube` filter parameters.
fn has_peer_tube_filter(param: &str) -> bool {
    let name = param.split_once('=').map_or(param, |(name, _)| name);
    PEER_TUBE_FILTERS.contains(&name)
}
