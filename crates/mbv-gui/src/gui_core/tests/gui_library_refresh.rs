//! Tests of the GUI Library's refresh dispatcher (gui-list-header-buttons
//! design D7, row 6.2): one `Refresh` command, one site per destination kind,
//! and the observable mark each site leaves. A destination whose refresh keeps
//! a loaded value records the key it refetches in its in-flight set, so a
//! second press while that fetch runs starts nothing (design D8). A Shows All
//! refresh marks stale the open rows the last in-view report names, so their
//! children reload in place, and starts no load for a row the report does not
//! name (spec "A tree refresh reloads the children of the open rows in view").
//! The Emby boundary is the in-memory mock transport, so no request leaves the
//! process.

use super::*;

use crate::gui_core::gui_library::GuiSectionRequest;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

const MUSIC_ID: &str = "lib-music";
const MOVIES_ID: &str = "lib-movies";
const TV_ID: &str = "lib-tv";
const HIPHOP: &str = "grp-hiphop";

/// One destination whose refresh leaves a mark of its own (design D7, D8): the
/// four destinations whose refetch records a key in an in-flight set, or marks
/// the Upcoming fetch running.
#[derive(Clone, Copy, Debug)]
enum RefreshCase {
    /// A Music destination with a loaded group: the group key joins
    /// `music_albums_refreshing`.
    MusicGroup,
    /// A plain Emby library with a loaded root list: the library index joins
    /// `emby_roots_refreshing`.
    PlainEmbyRoot,
    /// A Shows destination in New with a landed episode list: the library id
    /// joins `shows_new_refreshing`.
    ShowsNew,
    /// A Shows destination in Upcoming with a stored snapshot: the fetch state
    /// is in flight.
    ShowsUpcoming,
}

impl RefreshCase {
    /// The mark this destination's refresh leaves, for the assertion message.
    const fn label(self) -> &'static str {
        match self {
            Self::MusicGroup => "the refreshed group's albums are in flight",
            Self::PlainEmbyRoot => "the refreshed root list is in flight",
            Self::ShowsNew => "the refreshed New list is in flight",
            Self::ShowsUpcoming => "the Upcoming snapshot fetch is in flight",
        }
    }
}

