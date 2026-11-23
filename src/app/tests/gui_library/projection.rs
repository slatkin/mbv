//! Tests of the GUI Music projection's Hero, per-group picks, and neighbour
//! warm window (add-gui-library-panel design D13 and D5; gui-collection-dropdown
//! "Each Music group keeps its last picked album"): a group with no pick
//! heroes its first album, each pill reopens on its own pick, the pick's
//! neighbours warm once navigation is idle, the Hero carries the album's
//! facts and tracklist, and a widening selection carries no album the Hero
//! never showed. The tests sit in their own child module of `gui_library` so
//! that file stays within its line ceiling.

use super::*;
use crate::app::dispatch::library::artist_lookup::ArtistNameLookup;
use mbv_ui_model::music_grouping::ArtistKey;
/// The album the Hero shows, by Service id (add-gui-library-panel design D13).
fn hero_album_id(app: &mut App) -> Option<String> {
    gui_music_projection(app, 0)
        .hero
        .map(|hero| hero.album.album_id)
}

/// A group with no pick heroes its first album in list order, so the Hero is
/// never blank while the group's albums are loaded (design D13).
#[test]
fn a_group_with_no_pick_heroes_its_first_album() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        Vec::new(),
    );
    let first = gui_music_projection(&mut app, 0).artists[0].albums[0]
        .album_id
        .clone();

    assert_eq!(hero_album_id(&mut app), Some(first));
}

/// Each pill keeps its own last picked album: a pick on one group survives a
/// switch to another, and reselecting the group reopens on the pick, while the
/// other group with no pick heroes its first album (design D13).
#[test]
fn each_pill_reopens_on_its_own_last_pick() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        vec![stored_album(
            "alb-3ft",
            "3 Feet High and Rising",
            "De La Soul",
            1989,
        )],
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-paul".into(),
        },
    );

    select_group(&mut app, SOUL);
    assert_eq!(hero_album_id(&mut app), Some("alb-3ft".to_string()));

    select_group(&mut app, HIPHOP);
    assert_eq!(hero_album_id(&mut app), Some("alb-paul".to_string()));
}

/// A stub `App` whose Music store holds `albums` and whose Emby runtime holds
/// no client: a warm's fetch reservation stays visible, and no fetch leaves
/// the process.
fn app_with_warm_albums(albums: Vec<mbv_emby_model::EmbyItem>) -> App {
    let mut app = app_with_stored_groups(albums, Vec::new());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::default();
    app.images
        .configure_protocol(Some("kitty".to_string()), true);
    app
}

/// Six albums of one artist, named so the projection's sort order is the
/// declared `alb-0` .. `alb-5` order.
fn six_warm_albums() -> Vec<mbv_emby_model::EmbyItem> {
    (0..6)
        .map(|index| {
            stored_album(
                &format!("alb-{index}"),
                &format!("Album {index}"),
                "Beastie Boys",
                1986,
            )
        })
        .collect()
}

/// The albums neighbouring the pick are warmed, one behind and three ahead:
/// the TUI's grouped-Music neighbour window
/// (`openspec/specs/music-library-hero/spec.md`) over this Client's
/// projection. The pick itself is the Hero's own fetch.
#[test]
fn the_picked_albums_neighbours_are_warmed() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_warm_albums(six_warm_albums());
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-3".into(),
        },
    );
    // The pick's own move closed the warm gate; the next idle pass opens it.
    app.last_nav_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap();

    let projection = gui_music_projection(&mut app, 0);

    assert_eq!(
        projection
            .hero
            .as_ref()
            .map(|hero| hero.album.album_id.as_str()),
        Some("alb-3"),
        "alb-3 is the picked album"
    );
    let warmed: Vec<String> = (0..6)
        .map(|index| format!("alb-{index}:P"))
        .filter(|key| app.images.is_loading(key))
        .collect();
    assert_eq!(
        warmed,
        ["alb-2:P", "alb-4:P", "alb-5:P"],
        "one behind and three ahead warm; the pick and the two behind do not"
    );
}

