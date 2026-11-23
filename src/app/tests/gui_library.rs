//! Tests of the GUI Library's Music collection store and its projection
//! (add-gui-library-panel design D5, rows 3.2 and 3.3): a selected group's
//! albums land in the store outside `nav_stack`, a loaded group is not fetched
//! again, one failing group keeps the other groups' albums, and the projection
//! unions the selected groups' artists with their facts. The Emby boundary is
//! the in-memory mock transport, so no request leaves the process. The Hero's
//! and the warm window's tests live in the `projection` child module, and the
//! presented destinations' tests in the `destinations` child module.

use std::io::ErrorKind;

use super::*;
use crate::app::gui_library::GuiGroupAlbums;
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

mod destinations;
mod launch;
mod projection;

const LIB_ID: &str = "lib-music";
const HIPHOP: &str = "grp-hiphop";
const SOUL: &str = "grp-soul";

/// One album of a group's listing, as the mock transport returns it.
const ALBUMS: &str = r#"{"Items":[{"Id":"alb-ill","Name":"Licensed to Ill","Type":"MusicAlbum"},{"Id":"alb-paul","Name":"Paul's Boutique","Type":"MusicAlbum"}],"TotalRecordCount":2}"#;

/// One album of a second group's listing, with ids the first group's list does
/// not hold, so a union assertion can tell the two lists apart.
const SOUL_ALBUMS: &str = r#"{"Items":[{"Id":"alb-3ft","Name":"3 Feet High and Rising","Type":"MusicAlbum"}],"TotalRecordCount":1}"#;

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

/// Select `group_id` for the stub's Music library as its one checked group
/// (one group is always selected). The fetch it started lands through
/// `land_one_group`, as a Client tick drains it. The selection installs
/// directly, so these store tests read the one-group rule the collection
/// commands build on.
fn select_group(app: &mut App, group_id: &str) {
    app.install_gui_library_collection(0, GuiCollectionSet::Checked(vec![group_id.to_string()]));
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
    app.gui.library_groups.insert(
        0,
        GuiCollectionSelection {
            set: GuiCollectionSet::Checked(vec![HIPHOP.to_string()]),
        },
    );
    app.gui
        .music_albums
        .entry(0)
        .or_default()
        .insert(HIPHOP.to_string(), GuiGroupAlbums::Loaded(hiphop));
    app.gui
        .music_albums
        .entry(0)
        .or_default()
        .insert(SOUL.to_string(), GuiGroupAlbums::Loaded(soul));
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
    app.handle_lib_event(event, None);
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

/// A failed group's album list refetches when the user presses All
/// (gui-collection-dropdown design D5): a selection change is the one refetch
/// trigger, so the All press drops the failed marks of the selection's groups
/// and the ensure step starts their fetches again, one at a time.
#[test]
fn pressing_all_refetches_a_failed_group() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.fail(ErrorKind::ConnectionRefused);
    let mut app = app_with_music_groups(&http);
    select_group(&mut app, HIPHOP);
    land_one_group(&mut app);
    assert_eq!(
        app.gui_group_album_ids_for_test(0, HIPHOP),
        None,
        "the failed group holds no album list"
    );

    http.respond(200, ALBUMS);
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::CollectionAll {
            target: GuiCollectionTarget::Music(0),
        },
    );
    land_one_group(&mut app);

    assert_eq!(
        app.gui_group_album_ids_for_test(0, HIPHOP),
        Some(vec!["alb-ill".to_string(), "alb-paul".to_string()]),
        "the All press refetched the failed group"
    );

    // The rest of All's groups fetch one at a time on later frames, and the
    // union holds every group's albums once their fetches land.
    http.respond(200, SOUL_ALBUMS);
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Music { lib_idx: 0 });
    land_one_group(&mut app);
    let projection = app.gui_music_projection(0);
    assert_eq!(
        app.gui_group_album_ids_for_test(0, SOUL),
        Some(vec!["alb-3ft".to_string()]),
        "All fetched the group the failed selection had not started"
    );
    assert!(
        projection.failure.is_none(),
        "the refetch cleared the failure line"
    );
}

/// A frame's ensure never refetches a failed group (gui-collection-dropdown
/// design D5): a hard failure retries only on a selection change, so a paint
/// never re-issues the fetch.
#[test]
fn an_ensure_call_does_not_refetch_a_failed_group() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.fail(ErrorKind::ConnectionRefused);
    let mut app = app_with_music_groups(&http);
    select_group(&mut app, HIPHOP);
    land_one_group(&mut app);

    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Music { lib_idx: 0 });
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Music { lib_idx: 0 });

    assert!(
        app.channels.lib_rx.try_recv().is_err(),
        "a failed group's ensure starts no new fetch"
    );
}

