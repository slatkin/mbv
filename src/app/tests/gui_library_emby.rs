//! Tests of the GUI Library's plain Emby-library data path (add-gui-library-
//! panel design D12, row 8.1): a Movies library's open lands its full root
//! list on the App through the GUI tick's own drain, a feed-view library's
//! open loads its default selection and its set-feed-group action loads
//! another group's items while a stale completion drops, the select action
//! stores the picked row and ignores a stale id, and the presented
//! destination set names the plain Emby libraries. The Emby boundary is the
//! in-memory mock transport, so no request leaves the process and no worker
//! outlives the assertion. The feed-group fetch path's tests live in the
//! `feed_fetch` child module.

use super::*;
use std::sync::Mutex;

use crate::app::state::service_runtime::EmbyRuntime;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

mod feed_fetch;

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

/// Drain exactly one event from the lib channel, as the GUI core tick's drain
/// does (`GuiCore::drain_lib_event`), through the GUI's own stores: the
/// event's App-side half and its collected store effects apply before the next
/// event (`decouple-client-core` design D4a (c)). The store half installs in
/// the GUI state the tests keep beside the `App` (task 8.1). The TUI's
/// `App::drain_lib_event` is this with no stores. One event per call keeps a
/// pump from swallowing an event that the step it lands starts a fetch for.
fn drain_one(app: &mut App) -> bool {
    let Ok(event) = app.channels.lib_rx.try_recv() else {
        return false;
    };
    crate::app::tests::stub_gui_state(|gui| app.handle_lib_event(event, Some(gui)));
    true
}

/// Land one library event as the GUI tick's drain does.
fn land(app: &mut App) {
    if drain_one(app) {
        return;
    }
    let event = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the fetch completed");
    let _ = app.channels.lib_tx.send(event);
    assert!(drain_one(app), "the event the fetch completed is queued");
}