/// A pick move warms nothing: it stamps navigation, so the neighbours wait for
/// the idle window and fast arrow traffic issues no fetch (the TUI's idle
/// gate).
#[test]
fn a_pick_move_warms_no_neighbour_until_navigation_is_idle() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_warm_albums(six_warm_albums());
    app.last_nav_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap();

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-3".into(),
        },
    );
    let projection = gui_music_projection(&mut app, 0);

    assert!(projection.hero.is_some(), "the pick still heroes its album");
    let warmed: Vec<String> = (0..6)
        .map(|index| format!("alb-{index}:P"))
        .filter(|key| app.images.is_loading(key))
        .collect();
    assert!(
        warmed.is_empty(),
        "a pick move closes the gate, so no neighbour art warms: {warmed:?}"
    );
}

/// The Hero's facts for the picked album: the artist header, the album block,
/// and the tracklist from the cached tracks (add-gui-library-panel design D5).
#[test]
fn a_picked_album_with_cached_tracks_yields_the_hero_facts_and_tracklist() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        Vec::new(),
    );
    app.album_tracks_cache.insert(
        "alb-ill".to_string(),
        vec![
            cached_track("trk-1", "Rhymin & Stealin", 1, 248),
            cached_track("trk-2", "The New Style", 2, 275),
        ],
    );
    app.album_tracks_cache.insert(
        "alb-paul".to_string(),
        vec![cached_track("trk-3", "Shake Your Rump", 1, 195)],
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-ill".into(),
        },
    );

    let hero = gui_music_projection(&mut app, 0)
        .hero
        .expect("the picked album has a Hero");

    assert_eq!(hero.artist, "Beastie Boys");
    assert_eq!(hero.collections, ["Hip-Hop"]);
    assert_eq!(hero.album_count, 2, "the artist's albums in the union");
    assert_eq!(hero.album.year, "1986");
    assert_eq!(hero.album.name, "Licensed to Ill");
    assert_eq!(
        hero.album.total,
        Some(GuiMusicTrackTotal {
            count: 2,
            duration: "8:43".to_string(),
        })
    );
    let tracklist: Vec<(i64, &str, &str)> = hero
        .album
        .tracks
        .iter()
        .flatten()
        .map(|track| (track.number, track.name.as_str(), track.duration.as_str()))
        .collect();
    assert_eq!(
        tracklist,
        [
            (1, "Rhymin & Stealin", "4:08"),
            (2, "The New Style", "4:35")
        ]
    );
    assert_eq!(
        hero.artist_tracks,
        Some(GuiMusicTrackTotal {
            count: 3,
            duration: "11:58".to_string(),
        }),
        "the artist header counts every loaded album's tracks"
    );
}

/// The artist header's track count and total time wait for every one of the
/// artist's album tracklists (add-gui-library-panel design D5).
#[test]
fn the_artist_total_waits_for_every_albums_tracklist() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        Vec::new(),
    );
    app.album_tracks_cache.insert(
        "alb-ill".to_string(),
        vec![cached_track("trk-1", "Rhymin & Stealin", 1, 248)],
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-ill".into(),
        },
    );

    let hero = gui_music_projection(&mut app, 0)
        .hero
        .expect("the picked album has a Hero");

    assert!(
        hero.album.total.is_some(),
        "the picked album's tracks are cached"
    );
    assert_eq!(
        hero.artist_tracks, None,
        "the artist's second album holds no tracklist yet"
    );
}

/// A second checked group keeps the Hero (gui-collection-dropdown "Each Music
/// group keeps its last picked album"): the pick the one checked group kept
/// carries into the session's union pick on the selection change, so the Hero
/// still shows Paul's Boutique while the union holds it.
#[test]
fn a_second_group_keeps_the_hero() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-paul",
            "Paul's Boutique",
            "Beastie Boys",
            1989,
        )],
        vec![stored_album(
            "alb-3ft",
            "3 Feet High and Rising",
            "De La Soul",
            1989,
        )],
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-paul".into(),
        },
    );

    crate::app::tests::stub_gui_state(|gui| {
        install_library_collection(
            &mut app,
            gui,
            0,
            GuiCollectionSet::Checked(vec![HIPHOP.to_string(), SOUL.to_string()]),
        );
    });

    assert_eq!(
        hero_album_id(&mut app),
        Some("alb-paul".to_string()),
        "the Hero keeps its album while the union holds it"
    );
}

