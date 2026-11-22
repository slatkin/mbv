//! The Library Hero's plain artwork data and producers (design D3/D5, task
//! 1.7): the artwork shape the header reserves a box for, where that artwork
//! comes from, the hero facts both Clients read, the panel's retained image
//! paint, and one producer and artwork policy per content type. Plain data —
//! no painting, no styling primitive (`HeroFacts` carries no
//! `Span`/`Style`/width; the header painter owns colours, truncation and
//! wrapping, design D5), and no toolkit type.

use crate::geom::Rect;
use crate::meta::{MetaSegment, meta_row_text};
use crate::playback::HeroImageState;

pub mod producers;
#[doc(inline)]
pub use producers::{
    HeroContentData, HeroCredit, abs_book_artwork_policy, abs_episode_artwork_policy,
    emby_artwork_policy, emby_hero_meta_rows_plain, feed_artwork_policy, hero_content_abs_book,
    hero_content_abs_episode, hero_content_emby, hero_content_feed, hero_content_music_album,
    hero_content_queue, hero_content_series_with_episode, hero_content_twitch, music_album_artwork,
};

/// Canonical TV Wide Series image-type candidate chain. The artwork policy
/// reuses this chain for Series landscape arms.
pub const SERIES_LANDSCAPE_IMAGE_TYPES: &[&str] = &["Thumb", "Primary", "Backdrop", "Logo"];

/// The artwork shape the header reserves a box for (spec: the Wide Hero
/// header's three types). Chosen by the artwork policy (design D5, task
/// 5.4); never by a destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtworkShape {
    /// 16:9 artwork spanning the pane's content width above title/meta.
    Landscape,
    /// 1:1 artwork right of title/meta.
    Square,
    /// 2:3 artwork right of title/meta.
    Portrait,
}

/// Where a header's artwork comes from (design D3/D5). Built only by the
/// artwork policy (task 5.4); plain data — an image-type chain and a cache
/// key — so the shell projection (task 5.10) can fetch at projection time
/// while the painter reads projected state only. The Audiobookshelf arm
/// carries the Service-scoped library item id the projection resolves to its
/// server-scoped cache key (`App::audiobookshelf_{,book_}cover_key`); its
/// image type is the fixed `Primary` cover chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtworkSource {
    /// An Emby image: fetch walks `image_types` (priority order, e.g. `Thumb`
    /// then `Primary`) under `cache_key`, with `series_id` for the fetch's
    /// series-fallback input.
    Emby {
        item_id: String,
        series_id: String,
        image_types: Vec<String>,
        cache_key: String,
    },
    /// An Audiobookshelf cover (podcast episode/show `:cover:`, book
    /// `:bookcover:`).
    AudiobookshelfCover { library_item_id: String, book: bool },
}

/// Declared artwork for one hero (design D3/D5). `source: None` means no
/// artwork source exists for the item (Feeds entries, an item with no
/// declared image): the header renders the shared placeholder at the policy
/// shape's box size.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeroArtwork {
    pub shape: ArtworkShape,
    pub source: Option<ArtworkSource>,
    /// Optional semantic decoration source, independent from base artwork.
    pub decoration: Option<ArtworkSource>,
    /// Projected image state (task 5.10, design D9): set only by the shell's
    /// projection; the painters read it and never fetch. Producers build the
    /// default (`None`) and the owner copies the projection's state in.
    pub image: HeroImageState,
}

impl HeroArtwork {
    /// The shape of the artwork that will actually paint: the declared
    /// policy shape until the projection has decoded the fetched image,
    /// then the decoded source's own pixel-aspect class — wider than 5:4
    /// stays Landscape, near-square (between 4:5 and 5:4) is Square, and
    /// anything taller is Portrait. A declared landscape image the provider
    /// cannot serve (the fetch chain falls through to the `Primary` poster)
    /// therefore re-arms the side-by-side layout instead of painting a
    /// portrait poster — uncropped in the overlay, cover-cropped in Wide —
    /// inside a 16:9 box; genuinely 16:9 artwork keeps the Landscape arm.
    #[must_use]
    pub fn painted_shape(&self) -> ArtworkShape {
        let decoded = match &self.image {
            HeroImageState::Ready { decoded, .. } => *decoded,
            _ => None,
        };
        let Some((w, h)) = decoded else {
            return self.shape;
        };
        painted_shape_for_pixels(w, h)
    }
}

/// The arm a decoded image's own pixel size paints (design D5): wider than 5:4
/// stays Landscape, near-square (between 4:5 and 5:4) is Square, and anything
/// taller is Portrait. The one rule [`HeroArtwork::painted_shape`] and the
/// GUI Library's Hero projection both apply, so a Client that reads its own
/// image cache re-arms the layout by the same arithmetic.
#[must_use]
pub fn painted_shape_for_pixels(width: u32, height: u32) -> ArtworkShape {
    if width * 4 >= height * 5 {
        ArtworkShape::Landscape
    } else if width * 5 > height * 4 {
        ArtworkShape::Square
    } else {
        ArtworkShape::Portrait
    }
}

/// Plain hero facts (design D3): one title, ordered plain-text meta rows
/// (coloured by position by the panel, never styled by the destination), the
/// meta row holding the duration (painted the `DURATION` role instead of its
/// cycle colour), the meta row holding a resume percentage (painted the
/// `PROGRESS_PERCENT` role instead of its cycle colour), and the policy-built
/// artwork.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeroFacts {
    pub title: String,
    /// Typed segments per meta row; the TUI paints each row's joined text.
    pub meta_rows: Vec<Vec<MetaSegment>>,
    pub duration_row: Option<usize>,
    pub progress_row: Option<usize>,
    pub links: Vec<HeroLink>,
    pub artwork: HeroArtwork,
}

impl HeroFacts {
    /// Title followed by meta rows, in display order — the raw entry
    /// sequence the Landscape two-column grid packs and positions against
    /// (`hero_header.rs`, `overview_box.rs`).
    pub fn entries(&self) -> impl Iterator<Item = String> + '_ {
        std::iter::once(self.title.clone())
            .chain(self.meta_rows.iter().map(|row| meta_row_text(row)))
    }

    /// [`Self::entries`] with empty rows skipped.
    pub fn live_entries(&self) -> impl Iterator<Item = String> + '_ {
        self.entries().filter(|line| !line.is_empty())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeroLink {
    pub name: String,
    pub url: String,
}

/// The hero image paint the panel retained from one view (task 5.10, design
/// D9): the projected protocol's cache key and the box rect it paints into.
/// The shell paints it right after `view` returns — the same
/// defer-the-pixel-paint seam every destination component already uses
/// (`take_image_paint`); the painters themselves only read projected state
/// and never fetch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanelHeroImagePaint {
    /// The artwork box the painter reserved for the projected image.
    pub area: Rect,
    /// The projected cache key (`HeroImageState::Ready`'s key).
    pub cache_key: String,
    /// `true` for a centered artwork box; `false` for right-aligned placement.
    pub centered: bool,
}
