//! The image fetch's transport and decode (`extract-client-core` design D3):
//! the per-Service byte reads and the shared pixel-or-source decode. The Emby
//! URL shapes live in `emby`.

use super::emby::fetch_emby_image;
use mbv_images::{
    ImageDecode, ImageFetchOutcome, ImageFetchReq, ImageRendition, ImageSource, SourceImage,
};
use std::io::Read as IoRead;

/// The server bytes a fetch kept, with the decoded image when `decode` asks
/// for pixels (design D14), or `Empty` when the bytes are no image. A
/// source-only fetch reads the header alone, so its size and MIME type come
/// from the bytes without a pixel decode.
pub(in crate::app) fn decode_fetched(bytes: Vec<u8>, decode: ImageDecode) -> ImageFetchOutcome {
    match decode {
        ImageDecode::Pixels => {
            let Ok(img) = image::load_from_memory(&bytes) else {
                return ImageFetchOutcome::Empty;
            };
            SourceImage::sniff(bytes).map_or(ImageFetchOutcome::Empty, |source| {
                ImageFetchOutcome::Image {
                    img: Some(img),
                    source,
                }
            })
        }
        ImageDecode::SourceOnly => SourceImage::sniff(bytes)
            .map_or(ImageFetchOutcome::Empty, |source| {
                ImageFetchOutcome::Image { img: None, source }
            }),
    }
}

/// One image request's result (invariant 27).
#[derive(Debug, PartialEq)]
pub(in crate::app) enum FetchedBytes {
    Bytes(Vec<u8>),
    /// The server answered without an image: the next kind may have one.
    Missing,
    /// No answer. A later kind could be the series' art, so the chain stops.
    Interrupted,
}

pub(in crate::app) fn fetch_image_bytes(
    req: ImageFetchReq,
    emby_url: &str,
    token: &str,
) -> FetchedBytes {
    let ImageFetchReq {
        cache_key,
        item_id,
        series_id,
        types,
        source,
        rendition,
    } = req;
    match source {
        ImageSource::Audiobookshelf {
            server_url,
            api_key,
        } => fetch_audiobookshelf_image(&cache_key, &server_url, &api_key, &item_id, &rendition),
        ImageSource::Emby => fetch_emby_image(
            &cache_key, &item_id, &series_id, &types, &rendition, emby_url, token,
        ),
    }
}

fn fetch_audiobookshelf_image(
    cache_key: &str,
    server_url: &str,
    api_key: &str,
    item_id: &str,
    rendition: &ImageRendition,
) -> FetchedBytes {
    if let Some(cached) = crate::config::read_image_disk_cache(cache_key) {
        return FetchedBytes::Bytes(cached);
    }
    let Ok(client) = mbv_audiobookshelf::AudiobookshelfClient::new(server_url) else {
        return FetchedBytes::Missing;
    };
    let bound = mbv_audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND;
    let result = match rendition {
        // The Square arm is a podcast cover, and the landscape/portrait arm a
        // book cover (design D6): each asks for its row width.
        ImageRendition::Row(row) if row.square => client.row_cover_bounded(
            api_key,
            item_id,
            mbv_audiobookshelf::ROW_PODCAST_COVER_WIDTH,
            bound,
        ),
        ImageRendition::Row(_) => client.row_cover_bounded(
            api_key,
            item_id,
            mbv_audiobookshelf::ROW_BOOK_COVER_WIDTH,
            bound,
        ),
        ImageRendition::Hero => client.cover_bounded(api_key, item_id, bound),
    };
    match result {
        Ok(bytes) => {
            crate::config::write_image_disk_cache(cache_key, &bytes);
            FetchedBytes::Bytes(bytes)
        }
        Err(error)
            if matches!(
                error.class,
                mbv_audiobookshelf::AudiobookshelfFailureClass::Connectivity
                    | mbv_audiobookshelf::AudiobookshelfFailureClass::Unavailable
            ) =>
        {
            FetchedBytes::Interrupted
        }
        Err(_) => FetchedBytes::Missing,
    }
}

pub(in crate::app) fn fetch_url(url: &str) -> FetchedBytes {
    let agent = mbv_net::native_tls_agent(
        mbv_net::HttpService::Emby,
        None,
        Some(std::time::Duration::from_secs(10)),
    );
    let response = match agent.get(url).call() {
        Ok(response) => response,
        Err(error) => return request_failure(&error),
    };
    let mut bytes = Vec::new();
    match response
        .into_body()
        .into_reader()
        .take(10 * 1024 * 1024)
        .read_to_end(&mut bytes)
    {
        Ok(_) => FetchedBytes::Bytes(bytes),
        Err(_) => FetchedBytes::Interrupted,
    }
}

/// A status code is the server's answer; every other failure is no answer.
pub(in crate::app) fn request_failure(error: &ureq::Error) -> FetchedBytes {
    if matches!(error, ureq::Error::StatusCode(_)) {
        FetchedBytes::Missing
    } else {
        FetchedBytes::Interrupted
    }
}
