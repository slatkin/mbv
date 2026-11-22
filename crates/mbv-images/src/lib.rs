//! Image cache, loading, and pixel processing.
//!
//! The `ImageCache` and per-Service cache-key helpers live here. It decodes and
//! caches pixels; the terminal image protocol and its placement live in
//! `mbv-tui-support`, and the Library and Queue panels own painting.

pub mod cache;
pub mod row;
pub mod title_overlay;
pub const QUEUE_CARD_PLACEHOLDER_KEY: &str = "__power_card_placeholder__";
pub static QUEUE_CARD_PLACEHOLDER_BYTES: &[u8] =
    include_bytes!("../../../assets/power-card-placeholder.webp");

#[must_use]
pub fn mem_key(cache_key: &str, suffix: &str) -> String {
    format!("{cache_key}@{suffix}")
}

/// Prefix shared by every Audiobookshelf-sourced cache key, used to filter
/// or clear Audiobookshelf entries from the image caches.
pub const AUDIOBOOKSHELF_CACHE_KEY_PREFIX: &str = "audiobookshelf:";

/// A cache key without its Audiobookshelf protocol suffix; other keys are
/// already identities.
pub(crate) fn cache_identity(key: &str) -> &str {
    if key.starts_with(AUDIOBOOKSHELF_CACHE_KEY_PREFIX) {
        key.rsplit_once(':').map_or(key, |(base, _)| base)
    } else {
        key
    }
}

/// Cache key for an Audiobookshelf cover under `server`, keyed by the
/// library item's `id` and the active protocol `suffix`.
#[must_use]
pub fn audiobookshelf_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:cover:{id}:{suffix}")
}

/// Cache key for an Audiobookshelf book cover. Distinct from the podcast
/// cover key (`:book:`, not `:cover:`) so a book and a podcast sharing an
/// id never share artwork state (book-browsing spec).
#[must_use]
pub fn audiobookshelf_book_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:bookcover:{id}:{suffix}")
}

/// Cache key for an Audiobookshelf cover as the Library hero draws it:
/// `{server}:hero:cover:{id}:{suffix}`, the plain cover key under a hero
/// scope.
///
/// The hero re-encodes the entry's protocol from a cover-fit crop of its
/// artwork box (`ensure_hero_cover_protocol`), so one entry can carry either
/// the hero's crop or a plain consumer's `Resize::Scale` — never both. Sharing
/// one key (the queue card paints the same show cover for a playing episode)
/// made each consumer take the other's `ThreadProtocol` on every frame, so the
/// hero fell back to the placeholder block and flashed. Emby's hero and card
/// keys are already distinct for the same reason (`{id}:Backdrop,Primary` vs
/// `{id}:P`).
#[must_use]
pub fn audiobookshelf_hero_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:hero:cover:{id}:{suffix}")
}

/// Hero-scoped sibling of [`audiobookshelf_book_cover_cache_key`], with the
/// same crop-vs-plain isolation as [`audiobookshelf_hero_cover_cache_key`].
#[must_use]
pub fn audiobookshelf_hero_book_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:hero:bookcover:{id}:{suffix}")
}

/// Cache key for an Emby card's primary image: the album key when `album_id`
/// is non-empty (audio tracks on the same album share one cache entry keyed
/// by album id), else the item key. Shared by the queue-card projection
/// (`src/app/state/projection/card.rs`) and MPRIS (`mbv-desktop::mpris`) so their
/// `mpris:artUrl` lookup can never drift from what the card projection
/// actually wrote to disk (issue #833).
#[must_use]
pub fn emby_card_cache_key(item_id: &str, album_id: &str) -> String {
    if album_id.is_empty() {
        format!("{item_id}:P")
    } else {
        format!("{album_id}:P")
    }
}

/// Cache key for landscape Emby queue-card artwork, isolated from Primary art
/// shared by MPRIS and other consumers.
#[must_use]
pub fn emby_queue_landscape_cache_key(item_id: &str) -> String {
    format!("{item_id}:QB")
}

/// The infix opening a Series artwork key: `{id}{SERIES_IMAGE_CACHE_KEY_INFIX}{types}`.
/// No other cache-key namespace uses it, which is what lets the image-completion
/// gate recognise the whole Series family from the key alone.
pub const SERIES_IMAGE_CACHE_KEY_INFIX: &str = ":ser:";

