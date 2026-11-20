//! Row-sized image requests (design D6): one image kind, the tag the item
//! declares for it, and the size its declared artwork arm asks for.
//!
//! A row shows the same image as its item's Hero, so the request addresses the
//! same source the Hero policy would (an episode's series image, for example).
//! Unlike the Hero's chain walk, the row request asks for exactly one kind: the
//! first in the row chain whose tag the addressed source declares. An item that
//! declares no tag in the chain asks for the whole chain, untagged.

use crate::{
    ImageFetchReq, ImageRendition, ImageSource, RowEmbyImage, RowRendition, emby_row_cache_key,
};
use mbv_emby_model::{EmbyImageTags, EmbyItem};

/// The fixed row image chain, in preference order (design D6).
const ROW_IMAGE_KINDS: [&str; 4] = ["Thumb", "Backdrop", "Logo", "Primary"];

/// The `Row` request for one Emby item: its id, its series addressing, and the
/// image tags it declares. `square` is the item's declared artwork arm.
#[must_use]
pub fn emby_row_request(item: &EmbyItem, square: bool) -> ImageFetchReq {
    emby_row_request_for(&item.id, &item.series_id, &item.image_tags, square)
}

/// The `Row` request from a raw address and declared tags. A caller whose row
/// is not an Emby item frame (a Music artist named only by its Service id)
/// passes an empty `series_id` and the default tags, which requests the chain
/// untagged.
#[must_use]
pub fn emby_row_request_for(
    item_id: &str,
    series_id: &str,
    tags: &EmbyImageTags,
    square: bool,
) -> ImageFetchReq {
    let emby = row_image(tags, series_id);
    let types = emby.as_ref().map_or_else(
        || ROW_IMAGE_KINDS.iter().map(ToString::to_string).collect(),
        |image| vec![image.kind.clone()],
    );
    ImageFetchReq {
        cache_key: emby_row_cache_key(item_id, emby.as_ref()),
        item_id: item_id.to_owned(),
        series_id: series_id.to_owned(),
        types,
        source: ImageSource::Emby,
        rendition: ImageRendition::Row(RowRendition { square, emby }),
    }
}

/// The first kind in the row chain whose tag the addressed source declares.
fn row_image(tags: &EmbyImageTags, series_id: &str) -> Option<RowEmbyImage> {
    ROW_IMAGE_KINDS.iter().find_map(|kind| {
        let tag = declared_tag(tags, series_id, kind);
        (!tag.is_empty()).then(|| RowEmbyImage {
            kind: (*kind).to_owned(),
            tag: tag.to_owned(),
        })
    })
}

/// The tag the row chain's `kind` declares, read from the source the request
/// addresses: with a `series_id`, `Thumb` and `Backdrop` address the series
/// (`series_thumb`, `series_backdrops[0]`); without one they address the item
/// (`thumb`, `backdrops[0]`). `Primary` always addresses the item. The item DTO
/// declares no series Logo tag, so a series-addressed row never selects `Logo`.
fn declared_tag<'a>(tags: &'a EmbyImageTags, series_id: &str, kind: &str) -> &'a str {
    let series = !series_id.is_empty();
    match kind {
        "Thumb" if series => &tags.series_thumb,
        "Thumb" => &tags.thumb,
        "Backdrop" if series => tags.series_backdrops.first().map_or("", String::as_str),
        "Backdrop" => tags.backdrops.first().map_or("", String::as_str),
        "Logo" if series => "",
        "Logo" => &tags.logo,
        _ => &tags.primary,
    }
}