/// A browse level hanging off `parent_id` over `items`.
fn level(parent_id: &str, items: Vec<EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        parent_id: parent_id.to_string(),
        title: parent_id.to_string(),
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

/// A stub `App` whose Emby runtime holds a client over `http`.
fn app_with_emby_client(http: &MockHttp) -> App {
    let mut app = make_app_stub();
    let mut config = app.config_for_test(|config| config.clone());
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .cached_emby_client_for_test()
        .expect("the installed setup holds a client")
        .with_test_agent(http.agent());
    app.adopt_ready_emby_client_for_test(client);
    app
}

/// One item of `name` and `kind` with the stable id `id`.
fn item(id: &str, name: &str, kind: &str) -> EmbyItem {
    let mut item = make_item(name, kind);
    item.id = id.to_string();
    item
}

/// The Music destination of design D7: one group level over one group, the
/// group selected, and the group's album list landed.
fn music_app(http: &MockHttp) -> App {
    let mut app = app_with_emby_client(http);
    app.seed_music_levels_for_test(&["group", "album"]);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = MUSIC_ID.into();
    library.collection_type = "music".into();
    let mut group = item(HIPHOP, "Hip-Hop", "MusicArtist");
    group.is_folder = true;
    let mut tab = LibraryTab::new(library);
    tab.nav_stack.push(level(MUSIC_ID, vec![group]));
    tab.nav_stack.push(level(HIPHOP, Vec::new()));
    app.push_library_for_test(tab);
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app.seed_gui_library_groups_for_test(0, HIPHOP);
    app.seed_gui_music_albums_for_test(0, HIPHOP, &[("alb-one", "Album One")]);
    app
}

/// The plain Emby destination of design D7: a movies library whose root list
/// store holds one landed row.
fn plain_emby_app(http: &MockHttp) -> App {
    let mut app = app_with_emby_client(http);
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = MOVIES_ID.into();
    library.collection_type = "movies".into();
    let mut tab = LibraryTab::new(library);
    tab.nav_stack.push(level(MOVIES_ID, Vec::new()));
    app.push_library_for_test(tab);
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app.seed_gui_emby_roots_for_test(0, vec![item("movie-one", "Movie One", "Movie")]);
    app
}

/// A Shows destination whose root level exists and is empty, as a first open
/// finds it.
fn shows_app(http: &MockHttp) -> App {
    let mut app = app_with_emby_client(http);
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = TV_ID.into();
    library.collection_type = "tvshows".into();
    let mut tab = LibraryTab::new(library);
    tab.nav_stack.push(level(TV_ID, Vec::new()));
    app.push_library_for_test(tab);
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app
}

/// The Shows destination of design D7 in New, with a landed episode list.
fn shows_new_app(http: &MockHttp) -> App {
    let app = shows_app(http);
    stub_gui_state(|gui| {
        gui.shows_views.insert(0, GuiShowsView::New);
        gui.shows_new
            .insert(TV_ID.to_string(), GuiSectionRequest::Landed(Vec::new()));
    });
    app
}

/// The Shows destination of design D7 in Upcoming, with a stored snapshot as
/// the last fetch left it.
fn shows_upcoming_app(http: &MockHttp) -> App {
    let mut app = shows_app(http);
    stub_gui_state(|gui| {
        gui.shows_views.insert(0, GuiShowsView::Upcoming);
    });
    app.send_lib_event_for_test(LibEvent::ModelContent(
        ModelContentEvent::TvUpcomingSnapshotFetched {
            library_id: TV_ID.to_string(),
            items: Vec::new(),
        },
    ));
    while app.drain_lib_event() {}
    app
}

/// The destination `case` names, with the store its refresh refetches loaded.
fn refreshed_destination_app(case: RefreshCase, http: &MockHttp) -> App {
    match case {
        RefreshCase::MusicGroup => music_app(http),
        RefreshCase::PlainEmbyRoot => plain_emby_app(http),
        RefreshCase::ShowsNew => shows_new_app(http),
        RefreshCase::ShowsUpcoming => shows_upcoming_app(http),
    }
}

/// Whether the mark `case`'s refresh leaves is set.
fn refresh_running(app: &App, case: RefreshCase) -> bool {
    match case {
        RefreshCase::MusicGroup => stub_gui_state(|gui| {
            gui.music_albums_refreshing
                .contains(&(0, HIPHOP.to_string()))
        }),
        RefreshCase::PlainEmbyRoot => stub_gui_state(|gui| gui.emby_roots_refreshing.contains(&0)),
        RefreshCase::ShowsNew => stub_gui_state(|gui| gui.shows_new_refreshing.contains(TV_ID)),
        RefreshCase::ShowsUpcoming => app.tv_upcoming_fetch_in_flight_for_test(TV_ID),
    }
}

/// One `Refresh` command leaves each destination's own in-flight mark (design
/// D7, D8): the refresh keeps the loaded content it paints and records the key
/// it refetches, so a second press while that fetch runs starts nothing.
#[rstest::rstest]
#[case::music_group(RefreshCase::MusicGroup)]
#[case::plain_emby_root(RefreshCase::PlainEmbyRoot)]
#[case::shows_new(RefreshCase::ShowsNew)]
#[case::shows_upcoming(RefreshCase::ShowsUpcoming)]
fn a_refresh_leaves_the_destinations_own_in_flight_mark(#[case] case: RefreshCase) {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = refreshed_destination_app(case, &http);

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::Refresh {
            album_ids: Vec::new(),
        },
    );

    assert!(refresh_running(&app, case), "{}", case.label());
}

/// A Shows All refresh marks stale the series the last in-view report names and
/// leaves a cached series the report does not name alone (design D7, D9, spec
/// "A tree refresh reloads the children of the open rows in view"): the open
/// rows reload their children in place, and a row that is not in view starts no
/// load.
#[test]
fn a_shows_all_refresh_marks_the_reported_series_stale_and_no_other() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = shows_app(&http);
    app.seed_series_detail_for_test(
        "series-open",
        SeriesDetail {
            seasons: vec![item("season-one", "Season 1", "Season")],
            episodes: std::collections::HashMap::new(),
        },
    );
    app.seed_series_detail_for_test(
        "series-elsewhere",
        SeriesDetail {
            seasons: Vec::new(),
            episodes: std::collections::HashMap::new(),
        },
    );

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsOpenInView {
            lib_idx: 0,
            series: vec!["series-open".to_string()],
            seasons: vec![("series-open".to_string(), "season-one".to_string())],
        },
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::Refresh {
            album_ids: Vec::new(),
        },
    );

    assert!(
        app.series_detail_stale("series-open"),
        "the open series in the report reloads its detail"
    );
    assert!(
        app.series_season_stale("series-open", "season-one"),
        "the open season in the report reloads its episodes"
    );
    assert!(
        !app.series_detail_stale("series-elsewhere"),
        "a cached series the report does not name starts no load"
    );
}