/// Land events until the root level's listing has landed: the open spawns
/// the folder listing and the destination Latest snapshot together, so the
/// listing's completion may follow the snapshot's event.
fn land_root_listing(app: &mut App) {
    while app.libs[0].nav_stack[0].loading {
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
    app.libs[0]
        .feed_home_video
        .as_ref()
        .is_some_and(|state| state.selected_len() == 0)
}

/// Whether a feed-group target's fetch is outstanding.
fn shown_selection_fetching(app: &App) -> bool {
    app.libs[0]
        .feed_home_video
        .as_ref()
        .is_some_and(|state| state.fetching.is_some())
}

/// The ids one checked folder's cached list holds, as the merged list reads
/// it.
fn checked_folder_ids<'a>(app: &'a App, folder_id: &str) -> Vec<&'a str> {
    app.libs[0]
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
    let _guard = crate::app::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // The open's listing and the store's paged fetch read the same catalog, so
    // a path-keyed answer follows each request's own path: the concurrent
    // workers' interleaving cannot script one wrongly.
    http.respond_when("ParentId=lib-movies", 200, MOVIES_BODY);
    let mut app = emby_app(&http, "movies", MOVIES_ID, "Movies");

    gui_emby_open(&mut app, 0);
    land(&mut app);
    crate::app::tests::wait_for_client_event(&mut app);
    while drain_one(&mut app) {}

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
    let _guard = crate::app::config::TestStateDirGuard::new();
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

    gui_emby_open(&mut app, 0);
    land_root_listing(&mut app);
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Library { lib_idx: 0 });
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
    let _guard = crate::app::config::TestStateDirGuard::new();
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

    toggle_folder(&mut app, "folder-1");
    // A completion for a folder the checked set does not hold: the stale-drop
    // rule drops it whatever order it arrives in.
    let _ = app.channels.lib_tx.send(LibEvent::Browse(
        crate::app::BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "gone".into(),
            selection: FeedHomeVideoSelection::Folder("gone".into()),
            items: vec![make_item("Stray", "Video")],
        },
    ));
    land_until_fetched(&mut app);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
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
    let _guard = crate::app::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    // One answer per folder, so the two fetches cannot mis-script each other:
    // the shared item rides both folders' lists.
    http.respond_when("ParentId=folder-1", 200, FIRST_FOLDER_BODY);
    http.respond_when("ParentId=folder-2", 200, SECOND_FOLDER_BODY);
    let mut app = emby_app(&http, "homevideos", FEED_ID, "YouTube");
    app.config
        .lock()
        .unwrap()
        .feed_view_libraries
        .push("youtube".into());
    app.libs[0].nav_stack.push(level(
        FEED_ID,
        "YouTube",
        vec![
            folder_item("folder-1", "Channel 1"),
            folder_item("folder-2", "Channel 2"),
        ],
    ));
    app.libs[0].feed_home_video = Some(FeedHomeVideoState::default());

    toggle_folder(&mut app, "folder-1");
    toggle_folder(&mut app, "folder-2");
    assert_eq!(
        app.libs[0].feed_home_video.as_ref().unwrap().fetching,
        Some(FeedHomeVideoSelection::Folder("folder-1".into())),
        "the first checked folder's fetch runs while the second waits"
    );

    land(&mut app);
    assert_eq!(
        app.libs[0].feed_home_video.as_ref().unwrap().fetching,
        None,
        "the first fetch retired its mark"
    );
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Library { lib_idx: 0 });
    assert_eq!(
        app.libs[0].feed_home_video.as_ref().unwrap().fetching,
        Some(FeedHomeVideoSelection::Folder("folder-2".into())),
        "the second folder's fetch starts once the first lands"
    );
    land(&mut app);

    let projection = crate::app::tests::gui_emby_projection(&mut app, 0);
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
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = MOVIES_ID.into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    let mut first = make_item("A Movie", "Movie");
    first.id = "m-1".into();
    crate::app::tests::stub_gui_state(|gui| {
        gui.emby_roots.insert(
            0,
            crate::app::gui_library::GuiEmbyRootItems::Loaded(vec![first]),
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
    let _guard = crate::app::config::TestStateDirGuard::new();
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
        Some(crate::app::gui_library::GuiDestinationKind::Library { lib_idx: 1 }),
        "the Movies destination's kind is the plain Emby-library one"
    );
}

/// A plain-mode root-list item with its year, runtime, and sort name.
fn movie_item(id: &str, name: &str, year: u32, runtime_ticks: i64) -> EmbyItem {
    let mut item = make_item(name, "Movie");
    item.id = id.to_string();
    item.production_year = year;
    item.runtime_ticks = runtime_ticks;
    item
}

/// A stub `App` whose plain Movies library is active, with the root-list
/// store holding `items` (row 8.1): the state a landed fetch leaves, read by
/// the projection with no Service behind it.
fn plain_movies_app(items: Vec<EmbyItem>) -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = MOVIES_ID.into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    crate::app::tests::stub_gui_state(|gui| {
        gui.emby_roots
            .insert(0, crate::app::gui_library::GuiEmbyRootItems::Loaded(items));
    });
    app
}

