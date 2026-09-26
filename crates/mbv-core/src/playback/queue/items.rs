use crate::api::EmbyItem;

// ---------------------------------------------------------------------------
// Content identity — typed provider-qualified identity, avoiding formatted
// string matching like `format!("abs:{}:{}", lib, ep)`.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "provider", content = "value")]
pub enum QueueItemContentId {
    Emby(String),
    Feed(String),
    Audiobookshelf {
        library_item_id: String,
        episode_id: String,
    },
    AudiobookshelfBook {
        library_item_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QueueItemKind {
    Emby,
    Feed,
    Audiobookshelf,
    AudiobookshelfBook,
}

// ---------------------------------------------------------------------------
// Now-playing title parts — typed, colour-free presentation metadata for the
// playback panel's title row: the item's own title part plus an optional
// context part naming the container it came from. Roles are closed; the
// painter resolves a role to a colour, never the producer.
// ---------------------------------------------------------------------------

/// The closed role a now-playing title part plays: the item's own name
/// (`Title`) or the container it came from (`Context`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackTitlePartRole {
    /// The item's own name: episode name, track name, entry title, ... .
    Title,
    /// The container the item came from: series, artist, show, subscription.
    Context,
}

/// One part of a now-playing title: its text and the closed role it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackTitlePart {
    pub role: PlaybackTitlePartRole,
    pub text: String,
}

/// The now-playing title parts for a queue item: the item's own title part,
/// plus the optional context part naming the container it came from. Media
/// types with no container — and media types whose container name is absent
/// — carry the title part alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackTitleParts {
    pub title: PlaybackTitlePart,
    pub context: Option<PlaybackTitlePart>,
}

impl PlaybackTitleParts {
    fn single(title: impl Into<String>) -> Self {
        PlaybackTitleParts {
            title: PlaybackTitlePart {
                role: PlaybackTitlePartRole::Title,
                text: title.into(),
            },
            context: None,
        }
    }

    fn two(title: impl Into<String>, context: impl Into<String>) -> Self {
        let mut parts = Self::single(title);
        parts.context = Some(PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: context.into(),
        });
        parts
    }
}

// ---------------------------------------------------------------------------
// AudiobookshelfQueueItem — identity, presentation, duration, progress,
// completion, and Service-scoped artwork identity. Excludes credentials,
// server URL, playback sessionId, resolved source URL, and headers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudiobookshelfQueueItem {
    #[serde(rename = "libraryItemId")]
    pub library_item_id: String,
    #[serde(rename = "episodeId")]
    pub episode_id: String,
    pub title: String,
    #[serde(default)]
    pub show_title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    /// Episode synopsis/description, when the catalog carries one. Used by
    /// Home's hero detail for Audiobookshelf rows.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub duration_ticks: Option<u64>,
    /// Playback position in ticks. Zero means start from the beginning.
    #[serde(default)]
    pub position_ticks: i64,
    #[serde(default)]
    pub played: bool,
    /// Publish time in Unix seconds, when available from the catalog.
    #[serde(default)]
    pub pub_date_secs: Option<u64>,
    /// Mirrors Audiobookshelf `is_finished` (distinct from played state
    /// persisted here). Defaulted for backward compat.
    #[serde(default)]
    pub is_finished: bool,
    /// Service-scoped artwork identity (cover path, not server URL).
    #[serde(default)]
    pub cover_path: Option<String>,
}

fn duration_ticks_as_i64(ticks: u64) -> i64 {
    // Preserve the persisted queue's existing two's-complement wrap semantics.
    i64::from_ne_bytes(ticks.to_ne_bytes())
}

