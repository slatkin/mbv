//! Tests of the GUI Library's Favourites data path (emby-favourites design D6,
//! D7, D8): the landing rule a failed first load and a failed in-place refresh
//! follow, the collections-first Title-ordered rows, the id resolution that
//! reads the store only while the library's view is Favourites, and the artist
//! path a favourite music artist row's play runs. The Emby boundary is the
//! in-memory mock transport, so no request leaves the process.

use super::*;

use crate::gui_core::gui_library::GuiSectionRequest;
use mbv_client_core::config::TestStateDirGuard;
use mbv_net::mock_http::MockHttp;

const LIB_ID: &str = "lib-tv";

/// An Emby item of one type, with the sort name its Title order reads. A
/// Series, a `BoxSet`, a music artist, and a music album are folders, as the
/// Service returns them.
fn typed_item(id: &str, name: &str, item_type: &str) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.id = id.to_string();
    item.sort_name = name.to_string();
    item.is_folder = matches!(
        item_type,
        "Series" | "BoxSet" | "MusicArtist" | "MusicAlbum"
    );
    item
}

/// A stub `App` whose Shows library is the active destination, whose
/// Favourites store holds `items`, and whose view is Favourites: the shape a
/// press on the view dropdown leaves.
fn favourites_app(items: Vec<EmbyItem>) -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = LIB_ID.into();
    library.collection_type = "tvshows".into();
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.favourites
            .insert(LIB_ID.to_string(), GuiSectionRequest::landed(items));
        gui.library_views.insert(0, GuiLibraryView::Favourites);
    });
    app
}

/// Land one favourites fetch result through the one drain the GUI tick runs,
/// as the worker's send would.
fn land(result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>) -> ClientEventLanding {
    crate::gui_core::test_support::stub_gui_state(|gui| {
        install_client_event(
            gui,
            LibraryFetchEvent::Favourites {
                library_id: LIB_ID.to_string(),
                result,
            },
        )
    })
}

/// Contract (design D6, D9): the list puts the collections first — the items
/// that hold items, a TV Series and a `BoxSet` — then the media items, each
/// group in Title order, so a TV library lists its Series before its Episodes.
#[test]
fn favourite_rows_put_collections_first_then_media_in_title_order() {
    let _guard = TestStateDirGuard::new();
    let mut app = favourites_app(vec![
        typed_item("ep-a", "Alpha", "Episode"),
        typed_item("series-z", "Zulu", "Series"),
        typed_item("movie-b", "Beta", "Movie"),
        typed_item("box-a", "Aaa", "BoxSet"),
    ]);

    let projection = crate::gui_core::test_support::gui_shows_projection(&mut app, 0);

    let favourites = projection.favourites.expect("the view carries the list");
    let ids: Vec<&str> = favourites.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["box-a", "series-z", "ep-a", "movie-b"],
        "the collections come first, each group in Title order"
    );
    assert_eq!(
        favourites.rows[0].kind,
        crate::gui_core::GuiEmbyRowKind::Collection
    );
    assert_eq!(
        favourites.rows[2].kind,
        crate::gui_core::GuiEmbyRowKind::Media
    );
    assert_eq!(
        favourites.selected.as_deref(),
        Some("box-a"),
        "the first row is the Favourites Hero default"
    );
    assert!(
        favourites.hero.is_some(),
        "a BoxSet favourite paints a Hero, which the plain-library gate excludes"
    );
}

/// Contract (design D6): a first load that fails lands an empty list, so the
/// list paints its empty state and no frame starts the same fetch again.
#[test]
fn a_failed_first_favourites_load_lands_an_empty_list() {
    let _guard = TestStateDirGuard::new();
    let mut app = favourites_app(Vec::new());
    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.favourites.remove(LIB_ID);
    });

    let landing = land(Err(mbv_emby::EmbyError::resolve("favourites failed")));

    assert_eq!(landing, ClientEventLanding::Installed);
    let projection = crate::gui_core::test_support::gui_shows_projection(&mut app, 0);
    let favourites = projection.favourites.expect("the view carries the list");
    assert!(!favourites.loading, "the landed fetch is not loading");
    assert!(favourites.rows.is_empty(), "the list paints no row");
}

/// Contract (design D6): an in-place refresh that fails keeps the rows the
/// store already holds.
#[test]
fn a_failed_favourites_refresh_keeps_the_rows_already_held() {
    let _guard = TestStateDirGuard::new();
    let mut app = favourites_app(vec![typed_item("movie-b", "Beta", "Movie")]);
    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.favourites_refreshing.insert(LIB_ID.to_string());
    });

    let landing = land(Err(mbv_emby::EmbyError::resolve("favourites failed")));

    assert_eq!(landing, ClientEventLanding::RefreshFailed);
    let projection = crate::gui_core::test_support::gui_shows_projection(&mut app, 0);
    let favourites = projection.favourites.expect("the view carries the list");
    assert_eq!(
        favourites
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["movie-b"],
        "the refresh's failure keeps the held row"
    );
}