/// The plain mode's projection (row 8.1, contract a): the rows are the
/// root-list store's rows in the server's `SortName` order, each with its
/// rail letter over the article-stripped sort name (`#` past Z), the first
/// row is the Hero default, the Hero is the shared Emby hero producer's
/// facts with no Workspace, and the mode carries no pill at all.
#[test]
fn the_plain_mode_projects_rows_letters_and_the_hero_without_workspace() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = plain_movies_app(vec![
        {
            let mut item = movie_item(
                "m-1",
                "A Movie",
                1999,
                5400 * mbv_emby_model::TICKS_PER_SECOND,
            );
            item.premiere_date = "1999-06-01T00:00:00Z".into();
            item.people = vec![
                mbv_emby_model::EmbyPerson {
                    name: "Kirsten Johnson".into(),
                    role: String::new(),
                    kind: "Director".into(),
                },
                mbv_emby_model::EmbyPerson {
                    name: "Jane Doe".into(),
                    role: "Lead".into(),
                    kind: "Actor".into(),
                },
            ];
            item.external_urls = vec![mbv_emby_model::EmbyLink {
                name: "IMDb".into(),
                url: "https://www.imdb.com/title/tt0000001".into(),
            }];
            item
        },
        movie_item("m-2", "B Movie", 0, 900 * mbv_emby_model::TICKS_PER_SECOND),
        movie_item("m-3", "9th Street", 0, 0),
    ]);

    let projection = crate::app::tests::gui_emby_projection(&mut app, 0);

    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["m-1", "m-2", "m-3"], "the rows are the store's list");
    // The rail letter reads the article-stripped sort name, as the TUI's
    // letter buckets do: "A Movie" sorts as "Movie".
    let letters: Vec<char> = projection
        .rows
        .iter()
        .filter_map(|row| row.sort_letter)
        .collect();
    assert_eq!(letters, ['M', 'B', '#'], "the rail letters read the names");
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("1999"),
        "a year rides the row's subtitle"
    );
    assert_eq!(
        projection.rows[1].subtitle.as_deref(),
        Some("15:00"),
        "a runtime rides the row's subtitle when no year exists"
    );
    assert!(projection.rows.iter().all(|row| row.date.is_none()));
    assert!(
        projection.collections.is_none() && !projection.feed_group,
        "the plain mode carries no collection selector"
    );
    assert!(!projection.loading && !projection.failed);
    assert_eq!(
        projection.selected.as_deref(),
        Some("m-1"),
        "the first row is the Hero default"
    );
    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(
        hero.kind, "movie",
        "the kicker's kind word is the item type's lowercase word"
    );
    assert_eq!(hero.title, "A Movie");
    // The shared Emby hero producer's facts: the premiere date, the
    // duration row, and the IMDb row the producer appends from the item's
    // external URLs — the same meta rows the TUI's hero paints.
    assert_eq!(
        mbv_ui_model::meta::meta_rows_text(&hero.meta_rows),
        vec!["1 Jun 1999", "01:30:00", "IMDb"]
    );
    assert_eq!(hero.duration_row, Some(1));
    assert_eq!(hero.progress_row, None);
    // The credits and the IMDb target ride the shared producer's output,
    // director first (design D17, Q7): the TUI paints the same table.
    assert_eq!(
        hero.credits,
        vec![
            GuiEmbyHeroCredit {
                name: "Kirsten Johnson".into(),
                role: "Director".into(),
            },
            GuiEmbyHeroCredit {
                name: "Jane Doe".into(),
                role: "Lead".into(),
            },
        ]
    );
    assert_eq!(
        hero.imdb_url.as_deref(),
        Some("https://www.imdb.com/title/tt0000001"),
        "the IMDb meta row's link target rides the hero"
    );
    assert!(hero.overview.is_none());
    // No image tag declares a source, so the Hero carries none and the
    // window paints its own placeholder.
    assert!(hero.art.is_none());
}

