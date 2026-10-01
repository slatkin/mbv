use super::{
    ActiveTitleSource, NowPlayingTitleSite, TitleArtFacts, TitleSiteFacts, TitleSitePlayback,
    TitleSlotFacts, active_title_source_skip_reason, card_cache_key, card_cache_key_for_id,
    card_image_types, overlay_or_plain_key, painted_overlay_key, resolve_title_site,
};

const KEY: &str = "art:t:8x4:1";

#[rstest::rstest]
#[case::movie("Movie", "item:QB", &["Backdrop", "Primary"])]
#[case::episode("Episode", "item:QB", &["Backdrop", "Primary"])]
#[case::audio("Audio", "item:P", &["Primary"])]
#[case::music_album(
    "MusicAlbum",
    "item:P",
    mbv_render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES
)]
fn queue_card_key_follows_hero_shape_and_preserves_music_keys(
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
#[case::episode(Some("Episode"), "item:QB")]
#[case::audio(Some("Audio"), "item:P")]
#[case::unknown(None, "item:P")]
fn slotless_card_key_uses_landscape_only_for_known_video_types(
    #[case] item_type: Option<&str>,
    #[case] expected: &str,
) {
    assert_eq!(card_cache_key_for_id("item", item_type), expected);
}

fn facts(
    playback: TitleSitePlayback,
    [
        protocol,
        box_ready,
        halfblock,
        visualizer,
        slot_shown,
        images,
        covered,
    ]: [bool; 7],
) -> TitleSiteFacts {
    TitleSiteFacts {
        playback,
        art: TitleArtFacts {
            protocol,
            halfblock,
            images,
        },
        slot: TitleSlotFacts {
            painted_box: box_ready,
            visualizer,
            visual_slot_shown: slot_shown,
        },
        covered,
    }
}

#[rstest::rstest]
#[case::active_and_painted(TitleSitePlayback::Active, [true, true, false, false, true, true, true], Some(KEY), NowPlayingTitleSite::Artwork)]
#[case::paused(TitleSitePlayback::Paused, [true, true, false, false, true, true, true], Some(KEY), NowPlayingTitleSite::Artwork)]
#[case::not_yet_painted(TitleSitePlayback::Active, [true, true, false, false, true, true, true], None, NowPlayingTitleSite::Header)]
#[case::halfblock(TitleSitePlayback::Active, [true, true, true, false, true, true, true], Some(KEY), NowPlayingTitleSite::Header)]
#[case::visualizer(TitleSitePlayback::Active, [true, true, false, true, true, true, true], Some(KEY), NowPlayingTitleSite::Header)]
#[case::idle_slot(TitleSitePlayback::Idle, [true, true, false, false, false, true, true], Some(KEY), NowPlayingTitleSite::Header)]
#[case::hidden_slot(TitleSitePlayback::Active, [true, true, false, false, false, true, true], Some(KEY), NowPlayingTitleSite::Header)]
#[case::zero_slot(TitleSitePlayback::Active, [true, false, false, false, true, true, true], Some(KEY), NowPlayingTitleSite::Header)]
#[case::no_images(TitleSitePlayback::Active, [false, true, false, false, true, false, true], Some(KEY), NowPlayingTitleSite::Header)]
#[case::uncovered_glyph(TitleSitePlayback::Active, [true, true, false, false, true, true, false], Some(KEY), NowPlayingTitleSite::Header)]
fn chooses_site_from_eligibility_and_painted_fact(
    #[case] playback: TitleSitePlayback,
    #[case] conditions: [bool; 7],
    #[case] painted_key: Option<&str>,
    #[case] expected: NowPlayingTitleSite,
) {
    assert_eq!(
        resolve_title_site(facts(playback, conditions), KEY, painted_key),
        expected
    );
}

#[rstest::rstest]
#[case::remote_slotless_title(
    ActiveTitleSource::Slotless,
    None,
    TitleSitePlayback::Active,
    NowPlayingTitleSite::Artwork
)]
#[case::remote_slotless_waits_for_title(
    ActiveTitleSource::NoTitle,
    Some("NoTitle"),
    TitleSitePlayback::Active,
    NowPlayingTitleSite::Header
)]
#[case::idle_cursor_art(
    ActiveTitleSource::NoActiveItem,
    Some("NoActiveItem"),
    TitleSitePlayback::Idle,
    NowPlayingTitleSite::Header
)]
#[case::local_active_unchanged(
    ActiveTitleSource::Local,
    None,
    TitleSitePlayback::Active,
    NowPlayingTitleSite::Artwork
)]
fn active_slotless_title_site_requires_a_title_and_painted_overlay(
    #[case] source: ActiveTitleSource,
    #[case] expected_skip_reason: Option<&str>,
    #[case] playback: TitleSitePlayback,
    #[case] expected_site: NowPlayingTitleSite,
) {
    assert_eq!(
        active_title_source_skip_reason(source),
        expected_skip_reason
    );
    assert_eq!(
        resolve_title_site(
            facts(
                playback,
                [
                    true,
                    true,
                    false,
                    false,
                    true,
                    true,
                    matches!(
                        source,
                        ActiveTitleSource::Local | ActiveTitleSource::Slotless
                    ),
                ],
            ),
            KEY,
            Some(KEY),
        ),
        expected_site
    );
}

#[test]
fn dim_backdrop_suffix_does_not_change_emby_or_audiobookshelf_identity() {
    use mbv_images::title_overlay::{TitleOverlayText, title_overlay_cache_key};
    let title = TitleOverlayText {
        context: None,
        title: "title",
    };
    let emby = title_overlay_cache_key("item:P", 8, 4, title);
    assert!(emby.starts_with("item:P:t:8x4:"));

    let kitty = title_overlay_cache_key("audiobookshelf:server:cover:item:kitty", 8, 4, title);
    let halfblock =
        title_overlay_cache_key("audiobookshelf:server:cover:item:halfblock", 8, 4, title);
    assert_eq!(kitty, halfblock);
    assert!(kitty.starts_with("audiobookshelf:server:cover:item:t:8x4:"));
}

#[test]
fn projected_card_key_uses_variant_only_when_eligible() {
    assert_eq!(overlay_or_plain_key("item:P", None), "item:P");
    assert_eq!(
        overlay_or_plain_key("item:P", Some("item:P:t:8x4:1".to_owned())),
        "item:P:t:8x4:1"
    );
}

#[test]
fn overlay_painted_fact_requires_successful_paint() {
    const KEY: &str = "item:P:t:8x4:1";
    assert_eq!(painted_overlay_key(Some(KEY), false), None);
    assert_eq!(painted_overlay_key(Some("item:P"), true), None);
    assert_eq!(painted_overlay_key(Some(KEY), true), Some(KEY));
}
