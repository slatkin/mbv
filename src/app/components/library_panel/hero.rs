//! The artwork policy and hero producers (task 5.4, design D5): one policy
//! function per provider content type, one producer per content type. The
//! policy chooses the artwork shape from provider metadata — never from the
//! fetched image, and never from a destination — and returns the Emby
//! image-type chain and cache key; the producers fill [`HeroFacts`] with
//! plain `meta_rows` (no `Span`/`Style`/width — the header painter colours,
//! truncates and wraps).

use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use mbv_core::playback_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfQueueItem, FeedEntry, QueueItem,
};

use crate::app::render::components::hero_model::{
    emby_hero_meta_rows_plain, SERIES_LANDSCAPE_IMAGE_TYPES,
};
use crate::app::render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES;
use crate::app::ui_util::{clean_overview, fmt_duration_approx};

use super::content::{ArtworkShape, ArtworkSource, HeroArtwork, HeroFacts, HeroImageState};

/// A producer's output (design D5): the facts plus the item's overview.
/// Destinations attach a `Workspace` when assembling [`HeroContent`]; the
/// header painter and the Narrow inline hero (task 5.7) both consume this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroContentData {
    pub facts: HeroFacts,
    pub overview: Option<String>,
}

// ── Artwork policy ─────────────────────────────────────────────────────

/// The artwork policy for one Emby item (design D5): Music (albums, tracks)
/// is always Square; everything else takes the first shape the provider
/// declares available in the order Landscape > Square > Portrait — a
/// declared `Thumb`/backdrop (an episode's series tags included) is
/// Landscape, a declared `Primary` poster alone is Portrait — and nothing
/// declared falls back to Landscape with the placeholder. Availability is
/// provider metadata only, so the arm is stable before the image loads.
pub(in crate::app) fn emby_artwork_policy(item: &EmbyItem) -> HeroArtwork {
    let music = item.item_type == "MusicAlbum" || item.is_audio();
    if music {
        return HeroArtwork {
            shape: ArtworkShape::Square,
            source: music_source(item),
            image: HeroImageState::None,
        };
    }
    let tags = &item.image_tags;
    let landscape_declared = !tags.thumb.is_empty()
        || !tags.backdrops.is_empty()
        || !tags.series_thumb.is_empty()
        || !tags.series_backdrops.is_empty();
    let poster_declared = !tags.primary.is_empty();
    if item.id.is_empty() {
        // Nothing to fetch: the Landscape placeholder (design D5's
        // nothing-declared fallback).
        return HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            image: HeroImageState::None,
        };
    }
    if landscape_declared {
        HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: Some(emby_source(item, landscape_image_chain(item))),
            image: HeroImageState::None,
        }
    } else if poster_declared {
        HeroArtwork {
            shape: ArtworkShape::Portrait,
            source: Some(emby_source(item, &["Primary", "Backdrop", "Logo"])),
            image: HeroImageState::None,
        }
    } else {
        HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            image: HeroImageState::None,
        }
    }
}

/// The artwork policy for one queue item: dispatches to the item kind's
/// policy (Emby, podcast episode, book, or feed entry).
pub(in crate::app) fn queue_artwork_policy(item: &QueueItem) -> HeroArtwork {
    match item {
        QueueItem::Emby(item) => emby_artwork_policy(item),
        QueueItem::Audiobookshelf(episode) => abs_episode_artwork_policy(episode),
        QueueItem::AudiobookshelfBook(book) => abs_book_artwork_policy(book),
        QueueItem::Feed(entry) => feed_artwork_policy(entry),
    }
}

/// The artwork policy for an Audiobookshelf book (design D5): books declare
/// a portrait cover; no cover means the Portrait placeholder.
pub(in crate::app) fn abs_book_artwork_policy(book: &AudiobookshelfBookQueueItem) -> HeroArtwork {
    HeroArtwork {
        shape: ArtworkShape::Portrait,
        source: book
            .cover_path
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(|_| ArtworkSource::AudiobookshelfCover {
                library_item_id: book.library_item_id.clone(),
                book: true,
            }),
        image: HeroImageState::None,
    }
}