/// Checking one group and then pressing All fetches the group the checked
/// selection did not hold and unions every group's albums (gui-collection-
/// dropdown "The selection is All or checked collections").
#[test]
fn checking_one_group_then_pressing_all_unions_every_group() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond(200, ALBUMS);
    let mut app = app_with_music_groups(&http);
    select_group(&mut app, HIPHOP);
    land_one_group(&mut app);

    http.respond(200, SOUL_ALBUMS);
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::CollectionAll {
            target: GuiCollectionTarget::Music(0),
        },
    );
    land_one_group(&mut app);

    assert_eq!(
        app.gui_group_album_ids_for_test(0, SOUL),
        Some(vec!["alb-3ft".to_string()]),
        "All fetched the group the checked selection did not hold"
    );
    assert_eq!(
        app.gui_group_album_ids_for_test(0, HIPHOP),
        Some(vec!["alb-ill".to_string(), "alb-paul".to_string()]),
        "the checked group keeps its albums"
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

    let rows: Vec<(&str, bool)> = checked_rows(&projection);
    assert_eq!(
        rows,
        [("Hip-Hop", true), ("Soul", false)],
        "the dropdown paints the core's selection"
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

/// The collection toggle checks two groups in turn (gui-collection-dropdown
/// design D5): the Music destination toggles on the effective set and installs
/// each change through its one selection entry, so two presses on two groups
/// check both and the list unions their albums.
#[test]
fn a_collection_toggle_checks_two_groups() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        vec![stored_album(
            "alb-3ft",
            "3 Feet High and Rising",
            "De La Soul",
            1989,
        )],
    );
    app.gui.library_groups.remove(&0);

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::CollectionToggle {
            target: GuiCollectionTarget::Music(0),
            key: HIPHOP.to_string(),
        },
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::CollectionToggle {
            target: GuiCollectionTarget::Music(0),
            key: SOUL.to_string(),
        },
    );

    assert_eq!(
        app.gui.library_groups.get(&0).map(|stored| &stored.set),
        Some(&GuiCollectionSet::Checked(vec![
            HIPHOP.to_string(),
            SOUL.to_string(),
        ])),
        "two presses check two groups"
    );
    let projection = app.gui_music_projection(0);
    let artists: Vec<&str> = projection
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect();
    assert_eq!(
        artists,
        ["Beastie Boys", "De La Soul"],
        "the union lists both checked groups' artists"
    );
}

/// A stored set the library emptied falls back to All (gui-collection-dropdown
/// design D1): no group stays checked, so the list is every group's.
#[test]
fn a_stored_group_the_library_lost_falls_back_to_all() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(Vec::new(), Vec::new());
    app.gui.library_groups.insert(
        0,
        GuiCollectionSelection {
            set: GuiCollectionSet::Checked(vec!["grp-gone".to_string()]),
        },
    );

    let projection = app.gui_music_projection(0);

    assert_eq!(
        checked_names(&projection),
        Vec::<&str>::new(),
        "the emptied checked set reads as All, which checks no group"
    );
    assert_eq!(
        projection.collections.selected_names(),
        ["All"],
        "All names itself in the frame's collection names"
    );
}

/// With no stored choice, the destination starts on All (gui-collection-dropdown
/// "The selection is All or checked collections"): no group is checked and the
/// union is every group's.
#[test]
fn no_stored_choice_selects_all() {
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
    app.gui.library_groups.remove(&0);

    let projection = app.gui_music_projection(0);

    assert_eq!(
        checked_names(&projection),
        Vec::<&str>::new(),
        "All is selected, which checks no group"
    );
    assert_eq!(
        projection.collections.selected_names(),
        ["All"],
        "All names itself in the frame's collection names"
    );
}

/// The Music dropdown's rows as `(name, checked)` pairs, in catalog order.
fn checked_rows(projection: &GuiMusicProjection) -> Vec<(&str, bool)> {
    let collections = &projection.collections;
    let entries = collections.entries.iter();
    entries
        .map(|entry| (entry.name.as_str(), entry.checked))
        .collect()
}

/// The names of the Music dropdown's checked groups.
fn checked_names(projection: &GuiMusicProjection) -> Vec<&str> {
    checked_rows(projection)
        .into_iter()
        .filter(|(_, checked)| *checked)
        .map(|(name, _)| name)
        .collect()
}

/// A group that failed to load keeps the other groups' albums and names itself
/// in the projection's failure line (add-gui-library-panel design D5).
#[test]
fn a_failed_selected_group_shows_a_failure_line() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    app.gui.library_groups.insert(
        0,
        GuiCollectionSelection {
            set: GuiCollectionSet::Checked(vec![SOUL.to_string()]),
        },
    );
    app.gui
        .music_albums
        .entry(0)
        .or_default()
        .insert(SOUL.to_string(), GuiGroupAlbums::Failed);

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

/// All with no Emby client at startup fetches once the client connects
/// (gui-collection-dropdown design D5): the no-client condition is not a fetch
/// failure, so the groups stay un-fetched, a later tick's ensure starts their
/// fetches, and the union shows every group's albums.
#[test]
fn all_fetches_once_the_client_appears() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_music_groups(&http);
    let ready = std::mem::replace(
        &mut app.emby_runtime,
        crate::app::state::service_runtime::EmbyRuntime::new(true),
    );
    app.install_gui_library_collection(0, GuiCollectionSet::All);

    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Music { lib_idx: 0 });
    assert!(
        app.channels.lib_rx.try_recv().is_err(),
        "no client starts no fetch"
    );

    http.respond(200, ALBUMS);
    http.respond(200, SOUL_ALBUMS);
    app.emby_runtime = ready;
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Music { lib_idx: 0 });
    land_one_group(&mut app);
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Music { lib_idx: 0 });
    land_one_group(&mut app);

    let projection = app.gui_music_projection(0);
    assert_eq!(
        app.gui_group_album_ids_for_test(0, HIPHOP),
        Some(vec!["alb-ill".to_string(), "alb-paul".to_string()]),
        "the client's arrival fetched Hip-Hop's albums"
    );
    assert_eq!(
        app.gui_group_album_ids_for_test(0, SOUL),
        Some(vec!["alb-3ft".to_string()]),
        "the client's arrival fetched Soul's albums"
    );
    assert!(
        projection.failure.is_none(),
        "the no-client condition names no failure"
    );
}

/// The saved identity of the stub's Grouped Music library, as the launch file
/// carries it.
fn saved_music_identity() -> mbv_config::TabIdentity {
    mbv_config::TabIdentity::ServiceLibrary {
        kind: mbv_queue::ServiceKind::Emby,
        library_id: LIB_ID.to_string(),
    }
}
