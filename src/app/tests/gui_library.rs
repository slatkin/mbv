//! Tests of the GUI Library's Music collection store and its projection
//! (add-gui-library-panel design D5, rows 3.2 and 3.3): a selected group's
//! albums land in the store outside `nav_stack`, a loaded group is not fetched
//! again, one failing group keeps the other groups' albums, and the projection
//! unions the selected groups' artists with their facts. The Emby boundary is
//! the in-memory mock transport, so no request leaves the process.

use std::io::ErrorKind;

use super::*;
use crate::app::gui_library::GuiGroupAlbums;
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

const LIB_ID: &str = "lib-music";
const HIPHOP: &str = "grp-hiphop";
const SOUL: &str = "grp-soul";

/// One album of a group's listing, as the mock transport returns it.
const ALBUMS: &str = r#"{"Items":[{"Id":"alb-ill","Name":"Licensed to Ill","Type":"MusicAlbum"},{"Id":"alb-paul","Name":"Paul's Boutique","Type":"MusicAlbum"}],"TotalRecordCount":2}"#;

/// A group row of a music group level.
fn group_item(id: &str, name: &str) -> mbv_emby_model::EmbyItem {
    let mut item = make_item(name, "MusicArtist");
    item.id = id.to_string();
    item.is_folder = true;
    item
}

/// A browse level over `items`, hanging off `parent_id`.
fn level(parent_id: &str, title: &str, items: Vec<mbv_emby_model::EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        parent_id: parent_id.to_string(),
        title: title.to_string(),
        items,
        rows: ServerRows::new(0),
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".to_string(),
        sort_order: "Ascending".to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// A stub `App` whose Emby runtime holds a client over `http`, with a grouped
/// Music library active: the group level holds Hip-Hop and Soul, and the album
/// level holds Hip-Hop's.
fn app_with_music_groups(http: &MockHttp) -> App {
    let mut app = make_app_stub();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    app.music_levels = vec!["group".to_string(), "album".to_string()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = LIB_ID.into();
    library.collection_type = "music".into();
    let mut tab = LibraryTab::new(library);
    tab.nav_stack.push(level(
        LIB_ID,
        "Music",
        vec![group_item(HIPHOP, "Hip-Hop"), group_item(SOUL, "Soul")],
    ));
    tab.nav_stack.push(level(HIPHOP, "Hip-Hop", Vec::new()));
    app.libs.push(tab);
    app.tab = TabSelection::EmbyLibrary(0);
    app
}

/// Select `group_ids` for the stub's Music library and land the fetch
/// completions the selection started, as a Client tick drains them.
fn select_groups(app: &mut App, group_ids: &[&str]) {
    app.apply_gui_library_command(GuiLibraryCommand::SetCollections {
        lib_idx: 0,
        group_ids: group_ids.iter().map(|id| (*id).to_string()).collect(),
    });
}

/// One album as the store holds it, with the display artist and year the
/// projection's facts come from.
fn stored_album(id: &str, name: &str, artist: &str, year: u32) -> mbv_emby_model::EmbyItem {
    let mut item = make_item(name, "MusicAlbum");
    item.id = id.to_string();
    item.artist = artist.to_string();
    item.production_year = year;
    item
}

/// One cached track of an album, `seconds` long.
fn cached_track(id: &str, name: &str, number: i64, seconds: i64) -> mbv_emby_model::EmbyItem {
    let mut item = make_item(name, "Audio");
    item.id = id.to_string();
    item.media_type = "Audio".to_string();
    item.index_number = number;
    item.runtime_ticks = seconds * TICKS_PER_SECOND;
    item
}

/// A stub `App` with Hip-Hop and Soul selected and both groups' album lists
/// already in the store, so a projection test reads the projection alone.
fn app_with_stored_groups(
    hiphop: Vec<mbv_emby_model::EmbyItem>,
    soul: Vec<mbv_emby_model::EmbyItem>,
) -> App {
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.gui_library_groups
        .insert(0, vec![HIPHOP.to_string(), SOUL.to_string()]);
    app.gui_music_albums
        .insert((0, HIPHOP.to_string()), GuiGroupAlbums::Loaded(hiphop));
    app.gui_music_albums
        .insert((0, SOUL.to_string()), GuiGroupAlbums::Loaded(soul));
    app
}

/// Drain one started group's fetch completion, the Client tick's library
/// drain over the mocked transport's answer.
fn land_one_group(app: &mut App) {
    let event = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the group's album fetch completed");
    app.handle_lib_event(event);
}

#[test]
fn selecting_a_group_lands_its_albums_in_the_store_without_touching_the_nav_stack() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(200, ALBUMS);
    let mut app = app_with_music_groups(&http);
    let stack_before: Vec<String> = app.libs[0]
        .nav_stack
        .iter()
        .map(|level| level.parent_id.clone())
        .collect();

    select_groups(&mut app, &[HIPHOP]);
    land_one_group(&mut app);

    assert_eq!(
        app.gui_group_album_ids_for_test(0, HIPHOP),
        Some(vec!["alb-ill".to_string(), "alb-paul".to_string()]),
        "the selected group's whole album list lands in the store"
    );
    let stack_after: Vec<String> = app.libs[0]
        .nav_stack
        .iter()
        .map(|level| level.parent_id.clone())
        .collect();
    assert_eq!(stack_after, stack_before, "nav_stack does not change");
}

#[test]
fn reselecting_a_loaded_group_starts_no_fetch() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(200, ALBUMS);
    http.respond(200, ALBUMS);
    let mut app = app_with_music_groups(&http);

    select_groups(&mut app, &[HIPHOP]);
    land_one_group(&mut app);
    select_groups(&mut app, &[HIPHOP, SOUL]);
    land_one_group(&mut app);
    let requests = http.request_count();

    select_groups(&mut app, &[HIPHOP]);

    assert_eq!(http.request_count(), requests, "a loaded group: no fetch");
    assert!(
        app.channels.lib_rx.try_recv().is_err(),
        "a loaded group's reselect lands no event"
    );
}