/// The artwork policy for an Audiobookshelf podcast episode (design D5):
/// podcasts are always Square; the cover is Service-scoped.
pub(in crate::app) fn abs_episode_artwork_policy(episode: &AudiobookshelfQueueItem) -> HeroArtwork {
    HeroArtwork {
        shape: ArtworkShape::Square,
        source: episode
            .cover_path
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(|_| ArtworkSource::AudiobookshelfCover {
                library_item_id: episode.library_item_id.clone(),
                book: false,
            }),
        image: HeroImageState::None,
    }
}

/// The artwork policy for an Audiobookshelf podcast show (design D5): square
/// cover, like every podcast.
pub(in crate::app) fn abs_show_artwork_policy(
    show: &mbv_core::audiobookshelf::AudiobookshelfShow,
) -> HeroArtwork {
    HeroArtwork {
        shape: ArtworkShape::Square,
        source: show
            .cover_path
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(|_| ArtworkSource::AudiobookshelfCover {
                library_item_id: show.library_item_id.clone(),
                book: false,
            }),
        image: HeroImageState::None,
    }
}

/// The artwork policy for a feed entry (design D5): no artwork source exists
/// for feed entries. Podcast feeds (declared `FeedKind::Audio`) are Square;
/// video (and unknown-kind legacy) feeds fall back to Landscape — both with
/// the shared placeholder.
pub(in crate::app) fn feed_artwork_policy(entry: &FeedEntry) -> HeroArtwork {
    let podcast = matches!(entry.feed_kind, Some(mbv_core::config::FeedKind::Audio));
    HeroArtwork {
        shape: if podcast {
            ArtworkShape::Square
        } else {
            ArtworkShape::Landscape
        },
        source: None,
        image: HeroImageState::None,
    }
}

/// The album art fetch chain (design D5): the album's own `Primary` image
/// first (Emby metadata art), then the shared `AudioChild` child probe (first
/// track's embedded art) for albums without album-level art. One constructor
/// for every fetcher (hero projection, neighbour pre-warm) so the chain and
/// the `{id}:P` cache key cannot drift apart.
pub(in crate::app) fn music_album_image_chain() -> Vec<String> {
    let mut types = Vec::with_capacity(MUSIC_ALBUM_IMAGE_TYPES.len() + 1);
    types.push("Primary".to_string());
    types.extend(MUSIC_ALBUM_IMAGE_TYPES.iter().map(|s| s.to_string()));
    types
}

/// Music's image chain and cache key (design D5): albums try the album's own
/// `Primary` image first (Emby metadata art) and fall back to the shared
/// `AudioChild` child probe (first track's embedded art), all under the
/// album's `{id}:P` key; tracks the `Primary` chain under the album's key
/// when the album id is known (the queue-card convention), else the item's
/// own.
fn music_source(item: &EmbyItem) -> Option<ArtworkSource> {
    if item.id.is_empty() && item.album_id.is_empty() {
        return None;
    }
    if item.item_type == "MusicAlbum" {
        return Some(ArtworkSource::Emby {
            item_id: item.id.clone(),
            series_id: String::new(),
            image_types: music_album_image_chain(),
            cache_key: format!("{}:P", item.id),
        });
    }
    let key_scope = if item.album_id.is_empty() {
        &item.id
    } else {
        &item.album_id
    };
    Some(ArtworkSource::Emby {
        item_id: item.id.clone(),
        series_id: String::new(),
        image_types: vec!["Primary".into()],
        cache_key: format!("{}:P", key_scope),
    })
}

/// The landscape chain for a non-music item with a declared landscape image:
/// Series and episodes use the `Thumb`-first series chain; other videos keep
/// the movie hero's backdrop-first chain (design D5).
fn landscape_image_chain(item: &EmbyItem) -> &'static [&'static str] {
    match item.item_type.as_str() {
        "Series" | "Episode" => SERIES_LANDSCAPE_IMAGE_TYPES,
        _ => &["Backdrop", "Primary", "Logo"],
    }
}

