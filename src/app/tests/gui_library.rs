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

/// Select `group_id` for the stub's Music library (one group is always
/// selected). The fetch it started lands through `land_one_group`, as a Client
/// tick drains it.
fn select_group(app: &mut App, group_id: &str) {
    app.apply_gui_library_command(GuiLibraryCommand::SelectCollection {
        lib_idx: 0,
        group_id: group_id.to_string(),
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
    app.gui_library_groups.insert(0, HIPHOP.to_string());
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

    select_group(&mut app, HIPHOP);
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

    select_group(&mut app, HIPHOP);
    land_one_group(&mut app);
    select_group(&mut app, SOUL);
    land_one_group(&mut app);
    let requests = http.request_count();

    select_group(&mut app, HIPHOP);

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
    select_group(&mut app, HIPHOP);
    land_one_group(&mut app);

    http.fail(ErrorKind::ConnectionRefused);
    select_group(&mut app, SOUL);
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

/// One selected group is the whole list: the other group's albums stay out, and
/// the selected pill is the only one painted (add-gui-library-panel design D5,
/// D6).
#[test]
fn the_selected_group_alone_fills_the_list() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
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
        [("Hip-Hop", true), ("Soul", false)],
        "the pills paint the core's selection"
    );
    let artists: Vec<&str> = projection
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect();
    assert_eq!(
        artists,
        ["Beastie Boys"],
        "one row per artist of the selected group, in artist sort-name order"
    );
    let letters: Vec<char> = projection
        .artists
        .iter()
        .map(|artist| artist.sort_letter)
        .collect();
    assert_eq!(letters, ['B'], "each artist row carries its letter");
    let shared: Vec<&str> = projection.artists[0]
        .albums
        .iter()
        .map(|album| album.name.as_str())
        .collect();
    assert_eq!(
        shared,
        ["Licensed to Ill"],
        "the selected group's albums sit under the artist row, and no other group's"
    );
}

/// A stored group the library no longer holds falls back to the first group,
/// as the TUI's persisted pill does (design D6): one group stays selected.
#[test]
fn a_stored_group_the_library_lost_falls_back_to_the_first_group() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(Vec::new(), Vec::new());
    app.gui_library_groups.insert(0, "grp-gone".to_string());

    let projection = app.gui_music_projection(0);

    let selected: Vec<&str> = projection
        .pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.group_id.as_str())
        .collect();
    assert_eq!(selected, [HIPHOP]);
}

/// With no stored choice, the first group in pill order is the selected one
/// (design D6): one group is always selected.
#[test]
fn the_first_group_is_selected_when_no_choice_is_stored() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        Vec::new(),
    );
    app.gui_library_groups.remove(&0);

    let projection = app.gui_music_projection(0);

    let selected: Vec<&str> = projection
        .pills
        .iter()
        .filter(|pill| pill.selected)
        .map(|pill| pill.group_id.as_str())
        .collect();
    assert_eq!(selected, [HIPHOP]);
}

/// The album the Hero shows, by Service id (add-gui-library-panel design D13).
fn hero_album_id(app: &mut App) -> Option<String> {
    app.gui_music_projection(0)
        .hero
        .map(|hero| hero.album.album_id)
}

/// A group with no pick heroes its first album in list order, so the Hero is
/// never blank while the group's albums are loaded (design D13).
#[test]
fn a_group_with_no_pick_heroes_its_first_album() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        Vec::new(),
    );
    let first = app.gui_music_projection(0).artists[0].albums[0]
        .album_id
        .clone();

    assert_eq!(hero_album_id(&mut app), Some(first));
}

/// Each pill keeps its own last picked album: a pick on one group survives a
/// switch to another, and reselecting the group reopens on the pick, while the
/// other group with no pick heroes its first album (design D13).
#[test]
fn each_pill_reopens_on_its_own_last_pick() {
    let _guard = crate::config::TestStateDirGuard::new();
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
    app.apply_gui_library_command(GuiLibraryCommand::ShowDetail {
        item_id: "alb-paul".into(),
    });

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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_warm_albums(six_warm_albums());
    app.apply_gui_library_command(GuiLibraryCommand::ShowDetail {
        item_id: "alb-3".into(),
    });
    // The pick's own move closed the warm gate; the next idle pass opens it.
    app.last_nav_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap();

    let projection = app.gui_music_projection(0);

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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_warm_albums(six_warm_albums());
    app.last_nav_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap();

    app.apply_gui_library_command(GuiLibraryCommand::ShowDetail {
        item_id: "alb-3".into(),
    });
    let projection = app.gui_music_projection(0);

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
    let _guard = crate::config::TestStateDirGuard::new();
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
    app.apply_gui_library_command(GuiLibraryCommand::ShowDetail {
        item_id: "alb-ill".into(),
    });

    let hero = app
        .gui_music_projection(0)
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
    let _guard = crate::config::TestStateDirGuard::new();
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
fn a_failed_selected_group_shows_a_failure_line() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.gui_library_groups.insert(0, SOUL.to_string());
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
    assert!(
        artists.is_empty(),
        "the failed selected group holds no albums"
    );
}

