//! Tests of the GUI Library's plain Emby-library data path (add-gui-library-
//! panel design D12, row 8.1): a Movies library's open lands its full root
//! list on the App through the GUI tick's own drain, a feed-view library's
//! open loads its default selection and its set-feed-group action loads
//! another group's items while a stale completion drops, the select action
//! stores the picked row and ignores a stale id, and the presented
//! destination set names the plain Emby libraries. The Emby boundary is the
//! in-memory mock transport, so no request leaves the process and no worker
//! outlives the assertion. The feed-group fetch path's tests live in the
//! `feed_fetch` child module, the feed-group refresh's in `feed_refresh`, and
//! the rows the plain and feed-group modes project in `projection`.

use super::*;

use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

mod feed_fetch;
mod feed_refresh;
mod projection;
mod root_refetch;

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

/// The first checked folder's listing: the old video and the video both
/// folders publish, with the date added the GUI's list sorts by.
const FIRST_FOLDER_BODY: &str = r#"{"Items":[{"Id":"vid-old","Name":"Old Clip","Type":"Video","DateCreated":"2020-01-01T10:00:00.0000000Z"},{"Id":"vid-shared","Name":"Shared Clip","Type":"Video","DateCreated":"2022-06-01T10:00:00.0000000Z"}],"TotalRecordCount":2}"#;

/// The second checked folder's listing: the shared video and the newest one,
/// so the merged list de-duplicates the shared row.
const SECOND_FOLDER_BODY: &str = r#"{"Items":[{"Id":"vid-shared","Name":"Shared Clip","Type":"Video","DateCreated":"2022-06-01T10:00:00.0000000Z"},{"Id":"vid-new","Name":"New Clip","Type":"Video","DateCreated":"2026-01-01T10:00:00.0000000Z"}],"TotalRecordCount":2}"#;

/// Press one folder in the Emby feed-view destination's collection dropdown.
fn toggle_folder(app: &mut App, folder_id: &str) {
    apply_gui_library_command(
        app,
        GuiLibraryCommand::CollectionToggle {
            target: GuiCollectionTarget::EmbyFeed(0),
            key: folder_id.to_string(),
        },
    );
}

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
    let mut config = app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .cached_emby_client_for_test()
        .expect("the installed setup holds a client")
        .with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);
    let mut library = make_item(name, "CollectionFolder");
    library.id = library_id.to_string();
    library.collection_type = collection_type.to_string();
    library.is_folder = true;
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app
}

/// Drain exactly one event from the lib channel, as the GUI core tick's drain
/// does (`GuiCore::drain_lib_event`), through the GUI's own stores: the
/// event's App-side half and its collected store effects apply before the next
/// event (`decouple-client-core` design D4a (c)). The store half installs in
/// the GUI state the tests keep beside the `App` (task 8.1). The TUI's
/// `App::drain_lib_event` is this with no stores. One event per call keeps a
/// pump from swallowing an event that the step it lands starts a fetch for.
fn drain_one(app: &mut App) -> bool {
    let Some(event) = app.try_recv_lib_event() else {
        return false;
    };
    stub_gui_state(|gui| app.handle_lib_event(event, Some(gui)));
    true
}

/// Land one library event as the GUI tick's drain does.
fn land(app: &mut App) {
    if drain_one(app) {
        return;
    }
    assert!(
        app.drain_lib_event_blocking_for_test(Duration::from_secs(2)),
        "the fetch completed"
    );
    assert!(drain_one(app), "the event the fetch completed is queued");
}

/// Land events until the root level's listing has landed: the open spawns
/// the folder listing and the destination Latest snapshot together, so the
/// listing's completion may follow the snapshot's event.
fn land_root_listing(app: &mut App) {
    while app.libs_for_test()[0].nav_stack[0].loading {
        land(app);
    }
}

/// Land events until the shown selection's items have landed: the completion
/// that runs the shown pass starting the selection's fetch, then that fetch's
/// own completion.
fn land_shown_selection(app: &mut App) {
    while shown_selection_unloaded(app) {
        land(app);
    }
}

/// Land events until no feed-group fetch is outstanding: the stale frame's
/// event and the fetch's own completion both ride the channel.
fn land_until_fetched(app: &mut App) {
    while shown_selection_fetching(app) {
        land(app);
    }
}

