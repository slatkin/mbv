//! Tests of the row art resolver (gui-delineated-rows design D4, D5, D6): the
//! Hero artwork policy decides whether a row has a source at all, the cache
//! read answers the three sourced states, and the fit comes from the cached
//! image's own pixels. Every boundary is the in-memory cache, so no request
//! leaves the process.

use mbv_emby_model::test_support::make_item;
use mbv_images::CachedImage;
use mbv_queue::{AudiobookshelfEpisodeCatalog, EpisodeResume};

use super::*;
use crate::app::make_app_stub;

/// A feed entry of `kind`: the Feeds destination's row input.
fn feed_entry(kind: FeedKind) -> FeedEntry {
    FeedEntry {
        guid: "f1".to_owned(),
        title: "A Feed Entry".to_owned(),
        enclosure_url: Some("https://example.invalid/e.mp3".to_owned()),
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(kind),
        feed_id: Some("https://example.invalid/feed.xml".to_owned()),
        position_ticks: 0,
        played: false,
    }
}

/// One podcast episode over the show `show_id`, with a cover declared.
fn episode(show_id: &str) -> AudiobookshelfQueueItem {
    AudiobookshelfQueueItem::from_catalog(
        AudiobookshelfEpisodeCatalog {
            library_item_id: show_id.to_owned(),
            episode_id: "ep-1".to_owned(),
            title: "An Episode".to_owned(),
            show_title: Some("A Show".to_owned()),
            author: None,
            description: None,
            duration_ticks: None,
            pub_date_secs: None,
            cover_path: Some("cover.jpg".to_owned()),
        },
        EpisodeResume::NOT_STARTED,
    )
}

/// An `App` whose Audiobookshelf setup names `server`, with the image cache
/// holding no entry for the row keys the tests read.
fn app_with_audiobookshelf(server: &str) -> App {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.seed_audiobookshelf_server_url_for_test(server);
    app
}

/// A feed row declares no art source (design D5), so the cache read answers the
/// no-source state and the core queues no request for it.
#[test]
fn a_feed_row_has_no_source_and_makes_no_request() {
    let app = make_app_stub();

    let source = feed_row_source(&feed_entry(FeedKind::Audio));

    assert_eq!(source, None, "the feed artwork policy declares no source");
    assert_eq!(
        row_art(&app, source.as_ref()),
        GuiRowArt::NoSource,
        "a row with no source paints the no-source state"
    );
    assert_eq!(
        app.card_image_fetch_count_for_test(),
        0,
        "no row fetch is reserved for a row with no source"
    );
}

/// A cached square cover paints whole (design D2, D4): the fit comes from the
/// cached image's own pixels, not from the arm the policy declared.
#[test]
fn a_cached_square_cover_paints_whole() {
    let server = "http://127.0.0.1:1";
    let mut app = app_with_audiobookshelf(server);
    let source = episode_row_source(&episode("show-1"), 7).expect("a podcast cover source");
    let key = mbv_images::abs_row_cache_key(server, "show-1", 7);
    app.seed_card_image_for_test(
        key.clone(),
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            72,
            72,
            image::Rgba([20, 40, 60, 255]),
        )),
    );

    assert_eq!(
        row_art(&app, Some(&source)),
        GuiRowArt::Image { key, square: true },
        "a square cover's pixels show the whole image"
    );
}

/// A fetch that answered with no image is final (design D4, invariant 27): the
/// row paints the missing state, not a loading state.
#[test]
fn an_empty_fetch_paints_missing() {
    let server = "http://127.0.0.1:1";
    let mut app = app_with_audiobookshelf(server);
    let source = episode_row_source(&episode("show-1"), 7).expect("a podcast cover source");
    let key = mbv_images::abs_row_cache_key(server, "show-1", 7);
    app.insert_cached_image_for_test(&key, CachedImage::empty());

    assert_eq!(
        row_art(&app, Some(&source)),
        GuiRowArt::Missing,
        "an entry with no art is the missing state"
    );
}

/// An Emby row with no declared image and no id has no source: the policy's
/// empty item answers the no-source state.
#[test]
fn an_emby_row_without_declared_images_has_no_source() {
    let app = make_app_stub();
    let mut item = make_item("A Movie", "Movie");
    item.id = "m-1".to_owned();

    assert_eq!(emby_row_source(&item), None);
    assert_eq!(row_art(&app, None), GuiRowArt::NoSource);
}

/// A row that is not cached yet is loading, whatever the fetch's state: the
/// projection reads the cache alone and never waits on a fetch.
#[test]
fn an_uncached_row_paints_loading() {
    let server = "http://127.0.0.1:1";
    let app = app_with_audiobookshelf(server);
    let source = episode_row_source(&episode("show-1"), 7).expect("a podcast cover source");

    assert_eq!(row_art(&app, Some(&source)), GuiRowArt::Loading);
}