#[test]
fn one_failing_group_keeps_the_other_groups_albums() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(200, ALBUMS);
    let mut app = app_with_music_groups(&http);
    select_groups(&mut app, &[HIPHOP]);
    land_one_group(&mut app);

    http.fail(ErrorKind::ConnectionRefused);
    select_groups(&mut app, &[HIPHOP, SOUL]);
    land_one_group(&mut app);

    assert_eq!(
        app.gui_group_album_ids_for_test(0, SOUL),
        None,
        "the failed group holds no album list"
    );
    assert_eq!(
        app.gui_group_album_ids_for_test(0, HIPHOP),
        Some(vec!["alb-ill".to_string(), "alb-paul".to_string()]),
        "the other group keeps its albums"
    );
}

/// The projection's union: two groups that share one artist yield that artist
/// once, in artist sort-name order, with its sort letter, and the pills paint
/// the core's selection (add-gui-library-panel design D5).
#[test]
fn two_groups_sharing_an_artist_yield_it_once_in_order_with_its_letter() {
    let _guard = crate::config::TestStateDirGuard::new();
    let app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        vec![
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
            stored_album("alb-3ft", "3 Feet High and Rising", "De La Soul", 1989),
        ],
    );

    let projection = app.gui_music_projection(0);

    let pills: Vec<(&str, bool)> = projection
        .pills
        .iter()
        .map(|pill| (pill.name.as_str(), pill.selected))
        .collect();
    assert_eq!(
        pills,
        [("Hip-Hop", true), ("Soul", true)],
        "the pills paint the core's selection"
    );
    let artists: Vec<&str> = projection
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect();
    assert_eq!(
        artists,
        ["Beastie Boys", "De La Soul"],
        "one row per artist, in artist sort-name order"
    );
    let letters: Vec<char> = projection
        .artists
        .iter()
        .map(|artist| artist.sort_letter)
        .collect();
    assert_eq!(letters, ['B', 'D'], "each artist row carries its letter");
    let shared: Vec<&str> = projection.artists[0]
        .albums
        .iter()
        .map(|album| album.name.as_str())
        .collect();
    assert_eq!(
        shared,
        ["Licensed to Ill", "Paul's Boutique"],
        "both groups' albums sit under the one shared artist row"
    );
}

/// The empty selection is the show-all selection (add-gui-library-panel, the
/// user's contract): the projection unions every group's albums, and the
/// pills paint no selection.
#[test]
fn an_empty_selection_unions_every_group() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        vec![stored_album(
            "alb-paul",
            "Paul's Boutique",
            "De La Soul",
            1989,
        )],
    );
    // The selection the restore would leave on a fresh launch: empty.
    app.gui_library_groups.insert(0, Vec::new());

    let projection = app.gui_music_projection(0);

    assert!(
        projection.pills.iter().all(|pill| !pill.selected),
        "the pills paint no selection"
    );
    let artists: Vec<&str> = projection
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect();
    assert_eq!(
        artists,
        ["Beastie Boys", "De La Soul"],
        "the empty selection shows every group's albums"
    );
}

