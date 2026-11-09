//! Tests of the GUI Library's plain Emby-library data path (add-gui-library-
//! panel design D12, row 8.1): a Movies library's open lands its full root
//! list on the App through the GUI tick's own drain, a feed-view library's
//! open loads its default selection and its set-feed-group action loads
//! another group's items while a stale completion drops, the select action
//! stores the picked row and ignores a stale id, and the presented
//! destination set names the plain Emby libraries. The Emby boundary is the
//! in-memory mock transport, so no request leaves the process and no worker
//! outlives the assertion.

use super::*;
use std::sync::Mutex;

use crate::app::state::service_runtime::EmbyRuntime;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

const MOVIES_ID: &str = "lib-movies";
const FEED_ID: &str = "lib-youtube";

/// A Movies listing of two rows, as the mock transport returns it: the
/// root-list store's fetch and the TUI root listing read the same fields, so
/// one body serves both.
const MOVIES_BODY: &str = r#"{"Items":[{"Id":"m-1","Name":"A Movie","Type":"Movie"},{"Id":"m-2","Name":"B Movie","Type":"Movie"}],"TotalRecordCount":2}"#;

/// A one-video listing: the answer a selection's items fetch parses into its
/// items, and one whose total a folder's count query reads as the count, so
/// the two requests share one scripted body whatever order they arrive in.
const VIDEO_BODY: &str =
    r#"{"Items":[{"Id":"vid-1","Name":"Clip","Type":"Video"}],"TotalRecordCount":1}"#;

/// A browse level over `items`, hanging off `parent_id`.
fn level(parent_id: &str, title: &str, items: Vec<EmbyItem>) -> BrowseLevel {
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

/// A folder row of a feed-view library's root listing.
fn folder_item(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.to_string();
    item.is_folder = true;
    item
}

/// A stub `App` whose Emby runtime holds a client over `http`, with the
/// library `collection_type` names active and no root level yet, as a first
/// open finds it.
fn emby_app(http: &MockHttp, collection_type: &str, library_id: &str, name: &str) -> App {
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
    app.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    let mut library = make_item(name, "CollectionFolder");
    library.id = library_id.to_string();
    library.collection_type = collection_type.to_string();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    app
}

/// Land one fetch completion the way the GUI core tick's drain applies it:
/// the event rides the lib channel exactly as the fetch worker sends it, and
/// `App::drain_lib_event`'s own handler applies the App-side half.
fn land(app: &mut App) {
    let event = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the fetch completed");
    app.handle_lib_event(event);
}

/// Land events until the root level's listing has landed: the open spawns
/// the folder listing and the destination Latest snapshot together, so the
/// listing's completion may follow the snapshot's event.
fn land_root_listing(app: &mut App) {
    while app.libs[0].nav_stack[0].loading {
        land(app);
    }
}

/// Land events until the shown selection's items have landed: the count
/// fan-out and the selection fetch race, so the selection's completion may
/// follow the count's event.
fn land_shown_selection(app: &mut App) {
    while app.libs[0]
        .feed_home_video
        .as_ref()
        .is_some_and(|state| state.selected_len() == 0)
    {
        land(app);
    }
}

/// Opening the Movies library lands its full root list on the App through the
/// GUI tick's drain (row 8.1, contract a): the open runs the TUI's own
/// activation — the root listing the TUI also loads — and the root-list
/// store's paged fetch, whose completion lands in the store outside
/// `nav_stack`.
#[test]
fn opening_a_movies_library_lands_its_root_rows_on_the_app() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_repeated(200, MOVIES_BODY, 4);
    let mut app = emby_app(&http, "movies", MOVIES_ID, "Movies");

    app.gui_emby_open(0);
    land(&mut app);
    land(&mut app);
    while app.drain_lib_event() {}

    assert_eq!(
        app.gui_emby_root_ids_for_test(0),
        Some(vec!["m-1".to_string(), "m-2".to_string()]),
        "the root-list store holds the Movies rows"
    );
    assert!(
        app.libs[0]
            .nav_stack
            .last()
            .is_some_and(|root| !root.loading),
        "the TUI's own root listing landed beside the store"
    );
}

/// Opening a feed-view library loads its default selection (row 8.1,
/// contract b): the open seeds the folder listing and the feed state, the
/// frame path's shown-pass trigger fetches the default All selection once the
/// listing completes, and its items land in the feed state the projection
/// reads.
#[test]
fn opening_a_feed_view_library_loads_its_default_selection() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // Every request this open path makes — the folder listing, the
    // destination Latest snapshot, and the default selection's fetch — reads
    // the same one-video body, so the concurrent requests' order cannot
    // mis-script one: the listing holds the video, the snapshot parses it,
    // and the selection's items are the video.
    http.respond_repeated(200, VIDEO_BODY, 6);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config
        .lock()
        .unwrap()
        .feed_view_libraries
        .push("youtube".into());

    app.gui_emby_open(0);
    land_root_listing(&mut app);
    app.ensure_gui_emby_selection(0);
    land_shown_selection(&mut app);

    let items = app.feed_home_video_selected_items(0);
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["vid-1"],
        "the default All selection's items landed"
    );
}