/// Cache key for Series artwork under the `{id}:ser:{types}` scheme.
/// Shared by the panel's Emby artwork projection and shell-side
/// prefetch/loading lookups so they cannot format the key differently and
/// silently miss each other's cache entries. Formats only;
/// chain ownership stays with the callers.
#[must_use]
pub fn series_image_cache_key(item_id: &str, image_types: &[&str]) -> String {
    format!(
        "{item_id}{SERIES_IMAGE_CACHE_KEY_INFIX}{}",
        image_types.join(",")
    )
}
pub struct CachedImage {
    /// The decoded image. `None` for a fetch that resolved without artwork,
    /// and for every fetch a [`ImageDecode::SourceOnly`] Client keeps (design
    /// D14): such an entry holds only `source`. Read it only where pixels are
    /// needed; readiness and size go through [`CachedImage::has_art`] and
    /// [`CachedImage::dimensions`].
    pub img: Option<image::DynamicImage>,
    /// The bytes `img` decoded from, as the server sent them. `Some` for a
    /// fetched image, `None` for an entry the app derived itself (a composed
    /// title overlay, the bundled placeholder) or a fetch with no artwork.
    pub source: Option<SourceImage>,
    /// The hero artwork box (cells) the current protocols were built from
    /// (task 5.10, design D5's cover fit): the panel hero projection
    /// `resize_to_fill`s the source to the box's pixel size before encoding,
    /// keyed by the box, so a box change rebuilds the protocol at the new
    /// size. `None` for every non-hero cache entry (plain `Resize::Scale`).
    pub cover_box: Option<(u16, u16)>,
    /// Cache identity of the Logo applied to the current hero protocols.
    /// `None` means the base-only protocol is valid (including pending/failed
    /// Logo fetches).
    pub applied_logo_key: Option<String>,
}

impl CachedImage {
    /// An entry for a fetch that resolved with no image.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn empty() -> Self {
        Self {
            img: None,
            source: None,
            cover_box: None,
            applied_logo_key: None,
        }
    }

    /// Whether the entry holds art a surface can show: the decoded image in a
    /// pixel Client, the source bytes in a source-only Client (design D14). A
    /// fetch that resolved without art holds neither.
    #[must_use]
    pub fn has_art(&self) -> bool {
        self.img.is_some() || self.source.is_some()
    }

    /// The art's pixel size: the decoded image's, else the size the source
    /// header declares. `None` when the entry holds no art.
    #[must_use]
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.img
            .as_ref()
            .map(image::GenericImageView::dimensions)
            .or_else(|| self.source.as_ref().map(|source| source.dimensions))
    }
}

/// What a fetched image keeps (design D14). A Client chooses it once, when it
/// builds its `App`, and the image fetch path applies it to every fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageDecode {
    /// Decode the bytes to pixels. The TUI paints them through its terminal
    /// protocols, which need the pixels.
    Pixels,
    /// Keep the source bytes and read the size from the header; decode no
    /// pixels. The GUI paints the server's bytes, so it holds no decoded
    /// image.
    SourceOnly,
}

/// An image exactly as the server sent it: its bytes, the MIME type those
/// bytes declare, and the size the header declares. A Client that paints no
/// terminal protocol hands these to its own image pipeline unchanged, so the
/// pixels it shows are the pixels the terminal protocols decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    /// The pixel size the header of `bytes` declares, read without decoding
    /// the pixels.
    pub dimensions: (u32, u32),
}

impl SourceImage {
    /// The image `bytes` hold, or `None` when they are not a recognised image
    /// format or their header declares no size. Reads the header only.
    #[must_use]
    pub fn sniff(bytes: Vec<u8>) -> Option<Self> {
        let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .ok()?;
        let mime = reader.format()?.to_mime_type();
        let dimensions = reader.into_dimensions().ok()?;
        Some(Self {
            bytes,
            mime,
            dimensions,
        })
    }
}

/// What a card-image fetch concluded (invariant 27).
#[derive(Debug)]
pub enum ImageFetchOutcome {
    Image {
        /// The decoded image, `None` when the fetch kept the source only
        /// ([`ImageDecode::SourceOnly`]).
        img: Option<image::DynamicImage>,
        source: SourceImage,
    },
    /// The server answered and no usable image exists. Final.
    Empty,
    /// No answer: a timeout, a transport failure, or no client yet. This says
    /// nothing about whether art exists, so the key stays open to a retry.
    Interrupted,
}

/// Prefix of every row-rendition cache key (design D6): row images have their
/// own memory LRU and their own pending queue.
pub const ROW_IMAGE_CACHE_KEY_PREFIX: &str = "row:";

/// Whether `key` names a row-rendition image.
#[must_use]
pub fn is_row_cache_key(key: &str) -> bool {
    key.starts_with(ROW_IMAGE_CACHE_KEY_PREFIX)
}

/// Cache key for an Emby row image (design D6): `row:{item_id}:{type}:{tag}`,
/// or `row:{item_id}:untagged` when the item declares no tag in the row chain.
#[must_use]
pub fn emby_row_cache_key(item_id: &str, image: Option<&RowEmbyImage>) -> String {
    image.map_or_else(
        || format!("{ROW_IMAGE_CACHE_KEY_PREFIX}{item_id}:untagged"),
        |image| {
            format!(
                "{ROW_IMAGE_CACHE_KEY_PREFIX}{item_id}:{}:{}",
                image.kind, image.tag
            )
        },
    )
}

/// Prefix of every Audiobookshelf row cache key (design D6), so a Service
/// setup change can drop them with the other Audiobookshelf entries.
pub const ABS_ROW_CACHE_KEY_PREFIX: &str = "row:abs:";

