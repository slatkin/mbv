//! Row-sized Library art (gui-delineated-rows design D4, D5, D6, D7): the art
//! state one top-level row paints, the source the row's Hero artwork policy
//! declares, and the cache read and `Row` request that source resolves to.
//!
//! A row shows the same image as its item's Hero, at a size near the painted
//! thumb (design D5). A projection never fetches: every frame reads the cache
//! (`App::gui_row_art`), and only the in-view path (`super::super::rows_in_view`)
//! queues a `Row` fetch.

use mbv_audiobookshelf::{AudiobookshelfBook, AudiobookshelfShow};
use mbv_components::library_panel::hero::{
    abs_book_artwork_policy, abs_episode_artwork_policy, emby_artwork_policy, feed_artwork_policy,
};
use mbv_emby_model::{EmbyImageTags, EmbyItem, UpcomingPremiere};
use mbv_images::ImageFetchReq;
use mbv_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry, FeedKind,
    QueueItem,
};
use mbv_ui_model::library_hero::{
    ArtworkShape, ArtworkSource, HeroArtwork, painted_shape_for_pixels,
};
use mbv_ui_model::music_grouping::ArtistKey;

use crate::app::App;

#[cfg(test)]
mod tests;

/// The media kind a row paints when it shows no image (design D4): the kind
/// icon of the missing and the no-source state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiRowMediaKind {
    Audio,
    Video,
}

/// The art one top-level Library row shows (design D4): its row-sized cached
/// image, a fetch in flight, a source that gave no image, or a row kind with
/// no art source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiRowArt {
    /// The row's cached image under its row cache key. `square` comes from the
    /// image's own pixels: a square image shows whole, and landscape and
    /// portrait images cover-fill the painted box.
    Image { key: String, square: bool },
    /// The row's image is not cached yet: its fetch is in flight, awaits its
    /// retry, or has not started because the row is out of view.
    Loading,
    /// The row's source answered with no usable image. Final.
    Missing,
    /// The row's kind declares no art source. Final, and no request is made.
    NoSource,
}

/// One row's image source (design D5, D6): what the row's Hero artwork policy
/// declares, in the terms a `Row` request and its cache key need.
#[derive(Clone, Debug, PartialEq)]
pub enum GuiRowSource {
    /// An Emby item's row chain: the addressed item, the series addressing the
    /// chain reads, the tags the item declares, and whether the declared arm
    /// is square.
    Emby {
        item_id: String,
        series_id: String,
        tags: EmbyImageTags,
        square: bool,
    },
    /// An Emby item's own `Primary` image, square (a Music artist).
    EmbyPrimary { item_id: String },
    /// An Audiobookshelf cover: the library item's id, whether it is a book,
    /// and the item's last update time in milliseconds (design D6).
    Audiobookshelf {
        library_item_id: String,
        book: bool,
        updated_at_ms: u64,
    },
}

/// The row source of an Emby item's row (design D5): the item's Hero artwork
/// policy, so the row shows the image the item's Hero shows. A policy that
/// declares no source makes no request.
#[must_use]
pub(in crate::app::gui_library) fn emby_row_source(item: &EmbyItem) -> Option<GuiRowSource> {
    let policy = emby_artwork_policy(item);
    emby_policy_source(&policy, item)
}

/// The row source an Emby item's Hero artwork policy declares: the policy's
/// declared arm sizes the request and its chain addresses the image. A policy
/// with no Emby source — the placeholder arms — declares none.
fn emby_policy_source(policy: &HeroArtwork, item: &EmbyItem) -> Option<GuiRowSource> {
    match policy.source {
        Some(ArtworkSource::Emby { .. }) => Some(emby_source(
            &item.id,
            &item.series_id,
            &item.image_tags,
            policy.shape,
        )),
        Some(ArtworkSource::AudiobookshelfCover { .. }) | None => None,
    }
}

