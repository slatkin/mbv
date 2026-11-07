//! The Client-side item-metadata fetches of `add-tauri-gui-client` design
//! D4a: chapters, intro/credits marks, and the trickplay source description,
//! fetched by the active queue item's `QueueItemContentId` and cached like
//! artwork (a `*_cache` + `*_loading` pair, one worker per key). The data is
//! presentation decoration only — no Owner, ctrl, or mpv change — and absent
//! data (no chapters, no segments, no trickplay, a fetch error) projects as
//! empty and never fails the frame.
//!
//! The TUI Client never issues these fetches; a Client's frame path calls
//! [`App::fetch_playback_metadata`] once per emitted frame, the same site
//! the artwork fetch issues from. The drain lands each completion in the
//! cache the [`crate::app::queue_playback_projection::QueuePlaybackProjection`]
//! reads, so the frame after a landing carries it.

use mbv_queue::{QueueItemContentId, ServiceKind};

use crate::app::App;
use crate::app::queue_playback_projection::{PlaybackChapter, SegmentMark, TrickplayGrid};

/// The metadata one item's fetch landed (design D4a): the chapter list, the
/// intro and credits marks, and the trickplay source. Every field is empty
/// when its kind fetched nothing — absence and error map to the same
/// decoration-free value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaybackDecorations {
    /// The item's chapters in start order, each with its name.
    pub chapters: Vec<PlaybackChapter>,
    /// The intro mark's tick bounds (the `MediaSegments` `Intro`).
    pub intro: Option<SegmentMark>,
    /// The credits mark's tick bounds (the `MediaSegments` `Outro`).
    pub credits: Option<SegmentMark>,
    /// The trickplay source description: one tile grid per available width.
    pub trickplay: Option<Vec<TrickplayGrid>>,
}

/// One item's resolved fetch source (design D4a): the Emby client the fill
/// installed, or the Audiobookshelf server and credential the cover fetch
/// builds its client from. Feed and Twitch items carry no metadata source.
enum MetadataFetch {
    Emby {
        client: Box<mbv_emby::EmbyClient>,
        item_id: String,
    },
    Audiobookshelf {
        server_url: String,
        api_key: String,
        library_item_id: String,
    },
}

impl MetadataFetch {
    /// Run the fetch off the core thread: each kind maps to its decorations
    /// or to empty on error, so a landed completion never fails.
    fn run(self) -> PlaybackDecorations {
        match self {
            Self::Emby { client, item_id } => fetch_emby_decorations(&client, &item_id),
            Self::Audiobookshelf {
                server_url,
                api_key,
                library_item_id,
            } => {
                let Ok(client) = mbv_audiobookshelf::AudiobookshelfClient::new(&server_url) else {
                    return PlaybackDecorations::default();
                };
                fetch_audiobookshelf_decorations(&client, &api_key, &library_item_id)
            }
        }
    }
}

/// The Emby item's decorations: `Chapters`, the `MediaSegments` `Intro`/`Outro`
/// bounds, and the `Trickplay` grids — each kind independently resolved, so a
/// kind's failure empties only itself (design D4a).
pub(in crate::app) fn fetch_emby_decorations(
    client: &mbv_emby::EmbyClient,
    item_id: &str,
) -> PlaybackDecorations {
    let chapters = client.get_item_chapters(item_id).map_or_else(
        |error| {
            tracing::warn!(
                name: "playback_metadata.chapters_failed",
                target: "playback_metadata",
                item = %item_id,
                error = %error,
                "item chapters fetch failed; projecting none"
            );
            Vec::new()
        },
        |chapters| {
            chapters
                .iter()
                .map(|chapter| PlaybackChapter {
                    name: chapter.name.clone(),
                    start_ticks: chapter.start_position_ticks,
                })
                .collect()
        },
    );
    let (intro, credits) = client.get_media_segments(item_id).map_or_else(
        |error| {
            tracing::warn!(
                name: "playback_metadata.segments_failed",
                target: "playback_metadata",
                item = %item_id,
                error = %error,
                "media segments fetch failed; projecting no marks"
            );
            (None, None)
        },
        |segments| segment_marks(&segments),
    );
    let trickplay = client.get_trickplay(item_id).map_or_else(
        |error| {
            tracing::warn!(
                name: "playback_metadata.trickplay_failed",
                target: "playback_metadata",
                item = %item_id,
                error = %error,
                "trickplay fetch failed; projecting no source"
            );
            Vec::new()
        },
        |grids| {
            grids
                .iter()
                .map(|grid| TrickplayGrid {
                    width: grid.width,
                    height: grid.height,
                    tile_width: grid.tile_width,
                    tile_height: grid.tile_height,
                    thumbnail_count: grid.thumbnail_count,
                    interval_ms: grid.interval,
                    media_source_id: grid.media_source_id.clone(),
                })
                .collect::<Vec<_>>()
        },
    );
    PlaybackDecorations {
        chapters,
        intro,
        credits,
        trickplay: (!trickplay.is_empty()).then_some(trickplay),
    }
}

