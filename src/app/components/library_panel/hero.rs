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

use super::content::{
    ArtworkShape, ArtworkSource, HeroArtwork, HeroCredit, HeroFacts, HeroImageState, HeroLink,
};

/// A producer's output (design D5): the facts plus the item's overview.
/// Destinations attach a `Workspace` when assembling [`HeroContent`]; the
/// header painter and the Narrow inline hero (task 5.7) both consume this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroContentData {
    pub facts: HeroFacts,
    pub overview: Option<String>,
    pub credits: Option<Vec<HeroCredit>>,
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
            decoration: None,
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
            decoration: None,
            image: HeroImageState::None,
        };
    }
    if landscape_declared {
        HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: Some(emby_source(item, landscape_image_chain(item))),
            decoration: movie_logo_source(item),
            image: HeroImageState::None,
        }
    } else if poster_declared {
        HeroArtwork {
            shape: ArtworkShape::Portrait,
            source: Some(emby_source(item, &["Primary", "Backdrop"])),
            decoration: None,
            image: HeroImageState::None,
        }
    } else {
        HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            decoration: None,
            image: HeroImageState::None,
        }
    }
}

fn movie_logo_source(item: &EmbyItem) -> Option<ArtworkSource> {
    (item.item_type == "Movie" && !item.image_tags.logo.is_empty())
        .then(|| emby_source(item, &["Logo"]))
}

/// The artwork policy for one queue item: dispatches to the item kind's
/// policy (Emby, Audiobookshelf podcast episode, book, or feed entry).
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
        decoration: None,
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
        decoration: None,
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
        decoration: None,
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
        decoration: None,
        image: HeroImageState::None,
    }
}

/// The shared album art source (design D5): the `AudioChild` chain under the
/// album's `{id}:P` key. One constructor for every album-shaped caller — the
/// typed `MusicAlbum` arm here and grouped Music's `music_album_artwork`
/// (whose rows are Emby `Folder` items) — so the chain and key cannot drift.
fn album_source(item: &EmbyItem) -> Option<ArtworkSource> {
    if item.id.is_empty() {
        return None;
    }
    Some(ArtworkSource::Emby {
        item_id: item.id.clone(),
        series_id: String::new(),
        image_types: MUSIC_ALBUM_IMAGE_TYPES
            .iter()
            .map(|s| s.to_string())
            .collect(),
        cache_key: format!("{}:P", item.id),
    })
}

/// Music's image chain and cache key (design D5): albums use the shared
/// `AudioChild` album chain under the album's `{id}:P` key; tracks the
/// `Primary` chain under the album's key when the album id is known (the
/// queue-card convention), else the item's own.
fn music_source(item: &EmbyItem) -> Option<ArtworkSource> {
    if item.item_type == "MusicAlbum" {
        return album_source(item);
    }
    if item.id.is_empty() && item.album_id.is_empty() {
        return None;
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

/// The artwork policy for a grouped-Music album row (design D5): always
/// Square, always the shared album art chain under the album's `{id}:P` key.
/// Album rows in a folder-view music library (`[library.music] levels =
/// ["group", "album"]`) are Emby `Folder` items with no `MusicAlbum`/audio
/// type, so `emby_artwork_policy`'s type dispatch cannot recognise them: the
/// Music destination knows its rows are albums and asks for the music arm
/// directly (the pre-panel music painters' unconditional chain).
pub(in crate::app) fn music_album_artwork(item: &EmbyItem) -> HeroArtwork {
    HeroArtwork {
        shape: ArtworkShape::Square,
        source: album_source(item),
        decoration: None,
        image: HeroImageState::None,
    }
}

/// The landscape chain for a non-music item with a declared landscape image:
/// Series and episodes use the `Thumb`-first series chain; other videos keep
/// the movie hero's backdrop-first chain (design D5).
fn landscape_image_chain(item: &EmbyItem) -> &'static [&'static str] {
    match item.item_type.as_str() {
        "Series" | "Episode" => SERIES_LANDSCAPE_IMAGE_TYPES,
        "Movie" => &["Backdrop", "Primary"],
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
    let movie = item.item_type == "Movie";
    let mut meta_rows = emby_hero_meta_rows_plain(item);
    if movie {
        let genres = item
            .genres
            .iter()
            .filter(|genre| !genre.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("/");
        if !genres.is_empty() {
            meta_rows.push(genres);
        }
        let links = item
            .external_urls
            .iter()
            .filter(|link| link.name == "IMDb")
            .take(1)
            .map(|link| HeroLink {
                name: link.name.clone(),
                url: link.url.clone(),
            })
            .collect::<Vec<_>>();
        if !links.is_empty() {
            meta_rows.push(
                links
                    .iter()
                    .map(|link| link.name.as_str())
                    .collect::<Vec<_>>()
                    .join("|"),
            );
        }
        let mut credits = Vec::new();
        credits.extend(
            item.people
                .iter()
                .filter(|person| person.kind == "Director")
                .map(|person| HeroCredit {
                    name: person.name.clone(),
                    role: if person.role.is_empty() {
                        person.kind.clone()
                    } else {
                        person.role.clone()
                    },
                }),
        );
        credits.extend(
            item.people
                .iter()
                .filter(|person| person.kind != "Director")
                .map(|person| HeroCredit {
                    name: person.name.clone(),
                    role: if person.role.is_empty() {
                        person.kind.clone()
                    } else {
                        person.role.clone()
                    },
                }),
        );
        let credits = (!credits.is_empty()).then_some(credits);
        let facts = HeroFacts {
            title: item.name.clone(),
            meta_rows,
            links,
            artwork: emby_artwork_policy(item),
        };
        let overview = clean_overview(&item.overview);
        return HeroContentData {
            facts,
            overview: (!overview.is_empty()).then_some(overview),
            credits,
        };
    }
    let facts = HeroFacts {
        title: item.name.clone(),
        meta_rows,
        links: Vec::new(),
        artwork: emby_artwork_policy(item),
    };
    let overview = clean_overview(&item.overview);
    HeroContentData {
        facts,
        overview: (!overview.is_empty()).then_some(overview),
        credits: None,
    }
}

/// The Music album producer (design D5): the `EmbyItem` producer's facts with
/// the album arm's artwork (`music_album_artwork`) instead of the type-based
/// policy. Grouped Music's album rows are Emby `Folder` items, so their
/// artwork cannot be derived from `item_type`.
pub(in crate::app) fn hero_content_music_album(item: &EmbyItem) -> HeroContentData {
    let mut data = hero_content_emby(item);
    data.facts.artwork = music_album_artwork(item);
    data
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
        links: Vec::new(),
        artwork: abs_book_artwork_policy(book),
    };
    HeroContentData {
        facts,
        overview: None,
        credits: None,
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
        links: Vec::new(),
        artwork: abs_episode_artwork_policy(episode),
    };
    let overview = episode
        .description
        .as_deref()
        .map(clean_overview)
        .filter(|d| !d.is_empty());
    HeroContentData {
        facts,
        overview,
        credits: None,
    }
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
        links: Vec::new(),
        artwork: abs_show_artwork_policy(show),
    };
    let overview = show
        .description
        .as_deref()
        .map(clean_overview)
        .filter(|d| !d.is_empty());
    HeroContentData {
        facts,
        overview,
        credits: None,
    }
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
        links: Vec::new(),
        artwork: feed_artwork_policy(entry),
    };
    HeroContentData {
        facts,
        overview: None,
        credits: None,
    }
}

#[cfg(test)]
#[path = "hero_tests.rs"]
mod hero_tests;
