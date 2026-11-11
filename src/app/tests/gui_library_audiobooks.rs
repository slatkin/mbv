//! Tests of the GUI Library's Audiobooks data path (add-gui-library-panel
//! design D12, row 9.1): an open of an unloaded book catalog starts the TUI's
//! own book catalog load and the fetched page lands on the App through the
//! GUI tick's own drain, a select stores the picked book and lands its
//! chapters in the detail cache through the TUI's loader, the frame path's
//! ensure loads the default pick's detail, and the presented destination
//! set names the book libraries as Audiobooks and the podcast libraries as
//! Podcasts (row 10.1 presents the Podcasts destination). The projection's
//! and the commands' tests live in the child module. The Audiobookshelf
//! boundary is the lib event channel, so no request leaves the process and
//! no worker outlives the assertion.

use super::*;
use mbv_audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookPage, AudiobookshelfChapter,
};
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;

mod projection;

/// The generation the event arms accept while the test's runtime holds none.
fn generation() -> mbv_core::service_runtime::SetupGeneration {
    mbv_core::service_runtime::SetupGeneration::default()
}

/// A book of the catalog the mock page carries.
fn book(id: &str, title: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: title.into(),
        author_display: None,
        author_sort_key: "Author".into(),
        cover_path: None,
        duration_seconds: 1.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

/// A one-book catalog page, whose completion commits in one event.
fn one_book_page(id: &str, title: &str) -> AudiobookshelfBookPage {
    AudiobookshelfBookPage {
        page: 0,
        limit: 20,
        total: 1,
        items: vec![book(id, title)],
    }
}

/// A stub `App` whose Audiobookshelf catalog holds one book library and one
/// podcast library, with the book destination active and a resolvable
/// Audiobookshelf setup over a closed loopback port: the state a settled
/// Audiobookshelf startup leaves before its book pages land, with the
/// loaders' workers sending their failure class instead of hanging. The
/// caller holds the test state dir, so the secret file and the worker's
/// connection attempt stay inside the scratch path.
fn abs_app() -> App {
    let mut app = make_app_stub();
    app.seed_gui_abs_libraries_for_test();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(mbv_config::AudiobookshelfSetup::new("http://127.0.0.1:1"));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "test-token")
        .expect("the secret is written under the test state dir");
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app
}

/// The book browse state the seeded catalog holds for library 0.
fn book_state(app: &App) -> &AudiobookshelfBookBrowseState {
    &app.audiobookshelf_book_browse[0]
}

/// Opening an unloaded book catalog starts the TUI's own catalog load and the
/// fetched page lands on the App through the GUI tick's drain (row 9.1,
/// contract a): the open's load sends its request, the page event's handler
/// fills the browse state the projection reads, and an open of the loaded
/// catalog starts no second chain.
#[test]
fn opening_an_unloaded_book_catalog_lands_its_page_on_the_app() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();

    app.gui_abs_open(0);
    // The open's load ran: its worker reported one completion. The no-setup
    // stub's worker reports the failure class; the event is dropped raw, so
    // the failure mark never applies and the page below carries the catalog.
    let _ = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the open's load sent its request");
    let request = book_state(&app).catalog_request;
    let _ = app.channels.lib_tx.send(LibEvent::Audiobookshelf(
        AudiobookshelfEvent::BooksFetched {
            generation: generation(),
            request,
            library_id: "lib-abs-books".into(),
            result: Ok(one_book_page("book-a", "A Book")),
        },
    ));
    while app.drain_lib_event() {}

    let state = book_state(&app);
    assert_eq!(
        state
            .books
            .iter()
            .map(|book| book.library_item_id.as_str())
            .collect::<Vec<_>>(),
        ["book-a"],
        "the fetched page landed on the App's browse state"
    );
    assert!(state.replacement.is_none(), "the completed batch published");
    assert_eq!(state.total, 1, "the page's total is the catalog's");

    // A loaded catalog's open starts no second load: no replacement batch
    // arms, so no worker runs and the chain stays single-flight.
    app.gui_abs_open(0);
    assert!(
        book_state(&app).replacement.is_none(),
        "the loaded catalog's open started no refresh batch"
    );
    assert!(
        app.channels.lib_rx.try_recv().is_err(),
        "no request left the loaded catalog's open"
    );
}

