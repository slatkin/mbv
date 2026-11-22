//! The Emby image read's own contracts (design D6, D14, invariant 27): which
//! URL each rendition asks for, that a status code separates the server's
//! answer from no answer, that an interrupted kind stops the chain, and how a
//! fetched image decodes. None of it builds a protocol value, so it stays
//! beside the transport rather than in the TUI shell.

use super::emby::{emby_type_url, first_image};
use super::transport::{FetchedBytes, decode_fetched, request_failure};
use crate::app::tests::make_app_stub;
use mbv_images::{ImageDecode, ImageFetchOutcome, ImageRendition, RowEmbyImage, RowRendition};

fn image(width: u32, height: u32) -> image::DynamicImage {
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([20, 40, 60, 255]),
    ))
}

/// User report 2026-10-30: only a status code is an answer. A timeout says
/// nothing about the art, and must not become a final "no image".
#[rstest::rstest]
#[case::status_code(ureq::Error::StatusCode(404), FetchedBytes::Missing)]
#[case::timeout(ureq::Error::Timeout(ureq::Timeout::Global), FetchedBytes::Interrupted)]
fn request_failure_separates_an_answer_from_no_answer(
    #[case] error: ureq::Error,
    #[case] expected: FetchedBytes,
) {
    assert_eq!(request_failure(&error), expected);
}

/// Same report: a timed-out `Primary` must not fall through to the series'
/// `Thumb`, which would cache series art under the episode's own key.
#[test]
fn an_interrupted_kind_stops_the_image_chain() {
    let kinds = ["Primary".to_owned(), "Thumb".to_owned()];

    let result = first_image(&kinds, |kind| match kind {
        "Primary" => FetchedBytes::Interrupted,
        _ => FetchedBytes::Bytes(vec![1]),
    });

    assert_eq!(result, FetchedBytes::Interrupted);
}

/// The GUI paints the server's bytes, not a re-encode of the decoded pixels
/// (commit 8957f2ccf's lossy-on-lossy regression): the fetch outcome carries
/// the bytes it decoded, unchanged, with the MIME type they declare.
#[test]
fn a_fetched_image_carries_the_servers_bytes_unchanged() {
    let bytes = crate::app::queue_card_artwork::test_source(&image(4, 4)).bytes;

    let outcome = decode_fetched(bytes.clone(), ImageDecode::Pixels);

    let ImageFetchOutcome::Image { source, .. } = outcome else {
        panic!("a decodable image yields an image outcome: {outcome:?}");
    };
    assert_eq!(source.bytes, bytes);
    assert_eq!(source.mime, "image/png");
}

/// Design D14: only a pixel Client decodes the fetched image. Both modes keep
/// the server's bytes and the size the header declares.
#[rstest::rstest]
#[case::pixel_client_decodes(ImageDecode::Pixels, true)]
#[case::source_only_client_keeps_no_pixels(ImageDecode::SourceOnly, false)]
fn a_fetch_decodes_pixels_only_in_a_pixel_client(
    #[case] decode: ImageDecode,
    #[case] decoded: bool,
) {
    let bytes = crate::app::queue_card_artwork::test_source(&image(4, 3)).bytes;

    let outcome = decode_fetched(bytes.clone(), decode);

    let ImageFetchOutcome::Image { img, source } = outcome else {
        panic!("a decodable image yields an image outcome: {outcome:?}");
    };
    assert_eq!(img.is_some(), decoded);
    assert_eq!(source.bytes, bytes);
    assert_eq!(source.dimensions, (4, 3));
}

/// Design D14: a source-only Client's completion lands in the cache as the
/// source alone, and the entry still reads as art at the header's size.
#[test]
fn a_source_only_completion_caches_the_source_and_no_pixels() {
    const KEY: &str = "gui-card:P";
    let mut app = make_app_stub();
    let bytes = crate::app::queue_card_artwork::test_source(&image(4, 3)).bytes;
    let outcome = decode_fetched(bytes, ImageDecode::SourceOnly);
    app.images
        .card_image_tx()
        .send((KEY.to_owned(), outcome))
        .expect("the completion channel is open");

    app.drain_card_image_completions();

    let entry = app
        .images
        .image(KEY)
        .expect("the completion lands in the cache");
    assert!(entry.img.is_none());
    assert!(entry.source.is_some());
    assert!(entry.has_art());
    assert_eq!(entry.dimensions(), Some((4, 3)));
}

fn row_rendition(square: bool, image: Option<RowEmbyImage>) -> ImageRendition {
    ImageRendition::Row(RowRendition {
        square,
        emby: image,
    })
}

/// Design D6: a row's declared kind is requested with its tag and the 2x row
/// width, at quality 80, with no height and no format parameter.
#[test]
fn a_tagged_row_request_url_carries_the_tag_and_row_width() {
    let rendition = row_rendition(
        false,
        Some(RowEmbyImage {
            kind: "Thumb".into(),
            tag: "thumb-1".into(),
        }),
    );

    let url = emby_type_url(
        "Thumb",
        "item-1",
        "",
        &rendition,
        Some("thumb-1"),
        "https://emby.example",
        "secret",
    );

    assert_eq!(
        url,
        "https://emby.example/Items/item-1/Images/Thumb?maxWidth=128&quality=80&tag=thumb-1&api_key=secret"
    );
}

/// Design D6: a chain walk over an item that declares no tag sends no tag.
#[test]
fn an_untagged_row_request_url_omits_the_tag_parameter() {
    let url = emby_type_url(
        "Backdrop",
        "item-1",
        "",
        &row_rendition(false, None),
        None,
        "https://emby.example",
        "secret",
    );

    assert_eq!(
        url,
        "https://emby.example/Items/item-1/Images/Backdrop/0?maxWidth=128&quality=80&api_key=secret"
    );
}

/// Design D6: a Square arm asks for both sides (Emby forces an exact width and
/// height), while the landscape arm asks for the width alone.
#[test]
fn a_square_row_request_url_asks_for_both_sides() {
    let url = emby_type_url(
        "Primary",
        "item-1",
        "",
        &row_rendition(true, None),
        None,
        "https://emby.example",
        "secret",
    );

    assert_eq!(
        url,
        "https://emby.example/Items/item-1/Images/Primary?maxWidth=72&maxHeight=72&quality=80&api_key=secret"
    );
}
