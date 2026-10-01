use super::{
    NowPlayingTitleSite, card_cache_key, card_cache_key_for_id, card_image_types,
    overlay_logo_source, painted_overlay_key, resolve_title_site, slotless_card_image_types,
};
use crate::app::App;
use crate::app::tests::render_fixtures::make_queue_app;
use mbv_images::CachedImage;
use ratatui_image::picker::{Picker, ProtocolType};

const KEY: &str = "art:t:8x4:1";

#[rstest::rstest]
#[case::movie("Movie", "item:QB", &["Backdrop", "Primary"])]
#[case::episode("Episode", "item:P", &["Primary", "Thumb", "Backdrop", "Logo"])]
#[case::audio("Audio", "item:P", &["Primary"])]
#[case::music_album(
    "MusicAlbum",
    "item:P",
    mbv_render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES
)]
fn queue_card_key_uses_landscape_for_movies_and_preserves_music_keys(
    #[case] item_type: &str,
    #[case] expected: &str,
    #[case] expected_image_types: &[&str],
) {
    let mut item = mbv_emby_model::test_support::make_item("Item", item_type);
    item.id = "item".into();
    assert_eq!(card_cache_key(&item), expected);
    assert_eq!(card_image_types(&item), expected_image_types);
}

#[rstest::rstest]
#[case::movie(Some("Movie"), "item:QB")]
#[case::episode(Some("Episode"), "item:P")]
#[case::audio(Some("Audio"), "item:P")]
#[case::unknown(None, "item:P")]
fn slotless_card_key_is_landscape_for_movies_only(
    #[case] item_type: Option<&str>,
    #[case] expected: &str,
) {
    assert_eq!(card_cache_key_for_id("item", item_type), expected);
}

// Guards 55c37ea71 (landscape key for movies and episodes) and its fix
// e39b5a8f4 (episode keeps its own still, Movie-only key): the chain must still reach a declared
// `Thumb` (a Thumb-only home video resolves, since `fetch_emby_image` is a
// first-success fallthrough), but only a Movie takes the landscape chain — an
// episode keeps its own poster-first chain so its card never shows series art.
#[test]
fn thumb_only_video_card_chain_reaches_thumb() {
    let mut item = mbv_emby_model::test_support::make_item("Home Video", "Video");
    item.id = "item".into();
    item.image_tags.thumb = "thumb-tag".into();
    assert_eq!(
        card_image_types(&item),
        &["Primary", "Thumb", "Backdrop", "Logo"]
    );
}

#[test]
fn episode_card_stays_episode_owned_despite_series_tags() {
    let mut item = mbv_emby_model::test_support::make_item("Pilot", "Episode");
    item.id = "item".into();
    item.image_tags.series_thumb = "series-thumb-tag".into();
    assert_eq!(card_cache_key(&item), "item:P");
    assert_eq!(
        card_image_types(&item),
        &["Primary", "Thumb", "Backdrop", "Logo"]
    );
}

#[test]
fn landscape_movie_card_chain_stays_backdrop_first() {
    let mut item = mbv_emby_model::test_support::make_item("Feature", "Movie");
    item.id = "item".into();
    item.image_tags.backdrops = vec!["backdrop-tag".into()];
    assert_eq!(card_image_types(&item), &["Backdrop", "Primary"]);
}

#[test]
fn poster_only_movie_card_still_takes_the_landscape_chain() {
    let mut item = mbv_emby_model::test_support::make_item("Feature", "Movie");
    item.id = "item".into();
    item.image_tags.primary = "primary-tag".into();
    assert_eq!(card_cache_key(&item), "item:QB");
    assert_eq!(card_image_types(&item), &["Backdrop", "Primary"]);
}

#[rstest::rstest]
#[case::movie(Some("Movie"), &["Backdrop", "Primary"])]
#[case::episode(Some("Episode"), &["Primary", "Thumb", "Backdrop", "Logo"])]
#[case::audio(Some("Audio"), &["Primary"])]
#[case::unknown(None, &["Primary"])]
fn slotless_chain_reserves_landscape_for_movies_only(
    #[case] item_type: Option<&str>,
    #[case] expected: &[&str],
) {
    assert_eq!(slotless_card_image_types(item_type), expected);
}