/// The Hero's facts for the picked album: the artist header, the album block,
/// and the tracklist from the cached tracks (add-gui-library-panel design D5).
#[test]
fn a_picked_album_with_cached_tracks_yields_the_hero_facts_and_tracklist() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        vec![stored_album(
            "alb-paul",
            "Paul's Boutique",
            "Beastie Boys",
            1989,
        )],
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
    app.apply_gui_library_command(GuiLibraryCommand::ShowDetail {
        item_id: "alb-ill".into(),
    });

    let hero = app
        .gui_music_projection(0)
        .hero
        .expect("the picked album has a Hero");

    assert_eq!(hero.artist, "Beastie Boys");
    assert_eq!(hero.collections, ["Hip-Hop", "Soul"]);
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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        vec![stored_album(
            "alb-paul",
            "Paul's Boutique",
            "Beastie Boys",
            1989,
        )],
    );
    app.album_tracks_cache.insert(
        "alb-ill".to_string(),
        vec![cached_track("trk-1", "Rhymin & Stealin", 1, 248)],
    );
    app.apply_gui_library_command(GuiLibraryCommand::ShowDetail {
        item_id: "alb-ill".into(),
    });

    let hero = app
        .gui_music_projection(0)
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

/// A group that failed to load keeps the other groups' albums and names itself
/// in the projection's failure line (add-gui-library-panel design D5).
#[test]
fn a_failed_group_keeps_the_other_albums_and_shows_a_failure_line() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.gui_library_groups
        .insert(0, vec![HIPHOP.to_string(), SOUL.to_string()]);
    app.gui_music_albums.insert(
        (0, HIPHOP.to_string()),
        GuiGroupAlbums::Loaded(vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )]),
    );
    app.gui_music_albums
        .insert((0, SOUL.to_string()), GuiGroupAlbums::Failed);

    let projection = app.gui_music_projection(0);

    assert_eq!(
        projection.failure.as_deref(),
        Some("Couldn't load albums: Soul"),
        "the failure line names the group whose album list failed"
    );
    let artists: Vec<&str> = projection
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect();
    assert_eq!(
        artists,
        ["Beastie Boys"],
        "the loaded group keeps its albums"
    );
}

/// A stub `App` whose catalog holds two Grouped Music libraries around a
/// Movies library, so the tabs present the Music libraries only, in catalog
/// order. No Service client: the tab projection reads the catalog alone.
fn app_with_two_music_libraries() -> App {
    let mut app = make_app_stub();
    app.music_levels = vec!["group".to_string(), "album".to_string()];
    for (name, collection_type) in [
        ("Music A", "music"),
        ("Movies", "movies"),
        ("Music B", "music"),
    ] {
        let mut library = make_item(name, "CollectionFolder");
        library.collection_type = collection_type.to_string();
        library.is_folder = true;
        app.libs.push(LibraryTab::new(library));
    }
    app.tab = TabSelection::EmbyLibrary(0);
    app
}

/// The frame's destination tabs (row 4.3, design D2): one tab per Grouped
/// Music library — the destinations phase 1 presents — in catalog order, each
/// naming the position the open-destination command takes, and no tab for a
/// library the GUI does not present.
#[test]
fn the_tabs_present_only_the_grouped_music_libraries_in_catalog_order() {
    let app = app_with_two_music_libraries();

    let tabs = app.gui_library_tabs().tabs;

    assert_eq!(
        tabs,
        vec![
            GuiLibraryTab {
                position: 1,
                label: "Music A".to_string(),
            },
            GuiLibraryTab {
                position: 3,
                label: "Music B".to_string(),
            },
        ],
        "the Movies library presents no tab, and the positions are the open command's"
    );
}

/// The current tab (row 4.3, design D2): the active tab's position while the
/// active destination is one of the presented ones.
#[test]
fn the_current_tab_names_the_active_presented_destination() {
    let mut app = app_with_two_music_libraries();
    app.tab = TabSelection::EmbyLibrary(2);

    assert_eq!(app.gui_library_tabs().current, Some(3));
}

/// No tab is current while the active destination is one the GUI does not
/// present (row 4.3, design D2): the top bar marks no tab.
#[test]
fn no_tab_is_current_while_a_destination_the_gui_does_not_present_is_active() {
    let mut app = app_with_two_music_libraries();
    app.tab = TabSelection::EmbyLibrary(1);

    assert_eq!(app.gui_library_tabs().current, None);
}