/// The first `Intro` and first `Outro` of a `MediaSegments` response, as the
/// scrubber's two marks. Every other segment kind is not a mark.
fn segment_marks(
    segments: &[mbv_emby_model::EmbyMediaSegment],
) -> (Option<SegmentMark>, Option<SegmentMark>) {
    let mark = |kind: mbv_emby_model::EmbyMediaSegmentType| {
        segments
            .iter()
            .find(|segment| segment.segment_type == kind)
            .map(|segment| SegmentMark {
                start_ticks: segment.start_ticks,
                end_ticks: segment.end_ticks,
            })
    };
    (
        mark(mbv_emby_model::EmbyMediaSegmentType::Intro),
        mark(mbv_emby_model::EmbyMediaSegmentType::Outro),
    )
}

/// The Audiobookshelf item's decorations: `media.chapters` through the
/// existing `book_detail` call (design D4a). Audiobookshelf carries no
/// segment marks or trickplay, so the other fields stay empty.
pub(in crate::app) fn fetch_audiobookshelf_decorations(
    client: &mbv_audiobookshelf::AudiobookshelfClient,
    api_key: &str,
    library_item_id: &str,
) -> PlaybackDecorations {
    let chapters = client
        .book_detail_bounded(
            api_key,
            library_item_id,
            mbv_audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
        )
        .map_or_else(
            |error| {
                tracing::warn!(
                    name: "playback_metadata.abs_chapters_failed",
                    target: "playback_metadata",
                    item = %library_item_id,
                    error = %error,
                    "book detail fetch failed; projecting no chapters"
                );
                Vec::new()
            },
            |(chapters, _audio_files)| {
                chapters
                    .iter()
                    .map(|chapter| PlaybackChapter {
                        name: chapter.title.clone(),
                        start_ticks: mbv_emby_model::seconds_to_ticks(chapter.start),
                    })
                    .collect()
            },
        );
    PlaybackDecorations {
        chapters,
        ..PlaybackDecorations::default()
    }
}

impl App {
    /// Issue the active queue item's metadata fetch when none is cached or
    /// in flight (design D4a): one worker per item identity, the fetch
    /// deduped on the `playback_metadata_loading` set exactly as
    /// `fetch_album_tracks` dedupes on its own. A no-op for an item kind
    /// with no metadata source and while the kind's source is absent — the
    /// Emby client until the fill lands, the Audiobookshelf credential
    /// until one is configured — so the next frame retries a transient
    /// absence instead of caching it.
    pub fn fetch_playback_metadata(&mut self) {
        let Some(key) = self
            .displayed_playback_state()
            .active_idx
            .and_then(|idx| self.playback_queue().item_at(idx))
            .map(mbv_queue::QueueItem::content_id)
        else {
            return;
        };
        if self.playback_metadata_loading.contains(&key)
            || self.playback_metadata_cache.contains_key(&key)
        {
            return;
        }
        let Some(fetch) = self.metadata_fetch(&key) else {
            return;
        };
        self.playback_metadata_loading.insert(key.clone());
        let tx = self.channels.metadata_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((key, fetch.run()));
        });
    }

    /// The fetch source `key` resolves to, `None` for a kind with no
    /// metadata source or while the kind's source is absent.
    fn metadata_fetch(&self, key: &QueueItemContentId) -> Option<MetadataFetch> {
        match key {
            QueueItemContentId::Emby(item_id) => {
                self.emby_snapshot().map(|client| MetadataFetch::Emby {
                    client: Box::new(client),
                    item_id: item_id.clone(),
                })
            }
            QueueItemContentId::Audiobookshelf {
                library_item_id, ..
            }
            | QueueItemContentId::AudiobookshelfBook { library_item_id } => {
                let server_url = self
                    .config
                    .lock()
                    .unwrap()
                    .audiobookshelf_setup
                    .as_ref()
                    .map(|setup| setup.server_url.clone())?;
                let api_key = mbv_config::load_service_secret(ServiceKind::Audiobookshelf)?;
                Some(MetadataFetch::Audiobookshelf {
                    server_url,
                    api_key,
                    library_item_id: library_item_id.clone(),
                })
            }
            QueueItemContentId::Feed(_) | QueueItemContentId::Twitch(_) => None,
        }
    }

    /// Drain one landed metadata completion into the cache the playback
    /// projection reads (design D4a). Every completion carries its key's
    /// decorations — a worker always sends exactly one — so the drain only
    /// frees the loading slot and stores the result.
    pub(in crate::app) fn drain_playback_metadata_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok((key, decorations)) = self.channels.metadata_rx.try_recv() {
            produced = true;
            self.playback_metadata_loading.remove(&key);
            self.playback_metadata_cache.insert(key, decorations);
        }
        produced
    }

    /// Seed the metadata cache for `key`, as a landed completion would, so
    /// a Client's test projects the decoration with no fetch in flight.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playback_metadata_for_test(
        &mut self,
        key: QueueItemContentId,
        decorations: PlaybackDecorations,
    ) {
        self.playback_metadata_cache.insert(key, decorations);
    }
}

#[cfg(test)]
mod tests;