/// `gui_emby_set_feed_group` loads another group's items and drops a stale
/// completion (row 8.1, contract b): the selection write and the lazy fetch
/// run the TUI's own path, a completion for a selection the library no
/// longer shows is dropped on arrival, and the shown group's own items land.
#[test]
fn setting_another_feed_group_loads_its_items_and_drops_a_stale_completion() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_repeated(200, VIDEO_BODY, 4);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config
        .lock()
        .unwrap()
        .feed_view_libraries
        .push("youtube".into());
    let folder = folder_item("folder-1", "Channel 1");
    app.libs[0]
        .nav_stack
        .push(level(FEED_ID, "YouTube", vec![folder]));
    app.libs[0].feed_home_video = Some(FeedHomeVideoState::default());

    app.gui_emby_set_feed_group(0, &FeedHomeVideoSelection::Folder("folder-1".into()));
    // A completion for a group the shown selection is not: the stale-drop
    // rule drops it whatever order it arrives in.
    let _ = app.channels.lib_tx.send(LibEvent::Browse(
        crate::app::BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "folder-1".into(),
            selection: FeedHomeVideoSelection::Folder("gone".into()),
            items: vec![make_item("Stray", "Video")],
        },
    ));
    land_shown_selection(&mut app);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder("folder-1".into()),
        "the shown group is the set-feed-group command's"
    );
    assert_eq!(
        app.feed_home_video_selected_items(0)
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["vid-1"],
        "the shown group's own items landed; the stale completion's did not"
    );
    assert_eq!(state.fetching, None, "the live fetch retired its mark");
}

/// The select action stores the picked row and ignores a stale id (row 8.1):
/// a row the root-list store holds becomes the Hero's item, and an id the
/// loaded list does not hold changes nothing.
#[test]
fn the_select_action_stores_the_picked_row_and_ignores_a_stale_id() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = MOVIES_ID.into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    let mut first = make_item("A Movie", "Movie");
    first.id = "m-1".into();
    app.gui_emby_roots.insert(
        0,
        crate::app::gui_library::GuiEmbyRootItems::Loaded(vec![first]),
    );

    app.gui_emby_select(0, "m-1");
    assert_eq!(
        app.gui_emby_picked_row_for_test(0),
        Some("m-1"),
        "the picked row is the select command's"
    );

    app.gui_emby_select(0, "gone");
    assert_eq!(
        app.gui_emby_picked_row_for_test(0),
        Some("m-1"),
        "a stale id changes nothing"
    );
}

/// The presented destination set names every Emby library that is neither a
/// Grouped Music library nor a TV library (row 8.1, contract c): the top
/// bar's tabs carry the Movies and the feed-view library beside Music, with
/// the labels the tabs already carry, and the Movies destination's kind is
/// the plain Emby-library one the open step reads.
#[test]
fn the_presented_tabs_include_the_plain_emby_libraries() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.music_levels = vec!["group".to_string(), "album".to_string()];
    let mut music = make_item("Music", "CollectionFolder");
    music.id = "lib-music".into();
    music.collection_type = "music".into();
    music.is_folder = true;
    app.libs.push(LibraryTab::new(music));
    let mut movies = make_item("Movies", "CollectionFolder");
    movies.id = MOVIES_ID.into();
    movies.collection_type = "movies".into();
    movies.is_folder = true;
    app.libs.push(LibraryTab::new(movies));
    let mut youtube = make_item("YouTube", "CollectionFolder");
    youtube.id = FEED_ID.into();
    youtube.collection_type = "homevideos".into();
    youtube.is_folder = true;
    app.libs.push(LibraryTab::new(youtube));
    app.tab = TabSelection::EmbyLibrary(1);

    let tabs = app.gui_library_tabs();
    let labels: Vec<&str> = tabs.tabs.iter().map(|tab| tab.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Music", "Movies", "YouTube"],
        "every non-Music non-Shows Emby library presents a tab"
    );
    assert_eq!(
        tabs.current,
        Some(tabs.tabs[1].position),
        "the active Movies destination's tab is current"
    );
    assert_eq!(
        app.gui_active_destination(),
        Some(crate::app::gui_library::GuiDestinationKind::Library { lib_idx: 1 }),
        "the Movies destination's kind is the plain Emby-library one"
    );
}
