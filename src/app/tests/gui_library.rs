//! Tests of the GUI Library's Music collection store (add-gui-library-panel
//! design D5, row 3.2): a selected group's albums land in the store outside
//! `nav_stack`, a loaded group is not fetched again, and one failing group
//! keeps the other groups' albums. The Emby boundary is the in-memory mock
//! transport, so no request leaves the process.

use std::io::ErrorKind;

use super::*;
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