impl AudiobookshelfQueueItem {
    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        QueueItemContentId::Audiobookshelf {
            library_item_id: self.library_item_id.clone(),
            episode_id: self.episode_id.clone(),
        }
    }

    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "Audiobookshelf episode position ticks → seconds through f64; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    pub fn resume_seconds(&self) -> f64 {
        if crate::api::should_resume(
            self.position_ticks,
            duration_ticks_as_i64(self.duration_ticks.unwrap_or(0)),
        ) {
            self.position_ticks as f64 / crate::api::TICKS_PER_SECOND as f64
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// AudiobookshelfBookQueueItem — the book-shaped queue snapshot: content
// identity, presentation, progress, and completion keyed by `library_item_id`
// only. No episode identity. Excludes credentials, server URL, playback
// sessionId, resolved source URLs, and headers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudiobookshelfBookQueueItem {
    #[serde(rename = "libraryItemId")]
    pub library_item_id: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub duration_ticks: Option<u64>,
    /// Playback position in ticks. Zero means start from the beginning.
    #[serde(default)]
    pub position_ticks: i64,
    #[serde(default)]
    pub played: bool,
    /// Mirrors Audiobookshelf `is_finished` (distinct from played state).
    #[serde(default)]
    pub is_finished: bool,
    /// Service-scoped artwork identity (cover path, not server URL).
    #[serde(default)]
    pub cover_path: Option<String>,
}

impl AudiobookshelfBookQueueItem {
    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        QueueItemContentId::AudiobookshelfBook {
            library_item_id: self.library_item_id.clone(),
        }
    }

    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "Audiobookshelf book position ticks → seconds through f64; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    pub fn resume_seconds(&self) -> f64 {
        if crate::api::should_resume(
            self.position_ticks,
            duration_ticks_as_i64(self.duration_ticks.unwrap_or(0)),
        ) {
            self.position_ticks as f64 / crate::api::TICKS_PER_SECOND as f64
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// FeedEntry — identity, playback, and resume fields for RSS/podcast/YouTube
// items. Identity (`feed_id`) and progress (`position_ticks`, `played`) are
// serde-defaulted so old queue payloads and ctrl snapshots remain loadable.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedEntry {
    pub guid: String,
    pub title: String,
    pub enclosure_url: Option<String>,
    pub link: Option<String>,
    pub mime_type: Option<String>,
    pub duration_ticks: Option<u64>,
    /// Publish time in unix seconds UTC (RSS `pubDate` / Atom
    /// `published`/`updated`), for the "All" group's newest-first sort.
    /// Missing dates sort last. `#[serde(default)]` keeps old
    /// `queue_state.json` files (pre-#471) loading.
    #[serde(default)]
    pub pub_date_secs: Option<u64>,
    /// Subscription's `FeedKind` carried into the queued snapshot. Canonical
    /// media kind when enclosure MIME is absent or unrecognized; enclosure MIME
    /// refines it when recognized. `None` means unknown — legacy
    /// persisted/wire data that predates this field.
    #[serde(default)]
    pub feed_kind: Option<crate::config::FeedKind>,
    /// Stable feed identity for the keyed feed-entry state store (#492).
    /// Set to the normalized subscription URL at fetch time. `None` means
    /// the entry cannot address the store (legacy or identity-less) and
    /// playback falls through to stateless behavior.
    #[serde(default)]
    pub feed_id: Option<String>,
    /// Playback position in ticks. Zero means start from the beginning
    /// (or the entry was marked played with position reset).
    #[serde(default)]
    pub position_ticks: i64,
    /// Whether the entry has been played to completion (known-runtime EOF
    /// or stop ≥ 95%). Played entries with position zero replay from start.
    #[serde(default)]
    pub played: bool,
}

impl FeedEntry {
    /// The best playable URL: enclosure first, then link as fallback.
    #[must_use]
    pub fn primary_source(&self) -> Option<&str> {
        self.enclosure_url.as_deref().or(self.link.as_deref())
    }
}

// ---------------------------------------------------------------------------
// QueueItem — enum wrapping the three item kinds the playback queue can hold.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AudiobookshelfItem {
    Episode(AudiobookshelfQueueItem),
    Book(AudiobookshelfBookQueueItem),
}