/// Selecting a book stores the pick and lands its chapters in the detail
/// cache through the TUI's loader (row 9.1, contract b): the select names the
/// browse state's request serial, the detail event's handler fills the cache,
/// and a stale id changes nothing.
#[test]
fn selecting_a_book_stores_the_pick_and_lands_its_chapters() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.audiobookshelf_book_browse[0].books = vec![book("book-a", "A Book")];

    app.gui_abs_select(0, "book-a");
    assert_eq!(
        app.gui_abs_picked_book_for_test(0),
        Some("book-a"),
        "the picked book is the select command's"
    );
    // The select started the TUI's detail loader: its worker reported one
    // completion, dropped raw so the failure mark never applies.
    let _ = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the select's detail load sent its request");
    let request = book_state(&app).next_detail_request;
    let _ = app.channels.lib_tx.send(LibEvent::Audiobookshelf(
        AudiobookshelfEvent::BookDetailLoaded {
            generation: generation(),
            request,
            library_item_id: "book-a".into(),
            result: Ok((
                vec![AudiobookshelfChapter {
                    id: 3,
                    start: 0.0,
                    end: 12.0,
                    title: "Chapter One".into(),
                }],
                vec![AudiobookshelfAudioFile {
                    index: 0,
                    ino: "file".into(),
                    duration: 12.0,
                }],
            )),
        },
    ));
    while app.drain_lib_event() {}

    assert!(
        book_state(&app).detail_cache.contains_key("book-a"),
        "the chapters landed in the detail cache"
    );

    app.gui_abs_select(0, "gone");
    assert_eq!(
        app.gui_abs_picked_book_for_test(0),
        Some("book-a"),
        "a stale id changes nothing"
    );
}

/// The frame path's ensure loads the default pick's detail (row 9.1): with no
/// select, the catalog's first book is the pick and its detail fetch starts
/// once; an ensure of a cached pick starts nothing.
#[test]
fn the_frame_paths_ensure_loads_the_default_picks_detail() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.audiobookshelf_book_browse[0].books = vec![book("book-a", "A Book")];

    app.ensure_gui_abs_selection(0);
    let _ = app
        .channels
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the ensure's detail load sent its request");
    let request = book_state(&app).next_detail_request;
    let _ = app.channels.lib_tx.send(LibEvent::Audiobookshelf(
        AudiobookshelfEvent::BookDetailLoaded {
            generation: generation(),
            request,
            library_item_id: "book-a".into(),
            result: Ok((Vec::new(), Vec::new())),
        },
    ));
    while app.drain_lib_event() {}
    assert!(book_state(&app).detail_cache.contains_key("book-a"));

    app.ensure_gui_abs_selection(0);
    assert_eq!(
        book_state(&app).next_detail_request,
        request,
        "a cached pick's ensure started no second fetch"
    );
}

/// Opening a catalog whose initial pages are still in flight starts no
/// second chain (row 9.1, review of unit 9.1): the unloaded condition counts
/// the in-flight page marks, so an open during the startup chain's first
/// page arms no replacement batch beside it. The armed batch is the
/// observable, because arming it is what starts the second chain's workers;
/// the request channel is no witness, since a worker's failure event lands
/// after the assertion would run.
#[test]
fn opening_a_catalog_with_a_page_in_flight_starts_no_second_chain() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.audiobookshelf_book_browse[0].loading_pages.insert(0);

    app.gui_abs_open(0);

    let state = book_state(&app);
    assert!(
        state.replacement.is_none(),
        "the open armed no replacement batch beside the in-flight page"
    );
    assert!(
        state.loading_pages.contains(&0),
        "the in-flight page mark is untouched"
    );
}

/// The presented destination set names the book libraries as Audiobooks and
/// the podcast libraries as Podcasts (row 9.1, contract c; row 10.1 presents
/// the Podcasts destination): the tabs carry the Audiobooks destination with
/// its library's label, the podcast library presents its own Podcasts tab at
/// the next position, and each active destination's kind is its own.
#[test]
fn the_presented_tabs_include_a_book_library_and_a_podcast_library() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();

    let tabs = app.gui_library_tabs();
    let labels: Vec<&str> = tabs.tabs.iter().map(|tab| tab.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Audiobooks", "Podcasts"],
        "the book library presents Audiobooks and the podcast library Podcasts"
    );
    assert_eq!(
        app.gui_active_destination()
            .and_then(crate::app::gui_library::GuiDestinationKind::library_index),
        Some(0),
        "the active book destination is the seeded one"
    );
    assert_eq!(
        app.gui_active_destination(),
        Some(crate::app::gui_library::GuiDestinationKind::Audiobooks { lib_idx: 0 }),
        "the active book destination's kind is the Audiobooks one"
    );

    app.tab = TabSelection::AudiobookshelfLibrary(1);
    assert_eq!(
        app.gui_active_destination(),
        Some(crate::app::gui_library::GuiDestinationKind::Podcasts { lib_idx: 1 }),
        "the podcast destination presents its own kind"
    );
}
