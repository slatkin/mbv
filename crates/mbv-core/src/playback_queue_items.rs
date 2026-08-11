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
// QueueItem — enum wrapping the two item kinds the playback queue can hold.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum QueueItem {
    #[serde(rename = "Emby")]
    Emby(Box<EmbyItem>),
    #[serde(rename = "Feed")]
    Feed(FeedEntry),
}

/// Custom deserializer for `QueueItem` that accepts both the tagged form
/// (with `"kind": "Emby"` or `"kind": "Feed"`) and legacy bare `EmbyItem`
/// objects (no `kind` field). This preserves backward compatibility with
/// `queue_state.json` files written before the `QueueItem` enum existed.
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
                other => Err(de::Error::unknown_variant(other, &["Emby", "Feed"])),
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
        }
    }

    /// The Emby item ID for Emby items, or the feed GUID for feed entries.
    /// Used for server-refresh matching (only Emby items have server IDs,
    /// but this keeps the lookup uniform).
    pub fn id(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.id,
            QueueItem::Feed(entry) => &entry.guid,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            QueueItem::Emby(item) => item.display_name(),
            QueueItem::Feed(entry) => entry.title.clone(),
        }
    }

    pub fn runtime_ticks(&self) -> i64 {
        match self {
            QueueItem::Emby(item) => item.runtime_ticks,
            QueueItem::Feed(entry) => entry.duration_ticks.unwrap_or(0) as i64,
        }
    }

    pub fn playback_position_ticks(&self) -> i64 {
        match self {
            QueueItem::Emby(item) => item.playback_position_ticks,
            QueueItem::Feed(entry) => entry.position_ticks,
        }
    }

    pub fn played(&self) -> bool {
        match self {
            QueueItem::Emby(item) => item.played,
            QueueItem::Feed(entry) => entry.played,
        }
    }

    /// Returns the inner `EmbyItem` if this is an Emby variant.
    /// Used at boundaries that only operate on Emby items (send_ep_info,
    /// set_current_item_metadata, start_item, mark_played, etc.).
    pub fn as_emby(&self) -> Option<&EmbyItem> {
        match self {
            QueueItem::Emby(item) => Some(item),
            QueueItem::Feed(_) => None,
        }
    }

    pub fn playlist_item_id(&self) -> &str {
        match self {
            QueueItem::Emby(item) => &item.playlist_item_id,
            QueueItem::Feed(_) => "",
        }
    }
}