impl AudiobookshelfItem {
    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            Self::Episode(item) => &item.title,
            Self::Book(item) => &item.title,
        }
    }

    #[must_use]
    pub fn duration(&self) -> Option<u64> {
        match self {
            Self::Episode(item) => item.duration_ticks,
            Self::Book(item) => item.duration_ticks,
        }
    }

    #[must_use]
    pub fn media_kind(&self) -> &str {
        "Audio"
    }

    #[must_use]
    pub fn is_audio(&self) -> bool {
        true
    }

    #[must_use]
    pub fn is_music(&self) -> bool {
        false
    }

    #[must_use]
    pub fn is_video(&self) -> bool {
        false
    }

    #[must_use]
    pub fn artwork_url(&self) -> Option<&str> {
        match self {
            Self::Episode(item) => item.cover_path.as_deref(),
            Self::Book(item) => item.cover_path.as_deref(),
        }
    }

    #[must_use]
    pub fn display_name_parts(&self) -> (String, Option<String>) {
        match self {
            Self::Episode(item) => item
                .show_title
                .as_deref()
                .filter(|show| !show.is_empty())
                .map_or_else(
                    || (item.title.clone(), None),
                    |show| (show.to_owned(), Some(item.title.clone())),
                ),
            Self::Book(item) => (item.title.clone(), None),
        }
    }

    #[must_use]
    pub fn playback_title_parts(&self) -> PlaybackTitleParts {
        match self {
            Self::Episode(item) => item
                .show_title
                .as_deref()
                .filter(|show| !show.is_empty())
                .map_or_else(
                    || PlaybackTitleParts::single(item.title.clone()),
                    |show| PlaybackTitleParts::two(item.title.clone(), show.to_owned()),
                ),
            Self::Book(item) => PlaybackTitleParts::single(item.title.clone()),
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Episode(item) => item
                .show_title
                .as_deref()
                .filter(|show| !show.is_empty())
                .map_or_else(
                    || item.title.clone(),
                    |show| format!("{show} - {}", item.title),
                ),
            Self::Book(item) => item.title.clone(),
        }
    }

    #[must_use]
    pub fn overview(&self) -> Option<&str> {
        match self {
            Self::Episode(item) => item.description.as_deref(),
            Self::Book(_) => None,
        }
    }

    #[must_use]
    pub fn runtime_ticks(&self) -> i64 {
        duration_ticks_as_i64(self.duration().unwrap_or(0))
    }

    #[must_use]
    pub fn playback_position_ticks(&self) -> i64 {
        match self {
            Self::Episode(item) => item.position_ticks,
            Self::Book(item) => item.position_ticks,
        }
    }

    #[must_use]
    pub fn played(&self) -> bool {
        match self {
            Self::Episode(item) => item.played || item.is_finished,
            Self::Book(item) => item.played || item.is_finished,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Episode(item) => &item.episode_id,
            Self::Book(item) => &item.library_item_id,
        }
    }

    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        match self {
            Self::Episode(item) => item.content_id(),
            Self::Book(item) => item.content_id(),
        }
    }

    #[must_use]
    pub fn playlist_item_id(&self) -> &str {
        ""
    }

    #[must_use]
    pub fn kind(&self) -> QueueItemKind {
        match self {
            Self::Episode(_) => QueueItemKind::Audiobookshelf,
            Self::Book(_) => QueueItemKind::AudiobookshelfBook,
        }
    }
}

#[derive(Debug, Clone)]
pub enum QueueItem {
    Emby(Box<EmbyItem>),
    Feed(FeedEntry),
    Audiobookshelf(AudiobookshelfItem),
}

