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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QueueItemKind {
    Emby,
    Feed,
    Audiobookshelf,
}

// ---------------------------------------------------------------------------
// AudiobookshelfQueueItem — identity, presentation, duration, progress,
// completion, and Service-scoped artwork identity. Excludes credentials,
// server URL, playback sessionId, resolved source URL, and headers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

impl AudiobookshelfQueueItem {
    pub fn content_id(&self) -> QueueItemContentId {
        QueueItemContentId::Audiobookshelf {
            library_item_id: self.library_item_id.clone(),
            episode_id: self.episode_id.clone(),
        }
    }

    pub fn resume_seconds(&self) -> f64 {
        if crate::api::should_resume(self.position_ticks, self.duration_ticks.unwrap_or(0) as i64) {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    pub fn primary_source(&self) -> Option<&str> {
        self.enclosure_url.as_deref().or(self.link.as_deref())
    }
}

// ---------------------------------------------------------------------------
// QueueItem — enum wrapping the three item kinds the playback queue can hold.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum QueueItem {
    #[serde(rename = "Emby")]
    Emby(Box<EmbyItem>),
    #[serde(rename = "Feed")]
    Feed(FeedEntry),
    #[serde(rename = "Audiobookshelf")]
    Audiobookshelf(AudiobookshelfQueueItem),
}

/// Custom deserializer for `QueueItem` that accepts both the tagged form
/// (with `"kind": "Emby"`, `"kind": "Feed"`, or `"kind": "Audiobookshelf"`)
/// and legacy bare `EmbyItem` objects (no `kind` field). This preserves
/// backward compatibility with `queue_state.json` files written before the
/// `QueueItem` enum existed.
impl<'de> serde::Deserialize<'de> for QueueItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        let value = serde_json::Value::deserialize(deserializer)?;

        // Try tagged first: {"kind":"Emby",...} or {"kind":"Feed",...}
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
                    let ep =
                        AudiobookshelfQueueItem::deserialize(value).map_err(de::Error::custom)?;
                    Ok(QueueItem::Audiobookshelf(ep))
                }
                other => Err(de::Error::unknown_variant(
                    other,
                    &["Emby", "Feed", "Audiobookshelf"],
                )),
            };
        }

        // Legacy fallback: bare EmbyItem object (no `kind` field)
        let item = EmbyItem::deserialize(value).map_err(de::Error::custom)?;
        Ok(QueueItem::Emby(Box::new(item)))
    }
}