/// Cache key for an Audiobookshelf row cover (design D6):
/// `row:abs:{server_hash}:{library_item_id}:{updated_at}`, where
/// `{updated_at}` is the library item's update time in milliseconds (`0` when
/// the catalog has none). A cover upload updates the item, so a new update
/// time makes a new key and changed art never reuses old bytes.
#[must_use]
pub fn abs_row_cache_key(server_url: &str, library_item_id: &str, updated_at_ms: u64) -> String {
    format!(
        "{ABS_ROW_CACHE_KEY_PREFIX}{:016x}:{library_item_id}:{updated_at_ms}",
        server_hash(server_url)
    )
}

/// A stable, compact server identity for a row cache key (design D6's
/// `server_hash`): FNV-1a over the server URL, so the key stays short and holds
/// none of the URL's own separators.
fn server_hash(server_url: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in server_url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A pending card-image fetch, queued when the in-flight limit is reached.
pub struct ImageFetchReq {
    pub cache_key: String,
    pub item_id: String,
    pub series_id: String,
    pub types: Vec<String>,
    pub source: ImageSource,
    /// The size and cache namespace the request asks for (design D6).
    pub rendition: ImageRendition,
}

/// The size and cache namespace one fetch asks for (design D6). A `Hero`
/// request fetches the large shared image under its source's own key; a `Row`
/// request fetches the small row image under a `row:` key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ImageRendition {
    #[default]
    Hero,
    Row(RowRendition),
}

/// The resolved data of one `Row` request (design D6): the declared artwork
/// arm that sizes the request, and the Emby image kind the item declares a tag
/// for, when it declares one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowRendition {
    /// Whether the item's declared artwork arm is Square. The request asks for
    /// a square image (the painted row box's 2x on both sides) instead of the
    /// landscape/portrait 2x width.
    pub square: bool,
    /// The one Emby image kind to request with its declared tag. `None`
    /// requests the whole `types` chain untagged.
    pub emby: Option<RowEmbyImage>,
}

/// One Emby image kind and the tag the item declares for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowEmbyImage {
    pub kind: String,
    pub tag: String,
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    Emby,
    Audiobookshelf { server_url: String, api_key: String },
}

/// Cover fit for a hero artwork box (design D5, task 5.4): the decoded
/// source image is scaled to cover the box's pixel size and centre-cropped,
/// so the box shows no margin; painting then uses the existing
/// `Resize::Scale`. The box's size comes from the panel's paint-free
/// `hero_artwork_box` (task 5.5); the shell projection that calls this is
/// keyed by the box (task 5.10, the hero protocol projection).
#[must_use]
pub fn cover_fill_hero_box(
    source: &image::DynamicImage,
    box_w: u32,
    box_h: u32,
) -> image::DynamicImage {
    let (w, h) = (box_w.max(1), box_h.max(1));
    source.resize_to_fill(w, h, image::imageops::FilterType::Lanczos3)
}

/// Logo decoration geometry, as percentages of the landscape hero bitmap
/// (design D3): contain-fit within 60% width / 20% height, then placed at a 5%
/// top-left inset clamped so the resized Logo stays inside the artwork.
const LOGO_MAX_WIDTH_PERCENT: u32 = 60;
const LOGO_MAX_HEIGHT_PERCENT: u32 = 20;
const LOGO_INSET_PERCENT: f32 = 5.0;

fn rounded_dimension(value: f32) -> u32 {
    value.to_string().parse().unwrap_or(u32::MAX)
}

/// Decorate an already cover-fitted landscape bitmap with a transparent Logo.
/// The Logo is contain-fitted into the prescribed bounds, then source-over
/// composited at the rounded, clamped inset.
#[must_use]
pub fn composite_landscape_logo(
    base: &image::DynamicImage,
    logo: &image::DynamicImage,
) -> image::DynamicImage {
    use image::GenericImageView;
    let mut base = base.to_rgba8();
    let (base_w, base_h) = base.dimensions();
    let max_w = ((base_w * LOGO_MAX_WIDTH_PERCENT) / 100).max(1);
    let max_h = ((base_h * LOGO_MAX_HEIGHT_PERCENT) / 100).max(1);
    let logo = logo.resize(max_w, max_h, image::imageops::FilterType::Lanczos3);
    let (logo_w, logo_h) = logo.dimensions();
    let inset_percent = LOGO_INSET_PERCENT / 100.0;
    let base_width_f32 = f32::from(u16::try_from(base_w).unwrap_or(u16::MAX));
    let base_height_f32 = f32::from(u16::try_from(base_h).unwrap_or(u16::MAX));
    let inset_x = rounded_dimension((base_width_f32 * inset_percent).round())
        .min(base_w.saturating_sub(logo_w));
    let inset_y = rounded_dimension((base_height_f32 * inset_percent).round())
        .min(base_h.saturating_sub(logo_h));
    image::imageops::overlay(
        &mut base,
        &logo.to_rgba8(),
        i64::from(inset_x),
        i64::from(inset_y),
    );
    image::DynamicImage::ImageRgba8(base)
}

impl std::fmt::Debug for CachedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedImage").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ImageFetchReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageFetchReq").finish_non_exhaustive()
    }
}