/// A queued source that can be resolved directly to an mpv URL.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MpvUrlSource<'a> {
    Emby(&'a EmbyItem),
    Feed(&'a FeedEntry),
}

// Serialize through the original flat internally-tagged representation. This
// preserves both the legacy discriminator and the inner fields' wire order.
#[derive(serde::Serialize)]
#[serde(tag = "kind")]
enum QueueItemSerialize<'a> {
    #[serde(rename = "Emby")]
    Emby(&'a EmbyItem),
    #[serde(rename = "Feed")]
    Feed(&'a FeedEntry),
    #[serde(rename = "Audiobookshelf")]
    Audiobookshelf(&'a AudiobookshelfQueueItem),
    #[serde(rename = "AudiobookshelfBook")]
    AudiobookshelfBook(&'a AudiobookshelfBookQueueItem),
}

impl serde::Serialize for QueueItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let proxy = match self {
            Self::Emby(item) => QueueItemSerialize::Emby(item),
            Self::Feed(item) => QueueItemSerialize::Feed(item),
            Self::Audiobookshelf(AudiobookshelfItem::Episode(item)) => {
                QueueItemSerialize::Audiobookshelf(item)
            }
            Self::Audiobookshelf(AudiobookshelfItem::Book(item)) => {
                QueueItemSerialize::AudiobookshelfBook(item)
            }
        };
        proxy.serialize(serializer)
    }
}

/// Custom deserializer for `QueueItem` that accepts both the tagged form
/// (with `kind` discriminators) and legacy bare `EmbyItem` objects (no `kind`
/// field). This preserves backward compatibility with older queue state.
impl<'de> serde::Deserialize<'de> for QueueItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        let value = serde_json::Value::deserialize(deserializer)?;

        if let Some(kind) = value.get("kind").and_then(|k| k.as_str()) {
            return match kind {
                "Emby" => {
                    let item = EmbyItem::deserialize(value).map_err(de::Error::custom)?;
                    Ok(QueueItem::Emby(Box::new(item)))
                }
                "Feed" => {
                    let entry = FeedEntry::deserialize(value).map_err(de::Error::custom)?;
                    Ok(QueueItem::Feed(entry))
                }
                "Audiobookshelf" => {
                    let item =
                        AudiobookshelfQueueItem::deserialize(value).map_err(de::Error::custom)?;
                    Ok(QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(item)))
                }
                "AudiobookshelfBook" => {
                    let item = AudiobookshelfBookQueueItem::deserialize(value)
                        .map_err(de::Error::custom)?;
                    Ok(QueueItem::Audiobookshelf(AudiobookshelfItem::Book(item)))
                }
                other => Err(de::Error::unknown_variant(
                    other,
                    &["Emby", "Feed", "Audiobookshelf", "AudiobookshelfBook"],
                )),
            };
        }

        // Legacy fallback: bare EmbyItem object (no `kind` field)
        let item = EmbyItem::deserialize(value).map_err(de::Error::custom)?;
        Ok(QueueItem::Emby(Box::new(item)))
    }
}