impl QueueItem {
    pub fn title(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.name,
            QueueItem::Feed(entry) => &entry.title,
            QueueItem::Audiobookshelf(ep) => &ep.title,
        }
    }

    pub fn duration(&self) -> Option<u64> {
        match self {
            QueueItem::Emby(item) => {
                if item.runtime_ticks > 0 {
                    Some(item.runtime_ticks as u64)
                } else {
                    None
                }
            }
            QueueItem::Feed(entry) => entry.duration_ticks,
            QueueItem::Audiobookshelf(ep) => ep.duration_ticks,
        }
    }

    pub fn media_kind(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.media_type,
            QueueItem::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("audio/") => "Audio",
                Some(m) if m.starts_with("video/") => "Video",
                _ => entry.feed_kind.map(|k| k.as_str()).unwrap_or("Video"),
            },
            QueueItem::Audiobookshelf(_) => "Audio",
        }
    }

    pub fn is_audio(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.is_audio(),
            QueueItem::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("audio/") => true,
                Some(m) if m.starts_with("video/") => false,
                _ => entry.feed_kind == Some(crate::config::FeedKind::Audio),
            },
            QueueItem::Audiobookshelf(_) => true,
        }
    }

    pub fn is_video(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.is_video(),
            QueueItem::Feed(entry) => match entry.mime_type.as_deref() {
                Some(m) if m.starts_with("video/") => true,
                Some(m) if m.starts_with("audio/") => false,
                _ => entry.feed_kind == Some(crate::config::FeedKind::Video),
            },
            QueueItem::Audiobookshelf(_) => false,
        }
    }

    /// Whether this is an Emby TV episode. Next Up is only meaningful for
    /// TVShow library items, not movies, music, or feed entries.
    pub fn is_tv_episode(&self) -> bool {
        matches!(self, QueueItem::Emby(item) if item.item_type == "Episode")
    }

    pub fn artwork_url(&self) -> Option<&str> {
        match self {
            QueueItem::Emby(_item) => None,
            QueueItem::Feed(_entry) => None,
            QueueItem::Audiobookshelf(ep) => ep.cover_path.as_deref(),
        }
    }

    /// The Emby item ID for Emby items, or the feed GUID for feed entries.
    /// For Audiobookshelf, returns the episode ID (typed identity is via
    /// `content_id()`).
    pub fn id(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.id,
            QueueItem::Feed(entry) => &entry.guid,
            QueueItem::Audiobookshelf(ep) => &ep.episode_id,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            QueueItem::Emby(item) => item.display_name(),
            QueueItem::Feed(entry) => entry.title.clone(),
            QueueItem::Audiobookshelf(ep) => {
                if let Some(show) = ep.show_title.as_deref().filter(|s| !s.is_empty()) {
                    format!("{show} - {}", ep.title)
                } else {
                    ep.title.clone()
                }
            }
        }
    }

    pub fn runtime_ticks(&self) -> i64 {
        match self {
            QueueItem::Emby(item) => item.runtime_ticks,
            QueueItem::Feed(entry) => entry.duration_ticks.unwrap_or(0) as i64,
            QueueItem::Audiobookshelf(ep) => ep.duration_ticks.unwrap_or(0) as i64,
        }
    }

    pub fn playback_position_ticks(&self) -> i64 {
        match self {
            QueueItem::Emby(item) => item.playback_position_ticks,
            QueueItem::Feed(entry) => entry.position_ticks,
            QueueItem::Audiobookshelf(ep) => ep.position_ticks,
        }
    }

    pub fn played(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.played,
            QueueItem::Feed(entry) => entry.played,
            QueueItem::Audiobookshelf(ep) => ep.played || ep.is_finished,
        }
    }

    /// Returns the inner `EmbyItem` if this is an Emby variant.
    /// Used at boundaries that only operate on Emby items (send_ep_info,
    /// set_current_item_metadata, start_item, mark_played, etc.).
    pub fn as_emby(&self) -> Option<&EmbyItem> {
        match self {
            QueueItem::Emby(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_feed(&self) -> Option<&FeedEntry> {
        match self {
            QueueItem::Feed(entry) => Some(entry),
            _ => None,
        }
    }

    pub fn as_audiobookshelf(&self) -> Option<&AudiobookshelfQueueItem> {
        match self {
            QueueItem::Audiobookshelf(ep) => Some(ep),
            _ => None,
        }
    }

    pub fn is_emby(&self) -> bool {
        matches!(self, QueueItem::Emby(_))
    }

    pub fn is_feed(&self) -> bool {
        matches!(self, QueueItem::Feed(_))
    }

    pub fn is_audiobookshelf(&self) -> bool {
        matches!(self, QueueItem::Audiobookshelf(_))
    }

    pub fn kind(&self) -> QueueItemKind {
        match self {
            QueueItem::Emby(_) => QueueItemKind::Emby,
            QueueItem::Feed(_) => QueueItemKind::Feed,
            QueueItem::Audiobookshelf(_) => QueueItemKind::Audiobookshelf,
        }
    }

    /// Typed Service-qualified content identity. Use this for matching
    /// and reconciliation instead of `format!("abs:{}:{}", library, episode)`.
    pub fn content_id(&self) -> QueueItemContentId {
        match self {
            QueueItem::Emby(item) => QueueItemContentId::Emby(item.id.clone()),
            QueueItem::Feed(entry) => QueueItemContentId::Feed(entry.guid.clone()),
            QueueItem::Audiobookshelf(ep) => ep.content_id(),
        }
    }

    /// Alias for typed identity (position tracking reuses content identity).
    pub fn content_key(&self) -> QueueItemContentId {
        self.content_id()
    }

    /// The Remote Service required to play this item. Emby and Feed items
    /// retain their existing local/source behavior; Audiobookshelf remains
    /// unplayable until a later playback capability enables an owner.
    pub fn required_service(&self) -> Option<crate::config::ServiceKind> {
        match self {
            QueueItem::Audiobookshelf(_) => Some(crate::config::ServiceKind::Audiobookshelf),
            QueueItem::Emby(_) | QueueItem::Feed(_) => None,
        }
    }

    pub fn admissible_for_owner(
        &self,
        audio_only: bool,
        has_service: impl Fn(crate::config::ServiceKind) -> bool,
    ) -> bool {
        // Audiobookshelf queue representation deliberately predates playback
        // support: no current owner may bind it, even if its Service exists.
        if self.is_audiobookshelf() {
            return false;
        }
        (!audio_only || self.is_audio()) && self.required_service().is_none_or(has_service)
    }

    pub fn playlist_item_id(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.playlist_item_id,
            QueueItem::Feed(_) => "",
            QueueItem::Audiobookshelf(_) => "",
        }
    }
}
