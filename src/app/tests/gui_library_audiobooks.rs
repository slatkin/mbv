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
        updated_at_ms: 0,
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

/// The frame path's load pass loads the default pick's detail (row 9.1; the
/// one driver pass of gui-one-destination-ensure design D3): with no select,
/// the catalog's first book is the pick and its detail fetch starts once; a
/// second pass of a cached pick starts nothing.
#[test]
fn the_frame_paths_ensure_loads_the_default_picks_detail() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.audiobookshelf_book_browse[0].books = vec![book("book-a", "A Book")];

    app.ensure_gui_destination(GuiDestinationKind::Audiobooks { lib_idx: 0 });
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

    app.ensure_gui_destination(GuiDestinationKind::Audiobooks { lib_idx: 0 });
    assert_eq!(
        book_state(&app).next_detail_request,
        request,
        "a cached pick's pass started no second fetch"
    );
}

/// Add a second book library at index 2, so the frame path's entry can be
/// asked for a book library the tab is not on.
fn add_second_book_library(app: &mut App) {
    let library = mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "lib-abs-books-b".into(),
        name: "Audiobooks B".into(),
        media_type: "book".into(),
    };
    app.audiobookshelf_libraries.push(library.clone());
    app.audiobookshelf_book_browse
        .push(AudiobookshelfBookBrowseState::new(library));
}

/// The open entry stays inert for a book library that is not the active
/// destination (review of gui-one-destination-ensure units 2.4 and 2.5): the
/// driver tests the tab before it runs the Audiobooks steps, so a `lib_idx`
/// naming another book library starts no load.
#[test]
fn the_open_entry_on_another_book_library_starts_no_load() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    add_second_book_library(&mut app);

    app.gui_abs_open(2);

    let stale = &app.audiobookshelf_book_browse[2];
    assert_eq!(
        stale.catalog_request, 0,
        "the stale library started no catalog chain"
    );
    assert_eq!(
        stale.next_detail_request, 0,
        "no detail fetch started for the stale library"
    );
    assert_eq!(
        book_state(&app).catalog_request,
        0,
        "the active library was not loaded through the stale index"
    );
}

/// A restored Audiobooks destination starts its catalog load from one frame's
/// load pass, with no open command (gui-one-destination-ensure design D3;
/// spec `gui-client`, "A restored Audiobooks or Podcasts library loads its
/// catalog"): the root step arms the catalog's replacement batch, and the
/// pass ends there, so the pick's detail waits for the catalog to land.
#[test]
fn a_restored_audiobooks_destination_starts_its_catalog_load() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();

    app.ensure_gui_destination(GuiDestinationKind::Audiobooks { lib_idx: 0 });

    let state = book_state(&app);
    assert_eq!(
        state.catalog_request, 1,
        "the frame began one catalog chain"
    );
    assert!(
        state.replacement.is_some(),
        "the catalog's replacement batch is collecting"
    );
    assert_eq!(
        state.next_detail_request, 0,
        "the pass ended at the root load, so the pick's detail waits"
    );
}

/// One frame's load pass for library 0's book catalog, as the catalog request
/// serial it left: an armed replacement batch bumps the serial.
fn pass_catalog_request(app: &mut App) -> u64 {
    app.ensure_gui_destination(GuiDestinationKind::Audiobooks { lib_idx: 0 });
    app.audiobookshelf_book_browse[0].catalog_request
}

/// A repaint beside an in-flight catalog starts no second chain
/// (gui-one-destination-ensure design D3, Risks; spec `gui-client`, "A repaint
/// starts no second load"): the root step keeps the open step's unloaded
/// guard, which counts the in-flight page marks and the collecting replacement
/// batch, so the frame's pass while the startup chain walks — and while a
/// refresh collects — arms nothing beside them.
#[test]
fn a_repaint_beside_an_in_flight_catalog_starts_no_second_chain() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.audiobookshelf_book_browse[0].loading_pages.insert(0);

    assert_eq!(
        pass_catalog_request(&mut app),
        0,
        "the startup chain's in-flight page started no second chain"
    );

    app.audiobookshelf_book_browse[0].loading_pages.clear();
    let armed = app.audiobookshelf_book_browse[0].begin_catalog_replacement();
    assert_eq!(
        pass_catalog_request(&mut app),
        armed,
        "the collecting replacement batch started no second chain"
    );
}

/// A failed catalog load is not retried by a repaint, and an open retries it
/// (gui-one-destination-ensure design Risks; spec `gui-client`, "A failed load
/// is not retried by a repaint"): the loader leaves the failure in the browse
/// state's `error` beside the still-unloaded list, the frame's root step
/// honours that mark, and the open clears it.
#[test]
fn a_failed_catalog_load_is_not_retried_by_a_repaint_but_an_open_retries_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.audiobookshelf_book_browse[0].error = Some("the catalog failed to load".into());

    assert_eq!(
        pass_catalog_request(&mut app),
        0,
        "the repaint honoured the failed mark"
    );

    app.gui_abs_open(0);
    assert_eq!(
        book_state(&app).catalog_request,
        1,
        "the open cleared the failed mark and retried the catalog"
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