/// Whether the shown selection holds no items yet.
fn shown_selection_unloaded(app: &App) -> bool {
    app.libs_for_test()[0]
        .feed_home_video
        .as_ref()
        .is_some_and(|state| state.selected_len() == 0)
}

/// Whether a feed-group target's fetch is outstanding.
fn shown_selection_fetching(app: &App) -> bool {
    app.libs_for_test()[0]
        .feed_home_video
        .as_ref()
        .is_some_and(|state| state.fetching.is_some())
}

/// The ids one checked folder's cached list holds, as the merged list reads
/// it.
fn checked_folder_ids<'a>(app: &'a App, folder_id: &str) -> Vec<&'a str> {
    app.libs_for_test()[0]
        .feed_home_video
        .as_ref()
        .and_then(|state| {
            state
                .items
                .get(&FeedHomeVideoSelection::Folder(folder_id.to_string()))
        })
        .map(|items| items.iter().map(|item| item.id.as_str()).collect())
        .unwrap_or_default()
}

/// Opening the Movies library lands its full root list on the App through the
/// GUI tick's drain (row 8.1, contract a): the open runs the TUI's own
/// activation — the root listing the TUI also loads — and the root-list
/// store's paged fetch, whose completion lands in the store outside
/// `nav_stack`.
#[test]
fn opening_a_movies_library_lands_its_root_rows_on_the_app() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // The open's listing and the store's paged fetch read the same catalog, so
    // a path-keyed answer follows each request's own path: the concurrent
    // workers' interleaving cannot script one wrongly.
    http.respond_when("ParentId=lib-movies", 200, MOVIES_BODY);
    let mut app = emby_app(&http, "movies", MOVIES_ID, "Movies");

    gui_emby_open(&mut app, 0);
    land(&mut app);
    wait_for_client_event(&mut app);
    while drain_one(&mut app) {}

    assert_eq!(
        app.gui_emby_root_ids_for_test(0),
        Some(vec!["m-1".to_string(), "m-2".to_string()]),
        "the root-list store holds the Movies rows"
    );
    assert!(
        app.libs_for_test()[0]
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
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // Every request this open path makes — the folder listing, the
    // destination Latest snapshot, and the default selection's fetch — reads
    // the same one-video body, so the concurrent requests' order cannot
    // mis-script one: the listing holds the video, the snapshot parses it,
    // and the selection's items are the video.
    http.respond_repeated(200, VIDEO_BODY, 6);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));

    gui_emby_open(&mut app, 0);
    land_root_listing(&mut app);
    ensure_gui_destination(&mut app, GuiDestinationKind::Library { lib_idx: 0 });
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

/// The interim folder pill checks a folder without moving the TUI's own
/// selection (2.6, design D2, D3): the GUI fetches the checked folder's items
/// into the shared cache, a completion for a folder the set does not hold is
/// dropped on arrival, and the TUI's `FeedHomeVideoState::selection` stays
/// what the TUI set.
#[test]
fn checking_a_feed_folder_loads_its_items_without_moving_the_tui_selection() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_repeated(200, VIDEO_BODY, 4);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    let folder = folder_item("folder-1", "Channel 1");
    app.edit_library_for_test(0, |library| {
        library
            .nav_stack
            .push(level(FEED_ID, "YouTube", vec![folder]));
    });
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState::default());
    });

    toggle_folder(&mut app, "folder-1");
    // A completion for a folder the checked set does not hold: the stale-drop
    // rule drops it whatever order it arrives in.
    app.send_lib_event_for_test(LibEvent::Browse(
        mbv_client_core::BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "gone".into(),
            selection: FeedHomeVideoSelection::Folder("gone".into()),
            items: vec![make_item("Stray", "Video")],
        },
    ));
    land_until_fetched(&mut app);

    let state = app.libs_for_test()[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::All,
        "the TUI's own selection stays where the TUI left it"
    );
    assert_eq!(
        checked_folder_ids(&app, "folder-1"),
        vec!["vid-1"],
        "the checked folder's items landed"
    );
    assert!(
        !state
            .items
            .contains_key(&FeedHomeVideoSelection::Folder("gone".into())),
        "the stale completion stored nothing"
    );
    assert_eq!(state.fetching, None, "the live fetch retired its mark");
}

