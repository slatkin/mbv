use ratatui::layout::Rect;

/// The image an in-progress hero render needs painted, computed without
/// `App` (design D2): the shell fetches/looks up the cached protocol and
/// paints it into `area` using App's image-cache authority right after
/// `view()` returns (task 3.4's confirmed extraction: share orchestration,
/// defer only the pixel paint).
///
/// `Emby` (the Home/Movies Keep-Watching-style hero card) and `CompactBanner`
/// (the Movies/TV compact detail banner poster) were deleted with their last
/// producers (task 6.1, design D7): every migrated destination's Narrow
/// inline hero and Wide header now derive from the panel's generic
/// `HeroContent`/artwork-policy image projection instead.
pub(in crate::app) enum HomeImagePaint {
    Series {
        area: Rect,
        item: Box<mbv_core::api::EmbyItem>,
        show_placeholder: bool,
        /// Ordered Emby image-type candidate chain to fetch, so wide TV's
        /// landscape hero can request the `Thumb`-first chain while other
        /// callers keep the narrow inline detail's `&["Primary"]`.
        image_types: &'static [&'static str],
    },
    AudiobookshelfCover {
        area: Rect,
        library_item_id: String,
        /// `true` for the narrow beside-image hero (`GenericBeside`), which
        /// always shows the dim placeholder while uncached, matching every
        /// other beside-image hero; `false` for the two-column/text `Generic`
        /// detail block, which renders nothing until the cover is cached (an
        /// existing, preserved difference between the two call sites).
        show_placeholder: bool,
    },
    /// Audiobookshelf book artwork must stay isolated from podcast artwork,
    /// including when both use the same library item ID (book-browsing spec
    /// line 124).
    AudiobookshelfBookCover {
        area: Rect,
        library_item_id: String,
        show_placeholder: bool,
    },
}
