// Deciding what a Google Cast receiver is given for a queue item: the media
// URL and (for Emby) the device profile that negotiates it, per provider,
// and which items cannot be cast at all. See
// openspec/changes/add-chromecast-target/specs/cast-media-dispatch/spec.md.

use crate::audiobookshelf::{AudiobookshelfAudioSource, AudiobookshelfSourceMethod};
use crate::cast_client::CastMediaItem;
use crate::playback_queue::{AudiobookshelfBookQueueItem, FeedEntry};

/// Which subtitle rendition, if any, a dispatched Emby item needs. Sidecar
/// text tracks are dropped from v1 (design.md Risks: `rust_cast` 0.21 has no
/// wire primitive for Cast media tracks) -- the only subtitle-driven request
/// left is image-subtitle burn-in, made through the device profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastSubtitleKind {
    None,
    Text,
    Image { stream_index: i64 },
}

/// The Chromecast device profile sent with a cast-bound `PlaybackInfo`
/// request. `profile_json` is the codec/container baseline confirmed
/// against a Shield Android TV (design.md Risks, task 1.4); loosen it
/// against observed receiver failures, not speculatively.
#[derive(Debug, Clone, PartialEq)]
pub struct CastDeviceProfile {
    pub profile_json: serde_json::Value,
    pub subtitle_stream_index: Option<i64>,
}

pub fn build_cast_device_profile(subtitles: CastSubtitleKind) -> CastDeviceProfile {
    let profile_json = serde_json::json!({
        "Name": "mbv-chromecast",
        "MaxStreamingBitrate": 120_000_000,
        "DirectPlayProfiles": [
            {"Type": "Video", "Container": "mp4", "VideoCodec": "h264", "AudioCodec": "aac"},
            {"Type": "Audio", "Container": "mp3", "AudioCodec": "mp3"},
        ],
        "TranscodingProfiles": [
            {"Type": "Video", "Container": "mp4", "VideoCodec": "h264", "AudioCodec": "aac", "Context": "Streaming", "Protocol": "hls"},
            {"Type": "Audio", "Container": "mp3", "AudioCodec": "mp3", "Context": "Streaming", "Protocol": "http"},
        ],
        // Empty: mbv delivers no subtitle tracks to a cast receiver in v1.
        // This also forces burn-in when `subtitle_stream_index` below asks
        // for an image subtitle, since the server has no compatible
        // subtitle profile to deliver it through instead.
        "SubtitleProfiles": [],
    });
    let subtitle_stream_index = match subtitles {
        CastSubtitleKind::Image { stream_index } => Some(stream_index),
        CastSubtitleKind::None | CastSubtitleKind::Text => None,
    };
    CastDeviceProfile {
        profile_json,
        subtitle_stream_index,
    }
}

/// Resolves a feed entry's existing enclosure URL for cast dispatch.
pub fn resolve_feed_dispatch(entry: &FeedEntry) -> Result<CastMediaItem, String> {
    let url = entry
        .primary_source()
        .ok_or_else(|| format!("\"{}\" has no media URL to cast", entry.title))?;
    Ok(CastMediaItem {
        url: url.to_string(),
        content_type: feed_content_type(entry),
    })
}

fn feed_content_type(entry: &FeedEntry) -> String {
    if let Some(mime) = entry.mime_type.as_deref().filter(|m| !m.is_empty()) {
        return mime.to_string();
    }
    match entry.feed_kind.unwrap_or_default() {
        crate::config::FeedKind::Audio => "audio/mpeg".to_string(),
        crate::config::FeedKind::Video => "video/mp4".to_string(),
    }
}

/// Resolves an Audiobookshelf podcast episode's media URL for cast dispatch,
/// carrying `api_key` in the URL (confirmed viable by task 1.1 against a
/// live server: `?token=` is accepted with no `Authorization` header).
/// Restricted to `Direct` sources -- an HLS rendition's segment URLs were
/// not confirmed to carry the credential the same way, so it stays
/// uncastable rather than assumed castable.
pub fn resolve_audiobookshelf_episode_dispatch(
    source: &AudiobookshelfAudioSource,
    api_key: &str,
) -> Result<CastMediaItem, String> {
    if source.method != AudiobookshelfSourceMethod::Direct {
        return Err("Audiobookshelf HLS renditions are not castable".to_string());
    }
    Ok(CastMediaItem {
        url: append_token(&source.url, api_key),
        content_type: source.mime_type.clone(),
    })
}