/// Contract (design D6, D8): a favourites id resolves only while that
/// library's active view is Favourites, so a Music tree artist row whose id
/// equals a favourite artist's id still resolves no item in All.
#[test]
fn a_favourite_id_resolves_only_while_the_view_is_favourites() {
    let _guard = TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    let artist = typed_item("artist-1", "Metallica", "MusicArtist");
    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.favourites.insert(
            "lib-music".to_string(),
            GuiSectionRequest::landed(vec![artist.clone()]),
        );
    });

    let in_all = crate::gui_core::test_support::stub_gui_state(|gui| {
        crate::gui_core::gui_library::library_item(&app, gui, "artist-1")
    });
    assert!(in_all.is_none(), "the All view resolves no favourite row");

    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.library_views.insert(0, GuiLibraryView::Favourites);
    });
    let in_favourites = crate::gui_core::test_support::stub_gui_state(|gui| {
        crate::gui_core::gui_library::library_item(&app, gui, "artist-1")
    });
    assert_eq!(
        in_favourites.map(|(lib_idx, item)| (lib_idx, item.id)),
        Some((0, "artist-1".to_string())),
        "the Favourites view resolves the row's item"
    );
}

/// The artist's tracks as the mock transport returns them: two albums, the
/// earlier one's tracks out of order in the reply, so the assertion proves the
/// album, disc, and track order rather than the reply's.
const ARTIST_TRACKS: &str = r#"{"Items":[
{"Id":"tr-beta","Name":"Beta One","Type":"Audio","MediaType":"Audio","AlbumId":"alb-beta","Album":"Beta","ProductionYear":2000,"ParentIndexNumber":1,"IndexNumber":1},
{"Id":"tr-alpha-2","Name":"Alpha Two","Type":"Audio","MediaType":"Audio","AlbumId":"alb-alpha","Album":"Alpha","ProductionYear":1990,"ParentIndexNumber":1,"IndexNumber":2},
{"Id":"tr-alpha-1","Name":"Alpha One","Type":"Audio","MediaType":"Audio","AlbumId":"alb-alpha","Album":"Alpha","ProductionYear":1990,"ParentIndexNumber":1,"IndexNumber":1}
],"TotalRecordCount":3}"#;

/// A stub `App` attached to a local daemon Owner, whose Music library's view is
/// Favourites and whose favourites list holds one music artist row: the
/// Library panel has focus, so the row's menu builds and its entries run.
fn favourite_artist_app(http: &MockHttp) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(Vec::new());
    let mut config = app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .cached_emby_client_for_test()
        .expect("the installed setup holds a client")
        .with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);
    app.take_libraries_for_test();
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app.set_panel_focus(PanelFocus::Library);
    crate::gui_core::test_support::stub_gui_state(|gui| {
        gui.favourites.insert(
            "lib-music".to_string(),
            GuiSectionRequest::landed(vec![typed_item("artist-1", "Metallica", "MusicArtist")]),
        );
        gui.library_views.insert(0, GuiLibraryView::Favourites);
    });
    (app, cmd_rx)
}

/// The last queue replacement the Owner received: its item ids. Drains the
/// channel, so call once per assertion point.
fn sent_replacement(cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) -> Option<Vec<String>> {
    let mut sent = None;
    for command in cmd_rx.try_iter() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, .. } = command {
            sent = Some(items.iter().map(|item| item.id().to_string()).collect());
        }
    }
    sent
}

/// Contract (design D8, spec "Favourite artist plays"): a favourite music
/// artist row's Play All fetches the artist's tracks through the `ArtistIds`
/// query and plays them in album, disc, and track order. No path sends
/// `ParentId=<artist id>`, so the folder walk never runs for the row.
#[test]
fn a_favourite_artist_rows_play_all_fetches_the_artists_tracks() {
    let _guard = TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_when("ArtistIds=artist-1", 200, ARTIST_TRACKS);
    let (mut app, cmd_rx) = favourite_artist_app(&http);

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::Play {
            item_id: "artist-1".to_string(),
        },
    );

    assert_eq!(
        sent_replacement(&cmd_rx),
        Some(vec![
            "tr-alpha-1".to_string(),
            "tr-alpha-2".to_string(),
            "tr-beta".to_string()
        ]),
        "the artist's tracks replace the queue in album, disc, and track order"
    );
    let requests = http.requests();
    assert!(
        requests
            .iter()
            .any(|request| request.contains("ArtistIds=artist-1")),
        "the play fetches the artist's tracks by id"
    );
    assert!(
        !requests
            .iter()
            .any(|request| request.contains("ParentId=artist-1")),
        "no path sends the artist's id as a folder parent"
    );
}

/// Contract (design D8): a favourite music artist row's Shuffle runs the
/// `ArtistIds` fetch and replaces the queue with a shuffle-sourced replacement.
#[test]
fn a_favourite_artist_rows_shuffle_fetches_the_artists_tracks() {
    let _guard = TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_when("ArtistIds=artist-1", 200, ARTIST_TRACKS);
    let (mut app, cmd_rx) = favourite_artist_app(&http);

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::Shuffle {
            row: GuiLibraryMenuRow::Item("artist-1".to_string()),
        },
    );

    let mut ids = sent_replacement(&cmd_rx).expect("the shuffle replaced the queue");
    ids.sort();
    assert_eq!(ids, ["tr-alpha-1", "tr-alpha-2", "tr-beta"]);
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.contains("ParentId=artist-1")),
        "the shuffle entry does not send the artist's id as a folder parent"
    );
}