#[rstest::rstest]
#[case::movie_with_logo("Movie", "logo-tag", "", Some(("item:Logo:logo-tag", "item", "")))]
#[case::movie_without_logo("Movie", "", "", None)]
#[case::episode_with_series("Episode", "", "series", Some(("series:Logo", "item", "series")))]
#[case::episode_without_series("Episode", "", "", None)]
#[case::audio("Audio", "", "", None)]
#[case::music_album("MusicAlbum", "", "", None)]
#[case::video("Video", "", "", None)]
fn overlay_logo_owner_is_movie_or_episode_series_only(
    #[case] item_type: &str,
    #[case] logo_tag: &str,
    #[case] series_id: &str,
    #[case] expected: Option<(&str, &str, &str)>,
) {
    let mut item = mbv_emby_model::test_support::make_item("Item", item_type);
    item.id = "item".into();
    item.image_tags.logo = logo_tag.into();
    item.series_id = series_id.into();
    let owner = overlay_logo_source(&item);
    assert_eq!(
        owner.as_ref().map(|logo| (
            logo.cache_key.as_str(),
            logo.item_id.as_str(),
            logo.series_id.as_str()
        )),
        expected
    );
}

#[rstest::rstest]
#[case::painted(Some(KEY), NowPlayingTitleSite::Artwork)]
#[case::not_yet_painted(None, NowPlayingTitleSite::Header)]
#[case::other_variant_painted(Some("art:t:8x4:2"), NowPlayingTitleSite::Header)]
fn chooses_site_from_painted_fact(
    #[case] painted_key: Option<&str>,
    #[case] expected: NowPlayingTitleSite,
) {
    assert_eq!(resolve_title_site(KEY, painted_key), expected);
}

#[test]
fn dim_backdrop_suffix_does_not_change_emby_or_audiobookshelf_identity() {
    use mbv_images::title_overlay::{TitleOverlayText, title_overlay_cache_key};
    let title = TitleOverlayText {
        context: None,
        title: "title",
    };
    let emby = title_overlay_cache_key("item:P", 8, 4, title, None);
    assert!(emby.starts_with("item:P:t:8x4:"));

    let kitty =
        title_overlay_cache_key("audiobookshelf:server:cover:item:kitty", 8, 4, title, None);
    let halfblock = title_overlay_cache_key(
        "audiobookshelf:server:cover:item:halfblock",
        8,
        4,
        title,
        None,
    );
    assert_eq!(kitty, halfblock);
    assert!(kitty.starts_with("audiobookshelf:server:cover:item:t:8x4:"));
}

#[test]
fn overlay_painted_fact_requires_overlay_key() {
    const KEY: &str = "item:P:t:8x4:1";
    assert_eq!(painted_overlay_key(Some("item:P")), None);
    assert_eq!(painted_overlay_key(Some(KEY)), Some(KEY));
}

fn cached_colour_image(width: u32, height: u32) -> CachedImage {
    CachedImage {
        img: Some(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(width, height, image::Rgba([20, 40, 60, 255])),
        )),
        protocols: std::collections::HashMap::new(),
        cover_box: None,
        applied_logo_key: None,
    }
}

/// An active-playing queue app with the card protocol and one base bitmap, so
/// the title-site decision can reach the overlay-variant build.
fn overlay_app() -> App {
    let mut app = make_queue_app(1);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    };
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.images.configure_protocol(None, true);
    app.images
        .set_image_pickers_for_test(picker, Picker::halfblocks());
    // (height, width): the card box the overlay is composed for.
    app.images.record_card_size(4, 8);
    let key = card_cache_key(
        app.playback_queue()
            .emby_item_at(0)
            .expect("the queue holds an Emby item"),
    );
    app.images.insert_image(key, cached_colour_image(4, 2));
    app
}

/// User-reported regression: every distinct fitted width during a queue-column
/// resize drag composed a Lanczos3 title overlay synchronously on the tick
/// thread. The gate must build no variant while the drag is active and compose
/// exactly one once it ends.
#[test]
fn column_resize_drag_builds_no_overlay_variant_until_it_ends() {
    let mut app = overlay_app();
    let base_key = card_cache_key(
        app.playback_queue()
            .emby_item_at(0)
            .expect("the queue holds an Emby item"),
    );
    app.cached_image_protocol_mut(&base_key)
        .expect("the base card protocol is available");
    let baseline = app.images.image_protocol_builds();

    app.refresh_queue_card_image(true);
    assert_eq!(
        app.images.image_protocol_builds(),
        baseline,
        "no overlay variant is composed while the column drag is active"
    );
    assert_eq!(
        app.queue_card_projection.cache_key.as_deref(),
        Some(base_key.as_str()),
        "the card paints plain base art during the drag"
    );

    app.refresh_queue_card_image(false);
    assert_eq!(
        app.images.image_protocol_builds(),
        baseline + 1,
        "the drag's final width composes the overlay once"
    );
    assert!(
        app.queue_card_projection
            .cache_key
            .as_deref()
            .is_some_and(|key| key.contains(":t:")),
        "the ended drag projects the overlay variant: {:?}",
        app.queue_card_projection.cache_key
    );
}
