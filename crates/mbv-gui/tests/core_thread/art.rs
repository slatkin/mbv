//! The art-request contract (design D7 of `gui-change-driven-sections`): the
//! tick host answers a `CoreInput::Art` through the `App`'s read-only cache
//! queries — the cached source's bytes and MIME, the bundled placeholder, and
//! `Missing` for everything the cache does not hold. An art request is not an
//! input the window paints: it sets no mark and no `input_received`, so a
//! settled core sends no batch for one.

use std::sync::mpsc;

use mbv::stub_app_with_playing_queue;
use mbv_gui::art::{ArtRequest, ArtResponse, PLACEHOLDER_PATH, card_path};
use mbv_gui::core_thread::CoreInput;
use mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES;
use rstest::rstest;

use crate::{TestCore, test_core, test_core_with};

/// The cached card image the seeded queue carries: the queue card's landscape
/// cache key over the stub's item id, as `queue_card_artwork` derives it.
const CACHED_CARD_KEY: &str = "id:QB";

/// The case rows the table runs (design D7's art row).
#[derive(Debug, Clone, Copy)]
enum ArtCase {
    /// The cache holds the queue card's landscape entry under the key.
    CachedCard,
    /// The bundled placeholder path.
    Placeholder,
    /// A well-formed card path naming no cached entry.
    UnknownCard,
    /// A path in no accepted form.
    MalformedPath,
}

/// A settled core over the case's App: the first construction batch is
/// consumed, so what the art step sends names a change the art request alone
/// caused (it changes none).
fn settled_core(art_case: ArtCase) -> TestCore {
    let mut test = match art_case {
        ArtCase::CachedCard => {
            let mut app = stub_app_with_playing_queue(1);
            app.enable_card_images_for_test();
            app.seed_card_image_for_test(
                CACHED_CARD_KEY.to_owned(),
                image::DynamicImage::new(4, 4, image::ColorType::Rgb8),
            );
            test_core_with(app)
        }
        ArtCase::Placeholder | ArtCase::UnknownCard | ArtCase::MalformedPath => test_core(2),
    };
    assert!(test.core.step());
    test.next_batch();
    test
}

/// The request path the case sends, as the window's URL carries it (design
/// D7): the card key percent-encoded by the one builder that owns the
/// encoding.
fn request_path(art_case: ArtCase) -> String {
    match art_case {
        ArtCase::CachedCard => card_path(CACHED_CARD_KEY),
        ArtCase::Placeholder => PLACEHOLDER_PATH.to_owned(),
        ArtCase::UnknownCard => card_path("no-such-card:ser:Primary"),
        ArtCase::MalformedPath => "bogus/answer".to_owned(),
    }
}

/// The answer the case expects from the core's cache (design D7).
fn expected_answer(art_case: ArtCase) -> ArtResponse {
    match art_case {
        ArtCase::CachedCard => {
            // The cached source's own bytes and the MIME those bytes declare:
            // the PNG encode the seed wrote, sniffed back to `image/png`.
            let source = image::DynamicImage::new(4, 4, image::ColorType::Rgb8);
            let mut png = std::io::Cursor::new(Vec::new());
            source.write_to(&mut png, image::ImageFormat::Png).unwrap();
            ArtResponse::Found {
                mime: "image/png",
                bytes: png.into_inner(),
            }
        }
        ArtCase::Placeholder => ArtResponse::Found {
            mime: "image/webp",
            bytes: QUEUE_CARD_PLACEHOLDER_BYTES.to_vec(),
        },
        ArtCase::UnknownCard | ArtCase::MalformedPath => ArtResponse::Missing,
    }
}

/// The contract's one case table: a cached card key answers with its bytes and
/// MIME type, the placeholder path with the placeholder, and an unknown key or
/// a malformed path with `Missing`.
#[rstest]
#[case::cached_card_gives_its_bytes_and_mime(ArtCase::CachedCard)]
#[case::placeholder_gives_the_placeholder(ArtCase::Placeholder)]
#[case::unknown_card_key_gives_missing(ArtCase::UnknownCard)]
#[case::malformed_path_gives_missing(ArtCase::MalformedPath)]
fn an_art_request_answers_through_the_read_only_cache(#[case] art_case: ArtCase) {
    let mut test = settled_core(art_case);
    let (answer_tx, answer_rx) = mpsc::channel();
    test.tx
        .send(CoreInput::Art(ArtRequest {
            path: request_path(art_case),
            reply: Box::new(move |response| {
                let _ = answer_tx.send(response);
            }),
        }))
        .unwrap();
    assert!(test.core.step());

    assert_eq!(
        answer_rx.try_recv().expect("the core answered the request"),
        expected_answer(art_case)
    );
    // The request marks no section and sets no `input_received` (design D7):
    // the art step sent no batch, on top of the settled first one.
    assert_eq!(test.batches.try_recv(), Err(mpsc::TryRecvError::Empty));
}