/// The row source of a Shows Upcoming row (design D5): the premiere's series
/// image, addressed to the series and tagged by the series item's own declared
/// images. A premiere whose series the Shows list does not carry requests the
/// chain untagged, as an item with no declared tag does.
#[must_use]
pub(in crate::app::gui_library) fn upcoming_row_source(
    premiere: &UpcomingPremiere,
    series: Option<&EmbyItem>,
) -> GuiRowSource {
    let tags = series.map_or_else(EmbyImageTags::default, |item| item.image_tags.clone());
    emby_source(
        premiere.series_id.as_str(),
        "",
        &tags,
        ArtworkShape::Landscape,
    )
}

/// The row source of a Music artist row (design D5): the artist's own `Primary`
/// image, requested untagged because the artist row frame declares no tags.
/// Only a Service-named artist has one; the fallback identity has no source.
#[must_use]
pub(in crate::app::gui_library) fn artist_row_source(artist_id: &str) -> GuiRowSource {
    GuiRowSource::EmbyPrimary {
        item_id: artist_id.to_owned(),
    }
}

/// The prefix a Service-named Music artist's `RowsInView` id carries (design
/// D7), so the core tells the Service identity from the fallback name.
pub(in crate::app::gui_library) const ARTIST_ROW_ID_PREFIX: &str = "artist:";

/// The prefix a fallback Music artist's `RowsInView` id carries: the name the
/// albums were grouped by, which the core maps to a Service artist item id.
pub(in crate::app::gui_library) const ARTIST_NAME_ROW_ID_PREFIX: &str = "artist-name:";

/// The id a Music artist row takes in `RowsInView` (design D7): the Service
/// identity the row was grouped by, or the fallback name when the Service
/// names no artist.
#[must_use]
pub fn artist_rows_in_view_id(artist: &ArtistKey) -> String {
    match artist {
        ArtistKey::Service(id) => format!("{ARTIST_ROW_ID_PREFIX}{id}"),
        ArtistKey::Fallback(name) => format!("{ARTIST_NAME_ROW_ID_PREFIX}{name}"),
    }
}

/// The row source a Music artist row's `RowsInView` id names (design D5): the
/// Service artist's own image, and none for the fallback identity.
#[must_use]
pub(in crate::app::gui_library) fn artist_id_row_source(id: &str) -> Option<GuiRowSource> {
    let artist_id = id.strip_prefix(ARTIST_ROW_ID_PREFIX)?;
    (!artist_id.is_empty()).then(|| artist_row_source(artist_id))
}

/// The row source of an Audiobookshelf book (design D5): the book artwork
/// policy's cover, keyed by the catalog record's last update time.
#[must_use]
pub(in crate::app::gui_library) fn book_row_source(
    item: &AudiobookshelfBookQueueItem,
    updated_at_ms: u64,
) -> Option<GuiRowSource> {
    policy_row_source(&abs_book_artwork_policy(item), updated_at_ms)
}

/// The row source of an Audiobookshelf podcast episode (design D5): the episode
/// artwork policy's show cover, keyed by the show's last update time.
#[must_use]
pub(in crate::app::gui_library) fn episode_row_source(
    item: &AudiobookshelfQueueItem,
    updated_at_ms: u64,
) -> Option<GuiRowSource> {
    policy_row_source(&abs_episode_artwork_policy(item), updated_at_ms)
}

/// The row source of a feed entry (design D5): the feed artwork policy declares
/// no source, so a Feeds row paints its media kind and makes no request.
#[must_use]
pub(in crate::app::gui_library) fn feed_row_source(entry: &FeedEntry) -> Option<GuiRowSource> {
    policy_row_source(&feed_artwork_policy(entry), 0)
}

/// The row source of a catalog record's cover (design D5): `None` when the
/// record declares no cover, else its library item keyed by its update time.
fn catalog_cover_row_source(
    library_item_id: &str,
    cover_path: Option<&str>,
    book: bool,
    updated_at_ms: u64,
) -> Option<GuiRowSource> {
    cover_path
        .is_some_and(|cover| !cover.is_empty())
        .then(|| GuiRowSource::Audiobookshelf {
            library_item_id: library_item_id.to_owned(),
            book,
            updated_at_ms,
        })
}