/// A stub `App` whose catalog holds two Grouped Music libraries around a
/// Movies library, so the tabs present every Emby library in catalog order
/// (row 8.1 presents the plain Emby libraries beside Music). No Service
/// client: the tab projection reads the catalog alone.
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

/// The frame's destination tabs (row 4.3, design D2; row 8.1 presents the
/// plain Emby libraries beside Music): one tab per Emby library in catalog
/// order, each naming the position the open-destination command takes.
#[test]
fn the_tabs_present_every_emby_library_in_catalog_order() {
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
                position: 2,
                label: "Movies".to_string(),
            },
            GuiLibraryTab {
                position: 3,
                label: "Music B".to_string(),
            },
        ],
        "every Emby library presents a tab, and the positions are the open command's"
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

/// The current tab names the plain Emby-library destination the row 8.1
/// presented set added (row 4.3, design D2): the Movies library's tab is
/// current while it is active.
#[test]
fn the_current_tab_names_the_movies_destination() {
    let mut app = app_with_two_music_libraries();
    app.tab = TabSelection::EmbyLibrary(1);

    assert_eq!(app.gui_library_tabs().current, Some(2));
}

/// A Music item's right-click item menu offers the played-state entries no
/// item kind (the context-menu spec's music rule, through the GUI's menu
/// builder): music ignores its played state everywhere, so the mark entries
/// would name changes the UI never shows. An album-folder music item carries
/// the standard folder entries — Play All, Shuffle, Add to Queue — and
/// nothing else.
#[test]
fn a_music_items_row_menu_leaves_out_the_played_toggle() {
    let _guard = crate::config::TestStateDirGuard::new();
    let app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        Vec::new(),
    );
    let mut app = app;
    app.set_panel_focus(PanelFocus::Library);
    app.apply_gui_library_command(GuiLibraryCommand::RowMenu {
        item_id: "alb-ill".into(),
    });

    let menu = app.gui_library_row_menu().expect("the album has a menu");
    let labels: Vec<&str> = menu
        .entries
        .iter()
        .map(|entry| entry.label.as_str())
        .collect();
    assert!(!labels.iter().any(|label| label.contains("Mark")));
    assert_eq!(labels, ["Play", "Add to Queue"]);
}

/// The opener the openUrl seam stages: it records the url the core handed
/// the opener, so the test observes the open without a real browser spawn.
static OPENED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn record_open(url: &str) {
    OPENED.lock().unwrap().push(url.to_string());
}

/// The `openUrl` command's trust boundary: the core opens an http or https
/// url through the shared opener, and any other scheme — the url comes from
/// the webview — reaches no opener.
#[test]
fn open_url_opens_an_http_url_and_drops_any_other_scheme() {
    let _guard = crate::app::OPEN_URL_TEST_LOCK.lock().unwrap();
    *crate::app::OPEN_URL_OVERRIDE.lock().unwrap() = Some(record_open);
    let mut app = make_app_stub();
    app.apply_gui_library_command(GuiLibraryCommand::OpenUrl {
        url: "https://www.imdb.com/title/tt0088763/".to_string(),
    });
    app.apply_gui_library_command(GuiLibraryCommand::OpenUrl {
        url: "javascript:alert(1)".to_string(),
    });
    *crate::app::OPEN_URL_OVERRIDE.lock().unwrap() = None;

    assert_eq!(
        *OPENED.lock().unwrap(),
        vec!["https://www.imdb.com/title/tt0088763/".to_string()],
        "the https url opened and the javascript url dropped"
    );
}

/// An interrupted art fetch's retry comes due once, after its retry time
/// (design D3 rule 3, invariant 27): the core learns of it from the first ask
/// past that time, and a later ask does not report the same retry again.
#[test]
fn an_interrupted_art_retry_comes_due_once_after_its_time() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.images.fetch_interrupted("alb-1:P".to_owned());
    let later = std::time::Instant::now() + std::time::Duration::from_secs(3600);

    assert!(
        !app.gui_art_retry_due(std::time::Instant::now()),
        "the retry time has not passed yet"
    );
    assert!(
        app.gui_art_retry_due(later),
        "the retry time passed since the last ask"
    );
    assert!(
        !app.gui_art_retry_due(later),
        "a retry the core already reported is not reported again"
    );
}
