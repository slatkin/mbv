use mbv_queue::FeedKind;

/// One user-configured feed subscription (`[[feeds]]` in config.toml).
/// The subscription list is the only persisted feed data: per-entry
/// playback state is never saved (#471 MVP).
#[derive(Debug, Clone)]
pub struct FeedSubscription {
    /// Display name shown in the Feeds tab and the management overlay.
    pub name: String,
    /// Feed URL (RSS 2.0 or Atom).
    pub url: String,
    /// Default media kind for this subscription's entries.
    pub kind: FeedKind,
}