/// Two checked folders fetch one at a time and merge (2.6, design D3, D4): the
/// first uncached target's fetch runs while `fetching` names it, the second
/// starts only after the first lands, and the list is both folders' items
/// de-duplicated by id and sorted by date added, newest first.
#[test]
fn two_checked_feed_folders_fetch_in_turn_and_merge_newest_first() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // One answer per folder, so the two fetches cannot mis-script each other:
    // the shared item rides both folders' lists.
    http.respond_when("ParentId=folder-1", 200, FIRST_FOLDER_BODY);
    http.respond_when("ParentId=folder-2", 200, SECOND_FOLDER_BODY);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    app.edit_library_for_test(0, |library| {
        library.nav_stack.push(level(
            FEED_ID,
            "YouTube",
            vec![
                folder_item("folder-1", "Channel 1"),
                folder_item("folder-2", "Channel 2"),
            ],
        ));
    });
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState::default());
    });

    toggle_folder(&mut app, "folder-1");
    toggle_folder(&mut app, "folder-2");
    assert_eq!(
        app.libs_for_test()[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .fetching,
        Some(FeedHomeVideoSelection::Folder("folder-1".into())),
        "the first checked folder's fetch runs while the second waits"
    );

    land(&mut app);
    assert_eq!(
        app.libs_for_test()[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .fetching,
        None,
        "the first fetch retired its mark"
    );
    ensure_gui_destination(&mut app, GuiDestinationKind::Library { lib_idx: 0 });
    assert_eq!(
        app.libs_for_test()[0]
            .feed_home_video
            .as_ref()
            .unwrap()
            .fetching,
        Some(FeedHomeVideoSelection::Folder("folder-2".into())),
        "the second folder's fetch starts once the first lands"
    );
    land(&mut app);

    let projection = gui_emby_projection(&mut app, 0);
    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["vid-new", "vid-shared", "vid-old"],
        "the merged list is newest first, with the shared item once"
    );
    let collections = projection
        .collections
        .as_ref()
        .expect("the feed-group mode carries its collections");
    let names: Vec<&str> = collections
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Channel 1", "Channel 2"],
        "the dropdown offers each folder, with no New row"
    );
    assert!(
        collections.entries.iter().all(|entry| entry.checked),
        "both checked folders' rows are checked"
    );
    assert!(!collections.all, "All is not the selection");
    assert_eq!(
        projection.collection_names,
        ["Channel 1", "Channel 2"],
        "the location names the checked folders"
    );
}

/// The select action stores the picked row and ignores a stale id (row 8.1):
/// a row the root-list store holds becomes the Hero's item, and an id the
/// loaded list does not hold changes nothing.
#[test]
fn the_select_action_stores_the_picked_row_and_ignores_a_stale_id() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = MOVIES_ID.into();
    library.collection_type = "movies".into();
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    let mut first = make_item("A Movie", "Movie");
    first.id = "m-1".into();
    stub_gui_state(|gui| {
        gui.emby_roots.insert(
            0,
            crate::gui_core::gui_library::GuiEmbyRootItems::Loaded(vec![first]),
        );
    });

    gui_emby_select(&mut app, 0, "m-1");
    assert_eq!(
        app.gui_emby_picked_row_for_test(0),
        Some("m-1".to_string()),
        "the picked row is the select command's"
    );

    gui_emby_select(&mut app, 0, "gone");
    assert_eq!(
        app.gui_emby_picked_row_for_test(0),
        Some("m-1".to_string()),
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
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.seed_music_levels_for_test(&["group", "album"]);
    let mut music = make_item("Music", "CollectionFolder");
    music.id = "lib-music".into();
    music.collection_type = "music".into();
    music.is_folder = true;
    app.push_library_for_test(LibraryTab::new(music));
    let mut movies = make_item("Movies", "CollectionFolder");
    movies.id = MOVIES_ID.into();
    movies.collection_type = "movies".into();
    movies.is_folder = true;
    app.push_library_for_test(LibraryTab::new(movies));
    let mut youtube = make_item("YouTube", "CollectionFolder");
    youtube.id = FEED_ID.into();
    youtube.collection_type = "homevideos".into();
    youtube.is_folder = true;
    app.push_library_for_test(LibraryTab::new(youtube));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(1));

    let tabs = gui_library_tabs(&app);
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
        gui_active_destination(&app),
        Some(crate::gui_core::gui_library::GuiDestinationKind::Library { lib_idx: 1 }),
        "the Movies destination's kind is the plain Emby-library one"
    );
}
