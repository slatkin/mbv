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
use crate::app::ui_util::{clean_overview, fmt_duration_hms, fmt_publish_date};

use super::content::{
    ArtworkShape, ArtworkSource, HeroArtwork, HeroCredit, HeroFacts, HeroImageState, HeroLink,
};

/// A producer's output (design D5): the facts plus the item's overview.
/// Destinations attach a `Workspace` when assembling [`HeroContent`]; the
/// header painter and Library Hero overlay both consume this.
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
    if item.item_type != "Movie" || item.image_tags.logo.is_empty() {
        return None;
    }
    let mut source = emby_source(item, &["Logo"]);
    // Unlike base artwork, a Logo is independently addressed by its provider
    // declaration. This prevents a changed Logo tag from reusing stale bytes.
    if let ArtworkSource::Emby { cache_key, .. } = &mut source {
        *cache_key = format!("{}:Logo:{}", item.id, item.image_tags.logo);
    }
    Some(source)
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
            .map(ToString::to_string)
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
        cache_key: format!("{key_scope}:P"),
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
/// the movie hero's backdrop-first chain with `Thumb` in front (design D5).
/// `Thumb` is load-bearing here: this arm is also reached by a `Thumb`-only
/// declaration (`landscape_declared` counts `ImageTags.Thumb`), and a home
/// video's landscape image is commonly a `Thumb` with no Primary/Backdrop at
/// all — omitting it left those items fetching nothing and painting the
/// placeholder while the artwork existed.
fn landscape_image_chain(item: &EmbyItem) -> &'static [&'static str] {
    match item.item_type.as_str() {
        "Series" | "Episode" => SERIES_LANDSCAPE_IMAGE_TYPES,
        "Movie" => &["Backdrop", "Primary"],
        _ => &["Thumb", "Backdrop", "Primary", "Logo"],
    }
}

/// Emby `ArtworkSource` for `chain`: Series keep the `:ser:`-infix key the
/// Series artwork family and the image-completion gate are keyed by; every
/// other item uses `{id}:{chain}`.
fn emby_source(item: &EmbyItem, chain: &[&str]) -> ArtworkSource {
    let image_types = chain.iter().map(ToString::to_string).collect();
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
    let (mut meta_rows, duration_row) = emby_hero_meta_rows_plain(item);
    if movie {
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
            duration_row,
            progress_row: None,
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
        duration_row,
        progress_row: None,
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
        QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Episode(
            episode,
        )) => hero_content_abs_episode(episode),
        QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Book(book)) => {
            hero_content_abs_book(book)
        }
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
    let (meta_rows, duration_row, progress_row) = abs_book_meta_rows(book);
    let facts = HeroFacts {
        title: book.title.clone(),
        meta_rows,
        duration_row,
        progress_row,
        links: Vec::new(),
        artwork: abs_book_artwork_policy(book),
    };
    HeroContentData {
        facts,
        overview: None,
        credits: None,
    }
}

fn abs_book_meta_rows(
    book: &AudiobookshelfBookQueueItem,
) -> (Vec<String>, Option<usize>, Option<usize>) {
    let mut rows = Vec::new();
    let mut duration_row = None;
    let mut progress_row = None;
    if let Some(author) = book.author.as_deref().filter(|a| !a.is_empty()) {
        rows.push(author.to_string());
    }
    if let Some(ticks) = book.duration_ticks.filter(|t| *t > 0) {
        duration_row = Some(rows.len());
        rows.push(fmt_duration_hms(
            i64::try_from(ticks / TICKS_PER_SECOND as u64).unwrap_or(i64::MAX),
        ));
    }
    if book.is_finished {
        rows.push("Finished".into());
    } else if book.position_ticks > 0 {
        if let Some(duration_ticks) = book.duration_ticks.filter(|ticks| *ticks > 0) {
            let numerator = book.position_ticks.checked_mul(100).unwrap_or(i64::MAX);
            let denominator = i64::try_from(duration_ticks).unwrap_or(i64::MAX);
            let percent = crate::app::render::components::math::int_ratio(numerator, denominator)
                .floor()
                .clamp(1.0, 99.0);
            let pct = format!("{percent:.0}").parse::<u8>().unwrap_or(u8::MAX);
            progress_row = Some(rows.len());
            rows.push(format!("{pct}%"));
        }
    }
    (rows, duration_row, progress_row)
}

/// The Audiobookshelf podcast episode producer (design D7, row 3.5): the
/// episode title, then the parent show's name, duration and publish date as
/// plain meta rows, the cleaned episode description as the overview, and the
/// parent show's Square cover. There is no credits block and no author row:
/// no such field exists on the episode payload. One producer for every
/// destination that presents the episode (podcast tab, queue panel, Home
/// rows), so the facts cannot drift.
pub(in crate::app) fn hero_content_abs_episode(
    episode: &AudiobookshelfQueueItem,
) -> HeroContentData {
    let mut meta_rows = Vec::new();
    let mut duration_row = None;
    if let Some(show) = episode.show_title.as_deref().filter(|s| !s.is_empty()) {
        meta_rows.push(show.to_string());
    }
    if let Some(ticks) = episode.duration_ticks.filter(|t| *t > 0) {
        duration_row = Some(meta_rows.len());
        meta_rows.push(fmt_duration_hms(
            i64::try_from(ticks / TICKS_PER_SECOND as u64).unwrap_or(i64::MAX),
        ));
    }
    if let Some(secs) = episode.pub_date_secs {
        meta_rows.push(fmt_publish_date(secs));
    }
    let facts = HeroFacts {
        title: episode.title.clone(),
        meta_rows,
        duration_row,
        progress_row: None,
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

/// The feed entry producer (design D5): title and duration; no overview and
/// no artwork source exist for feed entries (design D5 keeps the
/// placeholder).
pub(in crate::app) fn hero_content_feed(entry: &FeedEntry) -> HeroContentData {
    let has_duration = entry.duration_ticks.is_some_and(|t| t > 0);
    let facts = HeroFacts {
        title: entry.title.clone(),
        meta_rows: entry
            .duration_ticks
            .filter(|t| *t > 0)
            .map(|ticks| {
                vec![fmt_duration_hms(
                    i64::try_from(ticks / TICKS_PER_SECOND as u64).unwrap_or(i64::MAX),
                )]
            })
            .unwrap_or_default(),
        duration_row: has_duration.then_some(0),
        progress_row: None,
        links: Vec::new(),
        artwork: feed_artwork_policy(entry),
    };
    HeroContentData {
        facts,
        overview: None,
        credits: None,
    }
}