/// The feed-group mode's projection (row 8.1, contract b; 2.6, design D2, D3,
/// D4): the pills are All and each root folder, with the checked set's pill
/// checked, the checked folders' items are the rows in the stored date order
/// with the year gutter and no rail letter, and the mode's flag is set.
#[test]
fn the_feed_group_mode_projects_pills_and_the_date_ordered_rows() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = FEED_ID.into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    app.config
        .lock()
        .unwrap()
        .feed_view_libraries
        .push("youtube".into());
    app.libs[0].nav_stack.push(level(
        FEED_ID,
        "YouTube",
        vec![
            folder_item("folder-1", "Channel 1"),
            folder_item("folder-2", "Empty"),
        ],
    ));
    let mut first = make_item("Old Clip", "Video");
    first.id = "vid-1".into();
    first.production_year = 2020;
    first.parent_id = "folder-1".into();
    first.date_added = "2020-01-01T10:00:00.0000000Z".into();
    let mut second = make_item("New Clip", "Video");
    second.id = "vid-2".into();
    second.parent_id = "folder-1".into();
    second.date_added = "2026-01-01T10:00:00.0000000Z".into();
    app.libs[0].feed_home_video = Some(FeedHomeVideoState {
        folder_counts: [("folder-1".to_string(), 2_usize)].into_iter().collect(),
        items: [(FeedHomeVideoSelection::All, vec![first, second])]
            .into_iter()
            .collect(),
        ..FeedHomeVideoState::default()
    });

    let projection = crate::app::tests::gui_emby_projection(&mut app, 0);

    assert!(projection.feed_group, "the feed-group mode is named");
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
        ["Channel 1", "Empty"],
        "the dropdown offers each root folder, with no New row"
    );
    let selected: Vec<&str> = collections
        .entries
        .iter()
        .filter(|entry| entry.checked)
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(selected, Vec::<&str>::new(), "All checks no folder's row");
    assert!(collections.all, "the selection is All");
    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["vid-2", "vid-1"],
        "the rows are the checked folders' date order, newest first"
    );
    assert!(
        projection.rows.iter().all(|row| row.sort_letter.is_none()),
        "the date-ordered list shows no rail"
    );
    assert_eq!(
        projection.rows[1].date.as_deref(),
        Some("2020"),
        "the year rides the feed row's gutter"
    );
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("Channel 1"),
        "an All row names the programme (root folder) the video sits in"
    );
    assert_eq!(projection.collection_names, vec!["All"]);
    assert_eq!(
        projection.selected.as_deref(),
        Some("vid-2"),
        "the first row is the Hero default"
    );
    assert!(projection.hero.is_some(), "the Hero paints");
}

/// The TUI's Hero gate (row 8.1, design D15): a row that is a folder paints no
/// Hero, because the Hero describes an item and a folder is a container, and a
/// Movies library admits only a `Movie`, so a `BoxSet` row paints none either.
/// `hero_item` is the TUI's own rule.
#[test]
fn a_folder_or_a_boxset_row_paints_no_hero() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut boxset = make_item("Boxset", "BoxSet");
    boxset.id = "box-1".into();
    let mut app = plain_movies_app(vec![boxset]);
    crate::app::tests::stub_gui_state(|gui| {
        gui.emby_libs.insert(
            0,
            crate::app::gui_library::GuiEmbyLibrarySelection {
                picked_row: Some("box-1".into()),
                ..Default::default()
            },
        );
    });

    let projection = crate::app::tests::gui_emby_projection(&mut app, 0);

    assert_eq!(
        projection.selected.as_deref(),
        Some("box-1"),
        "the row is still the selection"
    );
    assert!(
        projection.hero.is_none(),
        "a BoxSet row names a container, so the Hero paints nothing"
    );
}

/// A row image for the cache, as a thumbnail rendition would decode: a
/// landscape still at the Emby row's size.
fn row_image() -> image::DynamicImage {
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        128,
        72,
        image::Rgba([20, 40, 60, 255]),
    ))
}

/// A plain list row paints the art its item's Hero artwork policy declares
/// (gui-delineated-rows design D4, D5): a row whose item declares an image
/// shows the cached row image with its item's media kind, and a row whose item
/// declares none is the no-source state.
#[test]
fn the_plain_mode_rows_paint_their_row_art() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut tagged = movie_item("m-1", "A Movie", 1999, 0);
    tagged.image_tags.thumb = "t1".into();
    let key = mbv_images::row::emby_row_request(&tagged, false).cache_key;
    let untagged = movie_item("m-2", "B Movie", 0, 0);
    let mut app = plain_movies_app(vec![tagged, untagged]);
    app.seed_card_image_for_test(key.clone(), row_image());

    let projection = crate::app::tests::gui_emby_projection(&mut app, 0);

    assert_eq!(
        projection.rows[0].art,
        GuiRowArt::Image {
            key: key.clone(),
            square: false
        },
        "a tagged row shows its cached row image"
    );
    assert_eq!(projection.rows[0].media, GuiRowMediaKind::Video);
    assert_eq!(
        projection.rows[1].art,
        GuiRowArt::NoSource,
        "an item that declares no image has no source"
    );
}