/// A pick under the one effectively-checked group is the kept per-group pick
/// (gui-collection-dropdown "Each Music group keeps its last picked album",
/// review P2): the pick rule reads the effective set the projection paints,
/// so a stored set the catalog trimmed to one group still saves the pick to
/// `library_picks`, which the launch file persists — never to the
/// session-only union pick.
#[test]
fn a_pick_under_the_effectively_single_group_is_kept() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-paul",
            "Paul's Boutique",
            "Beastie Boys",
            1989,
        )],
        Vec::new(),
    );
    // The catalog refresh dropped Soul while the stored set still checks
    // both groups: the effective selection is one group.
    app.libs[0].nav_stack[0]
        .items
        .retain(|group| group.id == HIPHOP);
    crate::app::tests::stub_gui_state(|gui| {
        gui.library_groups.insert(
            0,
            GuiCollectionSelection {
                set: GuiCollectionSet::Checked(vec![HIPHOP.to_string(), SOUL.to_string()]),
            },
        );
    });

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-paul".into(),
        },
    );

    assert_eq!(
        app.gui_library_pick_for_test(0, HIPHOP),
        Some("alb-paul".to_string()),
        "the pick lands in the kept per-group pick"
    );
    assert!(
        !crate::app::tests::stub_gui_state(|gui| gui.music_union_picks.contains_key(&0)),
        "the session-only union pick stays empty"
    );
}

/// A widening carry seeds no album the Hero never showed (design D11,
/// review P2): with All selected and no union pick, checking more groups
/// carries nothing, so resolution falls through to the union's first album
/// in artist order — never to a list-order first album of one group.
#[test]
fn a_widening_carry_seeds_no_album_the_hero_never_showed() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album("alb-zeb", "Zebra Album", "Zebra", 1990)],
        vec![stored_album("alb-aar", "Aardvark Album", "Aardvark", 1989)],
    );
    crate::app::tests::stub_gui_state(|gui| gui.library_groups.remove(&0));

    crate::app::tests::stub_gui_state(|gui| {
        install_library_collection(
            &mut app,
            gui,
            0,
            GuiCollectionSet::Checked(vec![HIPHOP.to_string(), SOUL.to_string()]),
        );
    });

    assert!(
        !crate::app::tests::stub_gui_state(|gui| gui.music_union_picks.contains_key(&0)),
        "no union pick is seeded from a list-order first album"
    );
    assert_eq!(
        hero_album_id(&mut app),
        Some("alb-aar".to_string()),
        "the union's first album in artist order is the Hero"
    );
}

/// A folder-based artist row, grouped by name, takes the Service artist item id
/// its level fill's tracks named for that name: the row requests that artist's
/// image instead of painting the media kind with no source.
#[test]
fn a_fallback_artist_row_requests_the_artist_item_its_tracks_named() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(Vec::new(), Vec::new());
    ArtistNameLookup::record(&mut app, "Beastie Boys", "artist-7".into());

    assert_eq!(
        music_artist_row_art(&app, &ArtistKey::Fallback("Beastie Boys".into())),
        GuiRowArt::Loading
    );
}

/// A Music artist row's art is the artist's own image when the Service names
/// the artist (gui-delineated-rows design D5), read from the row cache alone,
/// and the fallback identity — a name the Service never named — has no source.
/// The Music Hero shows albums, so the artist's own `Primary` is the row's
/// image; the projection's own rows carry that art beside their media kind.
#[test]
fn an_artist_row_shows_a_services_image_only_when_the_service_names_it() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        Vec::new(),
    );
    let key = mbv_images::row::emby_row_request_for(
        "artist-1",
        "",
        &mbv_emby_model::EmbyImageTags::default(),
        true,
    )
    .cache_key;
    let cover = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        72,
        72,
        image::Rgba([20, 40, 60, 255]),
    ));
    app.seed_card_image_for_test(key.clone(), cover);

    let projection = gui_music_projection(&mut app, 0);

    assert_eq!(
        projection.artists[0].art,
        GuiRowArt::NoSource,
        "a union row the Service names no artist id for has no source"
    );
    assert_eq!(projection.artists[0].media, GuiRowMediaKind::Audio);
    assert_eq!(
        music_artist_row_art(&app, &ArtistKey::Service("artist-1".into())),
        GuiRowArt::Image {
            key: key.clone(),
            square: true
        },
        "a Service-named artist shows its own cached image"
    );
    assert_eq!(
        music_artist_row_art(&app, &ArtistKey::Fallback("Beastie Boys".into())),
        GuiRowArt::NoSource
    );
}