fn append_token(url: &str, api_key: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}token={api_key}")
}

/// Audiobookshelf books are excluded from casting outright (design.md
/// "Audiobookshelf books are excluded"): a book's position is defined
/// across its whole timeline, and a receiver can only report position
/// within the file it is playing. Classification never opens a playback
/// session or otherwise touches the book's stored position.
pub fn resolve_audiobookshelf_book_dispatch(
    book: &AudiobookshelfBookQueueItem,
) -> Result<CastMediaItem, String> {
    Err(format!(
        "\"{}\" is a multi-file audiobook and can't be cast",
        book.title
    ))
}

/// One item's outcome when deciding whether it can be dispatched to a cast
/// receiver: its display name paired with either the media to load or the
/// reason it can't be.
pub struct CastDispatchItem {
    pub name: String,
    pub result: Result<CastMediaItem, String>,
}

/// Splits a selection's per-item resolutions into the media to dispatch and
/// the (name, reason) pairs to surface, in one pass.
pub fn partition_cast_dispatch(
    items: Vec<CastDispatchItem>,
) -> (Vec<CastMediaItem>, Vec<(String, String)>) {
    let mut dispatchable = Vec::new();
    let mut uncastable = Vec::new();
    for item in items {
        match item.result {
            Ok(media) => dispatchable.push(media),
            Err(reason) => uncastable.push((item.name, reason)),
        }
    }
    (dispatchable, uncastable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audiobookshelf::AudiobookshelfSourceMethod;

    // ── 4.1 device profile ──────────────────────────────────────────────

    #[test]
    fn profile_never_declares_a_subtitle_delivery_method() {
        for kind in [
            CastSubtitleKind::None,
            CastSubtitleKind::Text,
            CastSubtitleKind::Image { stream_index: 2 },
        ] {
            let profile = build_cast_device_profile(kind);
            assert_eq!(
                profile.profile_json["SubtitleProfiles"],
                serde_json::json!([])
            );
        }
    }

    #[test]
    fn profile_requests_no_subtitle_stream_when_absent() {
        let profile = build_cast_device_profile(CastSubtitleKind::None);
        assert_eq!(profile.subtitle_stream_index, None);
    }

    #[test]
    fn profile_requests_no_subtitle_stream_for_text() {
        let profile = build_cast_device_profile(CastSubtitleKind::Text);
        assert_eq!(profile.subtitle_stream_index, None);
    }

    #[test]
    fn profile_requests_the_image_subtitle_stream_for_burn_in() {
        let profile = build_cast_device_profile(CastSubtitleKind::Image { stream_index: 3 });
        assert_eq!(profile.subtitle_stream_index, Some(3));
    }

    // ── 4.6 burn-in vs. no request ──────────────────────────────────────

    #[test]
    fn image_subtitle_item_requests_burn_in() {
        let profile = build_cast_device_profile(CastSubtitleKind::Image { stream_index: 5 });
        assert!(profile.subtitle_stream_index.is_some());
    }

    #[test]
    fn text_subtitle_item_requests_neither_burn_in_nor_a_sidecar_track() {
        let profile = build_cast_device_profile(CastSubtitleKind::Text);
        assert!(profile.subtitle_stream_index.is_none());
        assert_eq!(
            profile.profile_json["SubtitleProfiles"],
            serde_json::json!([])
        );
    }

    // ── 4.3 feed dispatch ────────────────────────────────────────────────

    fn feed_entry(enclosure_url: Option<&str>, mime_type: Option<&str>) -> FeedEntry {
        FeedEntry {
            guid: "guid".into(),
            title: "Episode".into(),
            enclosure_url: enclosure_url.map(String::from),
            link: None,
            mime_type: mime_type.map(String::from),
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: Some("feed".into()),
            position_ticks: 0,
            played: false,
        }
    }

    #[test]
    fn feed_entry_with_enclosure_dispatches_unchanged() {
        let entry = feed_entry(Some("https://feed.test/file.mp3"), Some("audio/mpeg"));
        let media = resolve_feed_dispatch(&entry).unwrap();
        assert_eq!(media.url, "https://feed.test/file.mp3");
        assert_eq!(media.content_type, "audio/mpeg");
    }

    #[test]
    fn feed_entry_without_url_is_uncastable() {
        let entry = feed_entry(None, None);
        let Err(error) = resolve_feed_dispatch(&entry) else {
            panic!("expected an uncastable result");
        };
        assert!(error.contains("Episode"));
    }

    // ── 4.4 Audiobookshelf episode dispatch ─────────────────────────────

    fn direct_source(url: &str) -> AudiobookshelfAudioSource {
        AudiobookshelfAudioSource {
            method: AudiobookshelfSourceMethod::Direct,
            url: url.to_string(),
            mime_type: "audio/mpeg".to_string(),
            duration_seconds: 100.0,
        }
    }

    #[test]
    fn episode_dispatch_appends_token_to_a_bare_url() {
        let source = direct_source("https://abs.test/api/items/1/file/2");
        let media = resolve_audiobookshelf_episode_dispatch(&source, "secret").unwrap();
        assert_eq!(
            media.url,
            "https://abs.test/api/items/1/file/2?token=secret"
        );
    }

    #[test]
    fn episode_dispatch_appends_token_after_existing_query() {
        let source = direct_source("https://abs.test/api/items/1/file/2?ts=1");
        let media = resolve_audiobookshelf_episode_dispatch(&source, "secret").unwrap();
        assert_eq!(
            media.url,
            "https://abs.test/api/items/1/file/2?ts=1&token=secret"
        );
    }

    #[test]
    fn hls_episode_is_uncastable() {
        let source = AudiobookshelfAudioSource {
            method: AudiobookshelfSourceMethod::Hls,
            ..direct_source("https://abs.test/hls/1.m3u8")
        };
        assert!(resolve_audiobookshelf_episode_dispatch(&source, "secret").is_err());
    }

    // ── 4.5 Audiobookshelf book classification ──────────────────────────

    fn book(position_ticks: i64) -> AudiobookshelfBookQueueItem {
        AudiobookshelfBookQueueItem {
            library_item_id: "book".into(),
            title: "Book".into(),
            author: None,
            duration_ticks: Some(1000),
            position_ticks,
            played: false,
            is_finished: false,
            cover_path: None,
        }
    }

    #[test]
    fn book_is_classified_uncastable() {
        let book = book(500);
        let Err(error) = resolve_audiobookshelf_book_dispatch(&book) else {
            panic!("expected an uncastable result");
        };
        assert!(error.contains("Book"));
    }

    #[test]
    fn book_classification_performs_no_position_write() {
        let book = book(500);
        let _ = resolve_audiobookshelf_book_dispatch(&book);
        assert_eq!(book.position_ticks, 500);
    }

    // ── 4.7 castability partition ───────────────────────────────────────

    #[test]
    fn mixed_selection_yields_dispatchable_entries_and_reasons_in_one_pass() {
        let items = vec![
            CastDispatchItem {
                name: "Castable".into(),
                result: Ok(CastMediaItem {
                    url: "https://a".into(),
                    content_type: "video/mp4".into(),
                }),
            },
            CastDispatchItem {
                name: "Uncastable".into(),
                result: Err("no media URL".into()),
            },
        ];
        let (dispatchable, uncastable) = partition_cast_dispatch(items);
        assert_eq!(dispatchable.len(), 1);
        assert_eq!(dispatchable[0].url, "https://a");
        assert_eq!(
            uncastable,
            vec![("Uncastable".to_string(), "no media URL".to_string())]
        );
    }
}