/// Emby `ArtworkSource` for `chain`: Series keep the `:ser:`-infix key the
/// Series artwork family and the image-completion gate are keyed by; every
/// other item uses `{id}:{chain}`.
fn emby_source(item: &EmbyItem, chain: &[&str]) -> ArtworkSource {
    let image_types = chain.iter().map(|s| s.to_string()).collect();
    let cache_key = if item.item_type == "Series" {
        crate::app::images::series_image_cache_key(&item.id, chain)
    } else {
        format!("{}:{}", item.id, chain.join(","))
    };
    ArtworkSource::Emby {
        item_id: item.id.clone(),
        series_id: item.series_id.clone(),
        image_types,
        cache_key,
    }
}

// ── Hero producers (one per content type) ──────────────────────────────

/// The Emby item producer (design D5): plain title, plain meta rows (the
/// series line, release date, duration), cleaned overview, policy artwork.
pub(in crate::app) fn hero_content_emby(item: &EmbyItem) -> HeroContentData {
    let facts = HeroFacts {
        title: item.name.clone(),
        meta_rows: emby_hero_meta_rows_plain(item),
        artwork: emby_artwork_policy(item),
    };
    let overview = clean_overview(&item.overview);
    HeroContentData {
        facts,
        overview: (!overview.is_empty()).then_some(overview),
    }
}

/// The queue-item producer (design D5): dispatches to the item kind's
/// producer, so a queued item and its source item share one set of facts.
pub(in crate::app) fn hero_content_queue(item: &QueueItem) -> HeroContentData {
    match item {
        QueueItem::Emby(item) => hero_content_emby(item),
        QueueItem::Audiobookshelf(episode) => hero_content_abs_episode(episode),
        QueueItem::AudiobookshelfBook(book) => hero_content_abs_book(book),
        QueueItem::Feed(entry) => hero_content_feed(entry),
    }
}

/// The Audiobookshelf book producer (design D5): title, author, duration and
/// plain-text progress as meta rows. Every destination — Home rows included —
/// calls this producer for the same book, so Home's row and the Books tab
/// render one set of facts. The book's queue snapshot is the one input both
/// paths hold (the Books tab resolves its selection through
/// `audiobookshelf_book_queue_item`); narrator/year fields return with the
/// tab's conversion in task 10.1.
pub(in crate::app) fn hero_content_abs_book(book: &AudiobookshelfBookQueueItem) -> HeroContentData {
    let facts = HeroFacts {
        title: book.title.clone(),
        meta_rows: abs_book_meta_rows(book),
        artwork: abs_book_artwork_policy(book),
    };
    HeroContentData {
        facts,
        overview: None,
    }
}

fn abs_book_meta_rows(book: &AudiobookshelfBookQueueItem) -> Vec<String> {
    let mut rows = Vec::new();
    if let Some(author) = book.author.as_deref().filter(|a| !a.is_empty()) {
        rows.push(author.to_string());
    }
    if let Some(ticks) = book.duration_ticks.filter(|t| *t > 0) {
        rows.push(fmt_duration_approx(ticks as i64 / TICKS_PER_SECOND));
    }
    if book.is_finished {
        rows.push("Finished".into());
    } else if book.position_ticks > 0 && book.duration_ticks.is_some_and(|t| t > 0) {
        let pct = ((book.position_ticks as f64 * 100.0 / book.duration_ticks.unwrap() as f64)
            .floor() as u8)
            .clamp(1, 99);
        rows.push(format!("{pct}%"));
    }
    rows
}