/// The row source of a book catalog record (design D5), for a caller that holds
/// the record rather than its queue item: the same book artwork policy over the
/// record's cover, keyed by its update time.
#[must_use]
pub(in crate::app::gui_library) fn catalog_book_row_source(
    book: &AudiobookshelfBook,
) -> Option<GuiRowSource> {
    catalog_cover_row_source(
        &book.library_item_id,
        book.cover_path.as_deref(),
        true,
        book.updated_at_ms,
    )
}

/// The row source of a podcast show's catalog record (design D5), for a caller
/// that holds the record rather than an episode's queue item: the episode
/// artwork policy names the parent show's cover, so the show's own record keys
/// the row. A show that declares no cover names no source.
#[must_use]
pub(in crate::app::gui_library) fn catalog_show_row_source(
    show: &AudiobookshelfShow,
) -> Option<GuiRowSource> {
    catalog_cover_row_source(
        &show.library_item_id,
        show.cover_path.as_deref(),
        false,
        show.updated_at_ms,
    )
}

/// The media kind an Emby item's row shows without art (design D4).
#[must_use]
pub(in crate::app::gui_library) fn emby_media_kind(item: &EmbyItem) -> GuiRowMediaKind {
    if item.is_audio() {
        GuiRowMediaKind::Audio
    } else {
        GuiRowMediaKind::Video
    }
}

/// The media kind a feed entry's row shows without art (design D4): the
/// declared `FeedKind::Audio` is audio, and every other entry is video.
#[must_use]
pub(in crate::app::gui_library) fn feed_media_kind(entry: &FeedEntry) -> GuiRowMediaKind {
    if matches!(entry.feed_kind, Some(FeedKind::Audio)) {
        GuiRowMediaKind::Audio
    } else {
        GuiRowMediaKind::Video
    }
}

/// The media kind a queue item's row shows without art (design D4): an Emby
/// item's own kind, an Audiobookshelf book or episode's audio, a feed entry's
/// declared kind, and a Twitch stream's video.
#[must_use]
pub(in crate::app::gui_library) fn queue_item_media_kind(item: &QueueItem) -> GuiRowMediaKind {
    match item {
        QueueItem::Emby(emby) => emby_media_kind(emby),
        QueueItem::Feed(entry) => feed_media_kind(entry),
        QueueItem::Audiobookshelf(_) => GuiRowMediaKind::Audio,
        QueueItem::Twitch(_) => GuiRowMediaKind::Video,
    }
}

impl App {
    /// The art `source` resolves to for one row (design D4, D7): the cache
    /// read alone, so a cached row out of view still shows its image. A source
    /// with no row cache key — the Service setup the key needs is absent —
    /// shows no image, and a source that declares none is the no-source state.
    #[must_use]
    pub fn gui_row_art(&self, source: Option<&GuiRowSource>) -> GuiRowArt {
        let Some(source) = source else {
            return GuiRowArt::NoSource;
        };
        let Some(key) = self.gui_row_cache_key(source) else {
            return GuiRowArt::Missing;
        };
        let Some(entry) = self.images.image(&key) else {
            return GuiRowArt::Loading;
        };
        if !entry.has_art() {
            return GuiRowArt::Missing;
        }
        GuiRowArt::Image {
            square: entry.dimensions().is_some_and(|(width, height)| {
                painted_shape_for_pixels(width, height) == ArtworkShape::Square
            }),
            key,
        }
    }

    /// The row cache key `source` names (design D6), or `None` when the Service
    /// setup the key needs is absent.
    fn gui_row_cache_key(&self, source: &GuiRowSource) -> Option<String> {
        match source {
            GuiRowSource::Emby {
                item_id,
                series_id,
                tags,
                square,
            } => Some(
                mbv_images::row::emby_row_request_for(item_id, series_id, tags, *square).cache_key,
            ),
            GuiRowSource::EmbyPrimary { item_id } => {
                Some(mbv_images::row::emby_row_primary_request(item_id).cache_key)
            }
            GuiRowSource::Audiobookshelf {
                library_item_id,
                updated_at_ms,
                ..
            } => self
                .audiobookshelf_server_url()
                .map(|url| mbv_images::abs_row_cache_key(&url, library_item_id, *updated_at_ms)),
        }
    }

