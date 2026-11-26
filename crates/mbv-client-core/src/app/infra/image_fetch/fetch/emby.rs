//! Emby image reads (design D6): one URL per image kind. A Hero request keeps
//! the shared 1280-pixel quality-90 cap; a Row request asks for the declared
//! kind and tag at the row box's 2x size.

use super::transport::{FetchedBytes, fetch_url};
use mbv_images::ImageRendition;

/// The Emby image fetch's long-side cap. Every Client shows the cached bytes
/// as they are — the TUI decodes them for its terminal protocol, and the GUI
/// hands them to the window unchanged — so this one cap is the pixel budget
/// of every surface: the GUI Hero at a 2x scale factor, and the card and row
/// art that share its cache entry. A smaller cap leaves the Hero's browser
/// upscaling the server's pixels, and a larger one costs fetch time and frame
/// payload for pixels no surface paints.
const IMAGE_FETCH_MAX_SIDE: u32 = 1280;

/// The Emby image fetch's JPEG quality. The server's encode is the only lossy
/// generation any Client paints, so it stays high.
const IMAGE_FETCH_QUALITY: u8 = 90;

/// A landscape or portrait row image's requested width: 2x the 72 x 40 row
/// box. No height joins it, because Emby then forces that exact size.
const ROW_LANDSCAPE_MAX_WIDTH: u32 = 128;

/// A Square row image's requested sides: about 2x the 40 x 40 painted box.
const ROW_SQUARE_MAX_SIDE: u32 = 72;

/// The Hero backdrop source's requested width: tiny, because the backdrop
/// blurs it into soft colour fields.
const BACKDROP_MAX_WIDTH: u32 = 32;

/// A row image's JPEG quality (design D6).
const ROW_IMAGE_QUALITY: u8 = 60;

pub(in crate::app) fn fetch_emby_image(
    cache_key: &str,
    item_id: &str,
    series_id: &str,
    types: &[String],
    rendition: &ImageRendition,
    server_url: &str,
    token: &str,
) -> FetchedBytes {
    if let Some(cached) = mbv_config::read_image_disk_cache(cache_key) {
        // Local-only cache hits power dim-then-undim cycles for warm-cache modals.
        tracing::debug!(name: "images.disk_cache.hit", target: "images", item = %item_id, "image disk cache hit");
        return FetchedBytes::Bytes(cached);
    }
    // A row that resolved a declared tag asks for that one kind and tag
    // (design D6); every other request walks `types` untagged.
    let declared = match rendition {
        ImageRendition::Row(row) => row.emby.as_ref(),
        ImageRendition::Hero | ImageRendition::Backdrop => None,
    };
    let fetched = declared.map_or_else(
        || {
            first_image(types, |kind| {
                fetch_emby_image_type(kind, item_id, series_id, rendition, None, server_url, token)
            })
        },
        |image| {
            fetch_emby_image_type(
                &image.kind,
                item_id,
                series_id,
                rendition,
                Some(&image.tag),
                server_url,
                token,
            )
        },
    );
    // Cache original server bytes as-is; Emby already resized these to
    // `IMAGE_FETCH_MAX_SIDE`.
    if let FetchedBytes::Bytes(ref bytes) = fetched {
        mbv_config::write_image_disk_cache(cache_key, bytes);
    }
    fetched
}

/// Walks the image kinds in order: only `Missing` moves on to the next kind.
pub(in crate::app) fn first_image(
    kinds: &[String],
    mut fetch: impl FnMut(&str) -> FetchedBytes,
) -> FetchedBytes {
    for kind in kinds {
        match fetch(kind) {
            FetchedBytes::Missing => {}
            result => return result,
        }
    }
    FetchedBytes::Missing
}

fn fetch_emby_image_type(
    kind: &str,
    item_id: &str,
    series_id: &str,
    rendition: &ImageRendition,
    tag: Option<&str>,
    server_url: &str,
    token: &str,
) -> FetchedBytes {
    if kind == "AudioChild" {
        let child_url = format!(
            "{server_url}/Items?ParentId={item_id}&IncludeItemTypes=Audio&Limit=1&api_key={token}"
        );
        let bytes = match fetch_url(&child_url) {
            FetchedBytes::Bytes(bytes) => bytes,
            result => return result,
        };
        let child_id = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| {
                value["Items"]
                    .get(0)
                    .and_then(|item| item["Id"].as_str().map(str::to_owned))
            });
        let Some(child_id) = child_id else {
            return FetchedBytes::Missing;
        };
        let url = emby_image_url(server_url, token, &child_id, "Primary", rendition, tag);
        return fetch_url(&url);
    }

    let url = emby_type_url(kind, item_id, series_id, rendition, tag, server_url, token);
    fetch_url(&url)
}

/// The URL of one Emby image kind, addressed the way the kind addresses its
/// source: `Logo`, `Backdrop`, and `Thumb` with a `series_id` address the
/// series, every other combination addresses the item.
pub(in crate::app) fn emby_type_url(
    kind: &str,
    item_id: &str,
    series_id: &str,
    rendition: &ImageRendition,
    tag: Option<&str>,
    server_url: &str,
    token: &str,
) -> String {
    let src = emby_image_source(kind, item_id, series_id);
    emby_image_url(
        server_url,
        token,
        src,
        emby_image_kind(kind),
        rendition,
        tag,
    )
}

fn emby_image_source<'a>(kind: &str, item_id: &'a str, series_id: &'a str) -> &'a str {
    match kind {
        "Logo" | "Backdrop" | "Thumb" if !series_id.is_empty() => series_id,
        _ => item_id,
    }
}

fn emby_image_kind(kind: &str) -> &str {
    match kind {
        "Backdrop" => "Backdrop/0",
        "Logo" => "Logo",
        "Thumb" => "Thumb",
        _ => "Primary",
    }
}

/// The request URL for one Emby image (design D6): a Hero request keeps the
/// shared 1280-pixel, quality-90 cap; a Row request asks for 2x the painted row
/// box — one width for a landscape or portrait arm, both sides for a Square
/// arm (Emby distorts an image given an exact width and height with a
/// different aspect). Only a declared row tag rides the URL.
fn emby_image_url(
    server_url: &str,
    token: &str,
    src: &str,
    image_kind: &str,
    rendition: &ImageRendition,
    tag: Option<&str>,
) -> String {
    let (size, quality) = match rendition {
        ImageRendition::Hero => (
            format!("maxWidth={IMAGE_FETCH_MAX_SIDE}&maxHeight={IMAGE_FETCH_MAX_SIDE}"),
            IMAGE_FETCH_QUALITY,
        ),
        ImageRendition::Row(row) if row.square => (
            format!("maxWidth={ROW_SQUARE_MAX_SIDE}&maxHeight={ROW_SQUARE_MAX_SIDE}"),
            ROW_IMAGE_QUALITY,
        ),
        ImageRendition::Row(_) => (
            format!("maxWidth={ROW_LANDSCAPE_MAX_WIDTH}"),
            ROW_IMAGE_QUALITY,
        ),
        ImageRendition::Backdrop => (
            format!("maxWidth={BACKDROP_MAX_WIDTH}"),
            IMAGE_FETCH_QUALITY,
        ),
    };
    let tag = tag.map_or_else(String::new, |tag| format!("&tag={tag}"));
    format!(
        "{server_url}/Items/{src}/Images/{image_kind}?{size}&quality={quality}{tag}&api_key={token}"
    )
}