/// The Audiobookshelf podcast episode producer (design D5): show and author
/// as plain meta rows, cleaned overview, Square cover.
pub(in crate::app) fn hero_content_abs_episode(
    episode: &AudiobookshelfQueueItem,
) -> HeroContentData {
    let mut meta_rows = Vec::new();
    if let Some(show) = episode.show_title.as_deref().filter(|s| !s.is_empty()) {
        meta_rows.push(show.to_string());
    }
    if let Some(author) = episode.author.as_deref().filter(|a| !a.is_empty()) {
        meta_rows.push(author.to_string());
    }
    if let Some(ticks) = episode.duration_ticks.filter(|t| *t > 0) {
        meta_rows.push(fmt_duration_approx(ticks as i64 / TICKS_PER_SECOND));
    }
    let facts = HeroFacts {
        title: episode.title.clone(),
        meta_rows,
        artwork: abs_episode_artwork_policy(episode),
    };
    let overview = episode
        .description
        .as_deref()
        .map(clean_overview)
        .filter(|d| !d.is_empty());
    HeroContentData { facts, overview }
}

/// The Audiobookshelf podcast show producer (design D5): author as a plain
/// meta row, cleaned overview, Square cover.
pub(in crate::app) fn hero_content_abs_show(
    show: &mbv_core::audiobookshelf::AudiobookshelfShow,
) -> HeroContentData {
    let facts = HeroFacts {
        title: show.title.clone(),
        meta_rows: show
            .author
            .as_deref()
            .filter(|a| !a.is_empty())
            .map(|a| vec![a.to_string()])
            .unwrap_or_default(),
        artwork: abs_show_artwork_policy(show),
    };
    let overview = show
        .description
        .as_deref()
        .map(clean_overview)
        .filter(|d| !d.is_empty());
    HeroContentData { facts, overview }
}