    /// The `Row` request that fills `source` (design D5, D6), or `None` when the
    /// Service setup it needs is absent.
    pub(in crate::app::gui_library) fn gui_row_fetch_req(
        &self,
        source: &GuiRowSource,
    ) -> Option<ImageFetchReq> {
        match source {
            GuiRowSource::Emby {
                item_id,
                series_id,
                tags,
                square,
            } => Some(mbv_images::row::emby_row_request_for(
                item_id, series_id, tags, *square,
            )),
            GuiRowSource::EmbyPrimary { item_id } => {
                Some(mbv_images::row::emby_row_primary_request(item_id))
            }
            GuiRowSource::Audiobookshelf {
                library_item_id,
                book,
                updated_at_ms,
            } => self.audiobookshelf_row_request(library_item_id, *book, *updated_at_ms),
        }
    }
}

impl App {
    /// The row art source of a queue item's row (design D5): its provider's
    /// Hero artwork policy, with the catalog's update time for an
    /// Audiobookshelf cover.
    #[must_use]
    pub(in crate::app::gui_library) fn queue_item_row_source(
        &self,
        item: &QueueItem,
    ) -> Option<GuiRowSource> {
        match item {
            QueueItem::Emby(emby) => emby_row_source(emby),
            QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(episode)) => {
                let updated_at = self.audiobookshelf_updated_at_ms(&episode.library_item_id);
                episode_row_source(episode, updated_at)
            }
            QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => {
                let updated_at = self.audiobookshelf_updated_at_ms(&book.library_item_id);
                book_row_source(book, updated_at)
            }
            QueueItem::Feed(entry) => feed_row_source(entry),
            QueueItem::Twitch(_) => None,
        }
    }

    /// The last update time the browse catalogs hold for a library item (design
    /// D6), or `0` when no catalog carries it — the row key's unknown arm.
    pub(in crate::app::gui_library) fn audiobookshelf_updated_at_ms(
        &self,
        library_item_id: &str,
    ) -> u64 {
        self.audiobookshelf_browse
            .iter()
            .flat_map(|state| state.shows.iter())
            .find(|show| show.library_item_id == library_item_id)
            .map(|show| show.updated_at_ms)
            .or_else(|| {
                self.audiobookshelf_book_browse
                    .iter()
                    .flat_map(|state| state.books.iter())
                    .find(|book| book.library_item_id == library_item_id)
                    .map(|book| book.updated_at_ms)
            })
            .unwrap_or(0)
    }
}

/// One Emby row source, from the item's address and the tags it declares.
fn emby_source(
    item_id: &str,
    series_id: &str,
    tags: &EmbyImageTags,
    shape: ArtworkShape,
) -> GuiRowSource {
    GuiRowSource::Emby {
        item_id: item_id.to_owned(),
        series_id: series_id.to_owned(),
        tags: tags.clone(),
        square: painted_square(shape),
    }
}

/// The row source an Audiobookshelf cover policy declares: the book and
/// episode policies both name the Service-scoped cover, and the catalog's last
/// update time keys the row's bytes. A policy with no cover declares none.
fn policy_row_source(policy: &HeroArtwork, updated_at_ms: u64) -> Option<GuiRowSource> {
    match &policy.source {
        Some(ArtworkSource::AudiobookshelfCover {
            library_item_id,
            book,
        }) => Some(GuiRowSource::Audiobookshelf {
            library_item_id: library_item_id.clone(),
            book: *book,
            updated_at_ms,
        }),
        Some(ArtworkSource::Emby { .. }) | None => None,
    }
}

/// Whether a declared artwork arm asks for a square row image (design D6):
/// only `Square` does; `Landscape` and `Portrait` both cover-fill the painted
/// box at one width.
fn painted_square(shape: ArtworkShape) -> bool {
    shape == ArtworkShape::Square
}