impl QueueItem {
    #[must_use]
    pub(crate) fn mpv_url_source(&self) -> Option<MpvUrlSource<'_>> {
        match self {
            Self::Emby(item) => Some(MpvUrlSource::Emby(item)),
            Self::Feed(entry) => Some(MpvUrlSource::Feed(entry)),
            Self::Audiobookshelf(_) => None,
        }
    }

    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            Self::Emby(item) => &item.name,
            Self::Feed(entry) => &entry.title,
            Self::Audiobookshelf(item) => item.title(),
        }
    }

    #[must_use]
    pub fn duration(&self) -> Option<u64> {
        match self {
            Self::Emby(item) => u64::try_from(item.runtime_ticks)
                .ok()
                .filter(|_| item.runtime_ticks > 0),
            Self::Feed(entry) => entry.duration_ticks,
            Self::Audiobookshelf(item) => item.duration(),
        }
    }

    #[must_use]
    pub fn media_kind(&self) -> &str {
        match self {
            Self::Emby(item) => &item.media_type,
            Self::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("audio/") => "Audio",
                Some(m) if m.starts_with("video/") => "Video",
                _ => entry.feed_kind.map_or("Video", |k| k.as_str()),
            },
            Self::Audiobookshelf(item) => item.media_kind(),
        }
    }

    #[must_use]
    pub fn is_audio(&self) -> bool {
        match self {
            Self::Emby(item) => item.is_audio(),
            Self::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("audio/") => true,
                Some(m) if m.starts_with("video/") => false,
                _ => entry.feed_kind == Some(crate::config::FeedKind::Audio),
            },
            Self::Audiobookshelf(item) => item.is_audio(),
        }
    }

    /// Whether this is a music item. Feeds and Audiobookshelf content are not music.
    #[must_use]
    pub fn is_music(&self) -> bool {
        match self {
            Self::Emby(item) => item.is_music(),
            Self::Feed(_) => false,
            Self::Audiobookshelf(item) => item.is_music(),
        }
    }

    #[must_use]
    pub fn is_video(&self) -> bool {
        match self {
            Self::Emby(item) => item.is_video(),
            Self::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("video/") => true,
                Some(m) if m.starts_with("audio/") => false,
                _ => entry.feed_kind == Some(crate::config::FeedKind::Video),
            },
            Self::Audiobookshelf(item) => item.is_video(),
        }
    }

    /// Whether this is an Emby TV episode. Next Up is only meaningful for
    /// `TVShow` library items, not movies, music, or feed entries.
    #[must_use]
    pub fn is_tv_episode(&self) -> bool {
        matches!(self, Self::Emby(item) if item.item_type == "Episode")
    }

    #[must_use]
    pub fn artwork_url(&self) -> Option<&str> {
        match self {
            Self::Emby(_) | Self::Feed(_) => None,
            Self::Audiobookshelf(item) => item.artwork_url(),
        }
    }

    /// Audiobookshelf returns the episode ID; typed identity is via `content_id()`.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Emby(item) => &item.id,
            Self::Feed(entry) => &entry.guid,
            Self::Audiobookshelf(item) => item.id(),
        }
    }

    #[must_use]
    pub fn display_name_parts(&self) -> (String, Option<String>) {
        match self {
            Self::Emby(item) => item.display_name_parts(),
            Self::Audiobookshelf(item) => item.display_name_parts(),
            other => (other.display_name(), None),
        }
    }

    /// The now-playing title parts for the playback panel: the item's own
    /// title plus optional context. The feed subscription name is caller-resolved
    /// and ignored by every non-feed kind.
    #[must_use]
    pub fn playback_title_parts(&self, feed_subscription_name: Option<&str>) -> PlaybackTitleParts {
        match self {
            Self::Emby(item) => {
                if item.item_type == "Episode" && !item.series_name.is_empty() {
                    PlaybackTitleParts::two(item.name.clone(), item.series_name.clone())
                } else if item.is_audio() && !item.artist.is_empty() {
                    PlaybackTitleParts::two(item.name.clone(), item.artist.clone())
                } else {
                    PlaybackTitleParts::single(item.name.clone())
                }
            }
            Self::Feed(entry) => {
                if let Some(name) = feed_subscription_name.filter(|n| !n.is_empty()) {
                    PlaybackTitleParts::two(entry.title.clone(), name.to_owned())
                } else {
                    PlaybackTitleParts::single(entry.title.clone())
                }
            }
            Self::Audiobookshelf(item) => item.playback_title_parts(),
        }
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Emby(item) => item.display_name(),
            Self::Feed(entry) => entry.title.clone(),
            Self::Audiobookshelf(item) => item.display_name(),
        }
    }

    /// A short synopsis/overview for the item, when one is available. Used by
    /// Home's hero detail.
    #[must_use]
    pub fn overview(&self) -> Option<&str> {
        match self {
            Self::Emby(item) => (!item.overview.is_empty()).then_some(item.overview.as_str()),
            Self::Feed(_) => None,
            Self::Audiobookshelf(item) => item.overview(),
        }
    }

    #[must_use]
    pub fn runtime_ticks(&self) -> i64 {
        match self {
            Self::Emby(item) => item.runtime_ticks,
            Self::Feed(entry) => duration_ticks_as_i64(entry.duration_ticks.unwrap_or(0)),
            Self::Audiobookshelf(item) => item.runtime_ticks(),
        }
    }

    #[must_use]
    pub fn playback_position_ticks(&self) -> i64 {
        match self {
            Self::Emby(item) => item.playback_position_ticks,
            Self::Feed(entry) => entry.position_ticks,
            Self::Audiobookshelf(item) => item.playback_position_ticks(),
        }
    }

    #[must_use]
    pub fn played(&self) -> bool {
        match self {
            Self::Emby(item) => item.played,
            Self::Feed(entry) => entry.played,
            Self::Audiobookshelf(item) => item.played(),
        }
    }

    /// Returns the inner `EmbyItem` if this is an Emby variant.
    /// Used at boundaries that only operate on Emby items (`send_ep_info`,
    /// `set_current_item_metadata`, `start_item`, `mark_played`, etc.).
    #[must_use]
    pub fn as_emby(&self) -> Option<&EmbyItem> {
        match self {
            Self::Emby(item) => Some(item),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_feed(&self) -> Option<&FeedEntry> {
        match self {
            Self::Feed(entry) => Some(entry),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_audiobookshelf(&self) -> Option<&AudiobookshelfQueueItem> {
        match self {
            Self::Audiobookshelf(AudiobookshelfItem::Episode(item)) => Some(item),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_audiobookshelf_book(&self) -> Option<&AudiobookshelfBookQueueItem> {
        match self {
            Self::Audiobookshelf(AudiobookshelfItem::Book(item)) => Some(item),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_emby(&self) -> bool {
        matches!(self, Self::Emby(_))
    }

    #[must_use]
    pub fn is_feed(&self) -> bool {
        matches!(self, Self::Feed(_))
    }

    #[must_use]
    pub fn is_audiobookshelf(&self) -> bool {
        matches!(self, Self::Audiobookshelf(_))
    }

    #[must_use]
    pub fn kind(&self) -> QueueItemKind {
        match self {
            Self::Emby(_) => QueueItemKind::Emby,
            Self::Feed(_) => QueueItemKind::Feed,
            Self::Audiobookshelf(item) => item.kind(),
        }
    }

    /// Typed Service-qualified content identity. Use this for matching
    /// and reconciliation instead of formatted string matching.
    #[must_use]
    pub fn content_id(&self) -> QueueItemContentId {
        match self {
            Self::Emby(item) => QueueItemContentId::Emby(item.id.clone()),
            Self::Feed(entry) => QueueItemContentId::Feed(entry.guid.clone()),
            Self::Audiobookshelf(item) => item.content_id(),
        }
    }

    #[must_use]
    pub fn content_key(&self) -> QueueItemContentId {
        self.content_id()
    }

    /// The Remote Service required to play this item. Emby and Feed items
    /// retain their existing local/source behavior; Audiobookshelf admission
    /// is decided by the owner capability supplied to
    /// `admissible_for_owner`.
    #[must_use]
    pub fn required_service(&self) -> Option<crate::config::ServiceKind> {
        match self {
            Self::Audiobookshelf(_) => Some(crate::config::ServiceKind::Audiobookshelf),
            Self::Emby(_) | Self::Feed(_) => None,
        }
    }

    pub fn admissible_for_owner(
        &self,
        audio_only: bool,
        has_service: impl Fn(crate::config::ServiceKind) -> bool,
    ) -> bool {
        self.admissible_for_owner_with_audiobookshelf(audio_only, has_service, false)
    }

    pub fn admissible_for_owner_with_audiobookshelf(
        &self,
        audio_only: bool,
        has_service: impl Fn(crate::config::ServiceKind) -> bool,
        can_admit_audiobookshelf: bool,
    ) -> bool {
        if self.is_audiobookshelf() && !can_admit_audiobookshelf {
            return false;
        }
        (!audio_only || self.is_audio()) && self.required_service().is_none_or(has_service)
    }

    #[must_use]
    pub fn playlist_item_id(&self) -> &str {
        match self {
            Self::Emby(item) => &item.playlist_item_id,
            Self::Feed(_) => "",
            Self::Audiobookshelf(item) => item.playlist_item_id(),
        }
    }
}