/// The feed entry producer (design D5): title and duration; no overview and
/// no artwork source exist for feed entries (design D5 keeps the
/// placeholder).
pub(in crate::app) fn hero_content_feed(entry: &FeedEntry) -> HeroContentData {
    let facts = HeroFacts {
        title: entry.title.clone(),
        meta_rows: entry
            .duration_ticks
            .filter(|t| *t > 0)
            .map(|ticks| vec![fmt_duration_approx(ticks as i64 / TICKS_PER_SECOND)])
            .unwrap_or_default(),
        artwork: feed_artwork_policy(entry),
    };
    HeroContentData {
        facts,
        overview: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::app::images::series_image_cache_key;
    use mbv_core::audiobookshelf::AudiobookshelfBook;
    use serde_json::json;

    // Policy table (task 5.4): parsed `EmbyItem`s from recorded item JSON
    // (the task 5.3 fixtures), ABS and feed content types as they exist.

    fn emby_item(raw: serde_json::Value) -> EmbyItem {
        mbv_core::api::parse_item(&raw)
    }

    fn shape(artwork: &HeroArtwork) -> ArtworkShape {
        artwork.shape
    }

    fn source(artwork: &HeroArtwork) -> &ArtworkSource {
        artwork.source.as_ref().expect("expected a source")
    }

    #[test]
    fn movie_with_backdrop_and_poster_is_landscape() {
        let item = emby_item(json!({
            "Id": "m1", "Name": "Dune", "Type": "Movie",
            "ImageTags": { "Primary": "poster" },
            "BackdropImageTags": ["backdrop-a"], "UserData": {}
        }));
        let artwork = emby_artwork_policy(&item);
        assert_eq!(shape(&artwork), ArtworkShape::Landscape);
        match source(&artwork) {
            ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
            } => {
                assert_eq!(item_id, "m1");
                assert_eq!(series_id, "");
                assert_eq!(image_types, &["Backdrop", "Primary", "Logo"]);
                assert_eq!(cache_key, "m1:Backdrop,Primary,Logo");
            }
            _ => panic!("expected Emby source"),
        }
    }

    #[test]
    fn movie_with_poster_only_is_portrait() {
        let item = emby_item(json!({
            "Id": "m2", "Name": "Poster only", "Type": "Movie",
            "ImageTags": { "Primary": "poster" }, "UserData": {}
        }));
        let artwork = emby_artwork_policy(&item);
        assert_eq!(shape(&artwork), ArtworkShape::Portrait);
        assert_eq!(
            source(&artwork),
            &ArtworkSource::Emby {
                item_id: "m2".into(),
                series_id: String::new(),
                image_types: vec!["Primary".into(), "Backdrop".into(), "Logo".into()],
                cache_key: "m2:Primary,Backdrop,Logo".into(),
            }
        );
    }

    #[test]
    fn album_is_always_square() {
        // Music is Square regardless of what else is declared (an album with
        // a thumb included).
        let item = emby_item(json!({
            "Id": "al1", "Name": "Album", "Type": "MusicAlbum",
            "ImageTags": { "Thumb": "thumb", "Primary": "cover" }, "UserData": {}
        }));
        let artwork = emby_artwork_policy(&item);
        assert_eq!(shape(&artwork), ArtworkShape::Square);
        match source(&artwork) {
            ArtworkSource::Emby {
                image_types,
                cache_key,
                ..
            } => {
                assert_eq!(image_types, &["Primary", "AudioChild"]);
                assert_eq!(cache_key, "al1:P");
            }
            _ => panic!("expected Emby source"),
        }
    }

    #[test]
    fn episode_with_series_landscape_tags_is_landscape() {
        let item = emby_item(json!({
            "Id": "e1", "Name": "Pilot", "Type": "Episode", "SeriesName": "Lost",
            "SeriesId": "s1",
            "ParentThumbImageTag": "series-thumb", "UserData": {}
        }));
        let artwork = emby_artwork_policy(&item);
        assert_eq!(shape(&artwork), ArtworkShape::Landscape);
        match source(&artwork) {
            ArtworkSource::Emby {
                series_id,
                image_types,
                cache_key,
                ..
            } => {
                assert_eq!(series_id, "s1");
                assert_eq!(
                    image_types,
                    &SERIES_LANDSCAPE_IMAGE_TYPES
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                );
                // Episodes do not use the `:ser:` Series namespace.
                assert_eq!(cache_key, "e1:Thumb,Primary,Backdrop,Logo");
            }
            _ => panic!("expected Emby source"),
        }
    }

    #[test]
    fn series_landscape_keeps_the_series_cache_key_namespace() {
        let item = emby_item(json!({
            "Id": "s1", "Name": "Lost", "Type": "Series", "UserData": {}
        }));
        // No tags declared: the Landscape fallback with no source.
        let artwork = emby_artwork_policy(&item);
        assert_eq!(shape(&artwork), ArtworkShape::Landscape);
        assert!(artwork.source.is_none());

        let item = emby_item(json!({
            "Id": "s1", "Name": "Lost", "Type": "Series",
            "ImageTags": { "Thumb": "thumb" }, "UserData": {}
        }));
        let artwork = emby_artwork_policy(&item);
        match source(&artwork) {
            ArtworkSource::Emby {
                image_types,
                cache_key,
                ..
            } => {
                assert_eq!(
                    image_types,
                    &SERIES_LANDSCAPE_IMAGE_TYPES
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    cache_key,
                    &series_image_cache_key("s1", SERIES_LANDSCAPE_IMAGE_TYPES)
                );
            }
            _ => panic!("expected Emby source"),
        }
    }

    #[test]
    fn item_without_declared_artwork_falls_back_to_landscape_placeholder() {
        let item =
            emby_item(json!({ "Id": "v1", "Name": "Video", "Type": "Movie", "UserData": {} }));
        let artwork = emby_artwork_policy(&item);
        assert_eq!(shape(&artwork), ArtworkShape::Landscape);
        assert!(artwork.source.is_none());
    }

    #[test]
    fn podcast_episode_is_square() {
        let artwork = abs_episode_artwork_policy(&episode_item(Some("cover")));
        assert_eq!(shape(&artwork), ArtworkShape::Square);
        assert_eq!(
            artwork.source,
            Some(ArtworkSource::AudiobookshelfCover {
                library_item_id: "lib-1".into(),
                book: false,
            })
        );
        // No cover path: Square placeholder.
        let artwork = abs_episode_artwork_policy(&episode_item(None));
        assert_eq!(shape(&artwork), ArtworkShape::Square);
        assert!(artwork.source.is_none());
    }

    #[test]
    fn book_is_portrait() {
        let book = book_item("book-1");
        let artwork = abs_book_artwork_policy(&book);
        assert_eq!(shape(&artwork), ArtworkShape::Portrait);
        assert_eq!(
            artwork.source,
            Some(ArtworkSource::AudiobookshelfCover {
                library_item_id: "book-1".into(),
                book: true,
            })
        );
        // No cover: Portrait placeholder.
        let mut no_cover = book_item("book-1");
        no_cover.cover_path = None;
        let artwork = abs_book_artwork_policy(&no_cover);
        assert_eq!(shape(&artwork), ArtworkShape::Portrait);
        assert!(artwork.source.is_none());
    }

    #[test]
    fn feed_entry_is_landscape_placeholder_and_podcast_feed_square() {
        // Legacy entries carry no kind and default to the video arm.
        let artwork = feed_artwork_policy(&feed_entry(None));
        assert_eq!(shape(&artwork), ArtworkShape::Landscape);
        assert!(artwork.source.is_none());
        let artwork = feed_artwork_policy(&feed_entry(Some(mbv_core::config::FeedKind::Audio)));
        assert_eq!(shape(&artwork), ArtworkShape::Square);
        assert!(artwork.source.is_none());
    }

    // ── Producers ──────────────────────────────────────────────────────

    #[test]
    fn emby_producer_fills_plain_meta_rows_and_overview() {
        let item = emby_item(json!({
            "Id": "s1", "Name": "Lost", "Type": "Series",
            "ProductionYear": 2004, "EndDate": "2010-05-23", "Genres": ["Adventure"],
            "Overview": "<b>The</b> survivors", "UserData": {}
        }));
        let produced = hero_content_emby(&item);
        assert_eq!(produced.facts.title, "Lost");
        assert_eq!(produced.facts.meta_rows, vec!["2004-2010  ADVENTURE"]);
        // The overview is trimmed plain text (clean_overview strips URLs, not
        // markup).
        assert_eq!(produced.overview.as_deref(), Some("<b>The</b> survivors"));
    }

    #[test]
    fn queue_producer_dispatches_by_kind() {
        let book = book_item("book-9");
        let produced = hero_content_queue(&QueueItem::AudiobookshelfBook(book.clone()));
        assert_eq!(produced, hero_content_abs_book(&book));
    }

    #[test]
    fn abs_episode_producer_rows_show_author_duration() {
        let mut episode = episode_item(Some("cover"));
        episode.show_title = Some("The Show".into());
        episode.author = Some("Host".into());
        episode.duration_ticks = Some(1_800 * TICKS_PER_SECOND as u64);
        let produced = hero_content_abs_episode(&episode);
        assert_eq!(produced.facts.title, "Ep 1");
        assert_eq!(produced.facts.meta_rows, vec!["The Show", "Host", "30m"]);
        // The overview is the cleaned plain text.
        assert_eq!(
            produced.overview.as_deref(),
            Some("<i>About</i> this episode".trim())
        );
        assert_eq!(produced.facts.artwork.shape, ArtworkShape::Square);
    }

    #[test]
    fn feed_producer_rows_and_policy_shape() {
        let entry = feed_entry(Some(mbv_core::config::FeedKind::Video));
        let produced = hero_content_feed(&entry);
        assert_eq!(produced.facts.title, "Entry 1");
        assert!(produced.overview.is_none());
        assert_eq!(produced.facts.artwork.shape, ArtworkShape::Landscape);
        assert!(produced.facts.artwork.source.is_none());
    }

    /// The Home path and the Books tab produce identical `HeroContent` for
    /// the same ABS book (task 5.4): the Books tab's selection resolves
    /// through `audiobookshelf_book_queue_item` (its queue-item conversion),
    /// and Home holds the same snapshot — both call the one ABS-book
    /// producer, so their `HeroContentData` is equal.
    #[test]
    fn home_path_and_books_tab_produce_identical_content_for_one_book() {
        use crate::app::audiobookshelf_browse_actions::audiobookshelf_book_queue_item;
        use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
        use mbv_core::audiobookshelf::{
            AudiobookshelfAudioFile, AudiobookshelfBookProgress, AudiobookshelfLibrary,
        };

        let mut state = AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Books".into(),
            media_type: "book".into(),
        });
        state.books.push(catalog_book("book-9"));
        state.selected_id = Some("book-9".into());
        state.detail_cache.insert(
            "book-9".into(),
            (
                Vec::new(),
                vec![AudiobookshelfAudioFile {
                    index: 0,
                    ino: "ino".into(),
                    duration: 3_600.0,
                }],
            ),
        );
        state.progress.insert(
            "book-9".into(),
            AudiobookshelfBookProgress {
                library_item_id: "book-9".into(),
                current_time_seconds: 1_800.0,
                is_finished: false,
            },
        );

        // Books-tab path: the tab's own selection conversion.
        let tab_item = audiobookshelf_book_queue_item(&state).expect("selected book");
        let books_tab = hero_content_queue(&tab_item);

        // Home path: the same book snapshot Home holds as a queue item.
        let home_item = book_item("book-9");
        let home = hero_content_queue(&QueueItem::AudiobookshelfBook(home_item));

        assert_eq!(books_tab, home);
        // And the content is the one producer's Portrait book facts.
        assert_eq!(books_tab.facts.title, "Book book-9");
        assert_eq!(
            books_tab.facts.meta_rows,
            vec!["Author book-9", "1h", "50%"]
        );
        assert_eq!(books_tab.facts.artwork.shape, ArtworkShape::Portrait);
    }

    // Fixtures.

    /// The catalog book the Books tab holds (mirrors `book_item`'s fields so
    /// the two paths describe the same book).
    fn catalog_book(id: &str) -> AudiobookshelfBook {
        AudiobookshelfBook {
            library_item_id: id.into(),
            title: format!("Book {id}"),
            author_display: Some(format!("Author {id}")),
            author_sort_key: format!("Author {id}"),
            cover_path: Some(format!("{id}-cover")),
            duration_seconds: 3_600.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        }
    }

    fn book_item(id: &str) -> AudiobookshelfBookQueueItem {
        AudiobookshelfBookQueueItem {
            library_item_id: id.into(),
            title: format!("Book {id}"),
            author: Some(format!("Author {id}")),
            duration_ticks: Some(3_600 * TICKS_PER_SECOND as u64),
            position_ticks: 1_800 * TICKS_PER_SECOND,
            played: false,
            is_finished: false,
            cover_path: Some(format!("{id}-cover")),
        }
    }

    fn episode_item(cover: Option<&str>) -> AudiobookshelfQueueItem {
        AudiobookshelfQueueItem {
            library_item_id: "lib-1".into(),
            episode_id: "ep-1".into(),
            title: "Ep 1".into(),
            show_title: None,
            author: None,
            description: Some("<i>About</i> this episode".into()),
            duration_ticks: Some(1_800 * TICKS_PER_SECOND as u64),
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: cover.map(|c| c.to_string()),
        }
    }

    fn feed_entry(kind: Option<mbv_core::config::FeedKind>) -> FeedEntry {
        FeedEntry {
            guid: "g".into(),
            title: "Entry 1".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: Some(600 * TICKS_PER_SECOND as u64),
            pub_date_secs: None,
            feed_kind: kind,
            feed_id: None,
            position_ticks: 0,
            played: false,
        }
    }
}
