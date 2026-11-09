//! Tests of the GUI Library's Audiobooks data path and projection
//! (add-gui-library-panel design D12, row 9.1): an open of an unloaded book
//! catalog starts the TUI's own book catalog load and the fetched page lands
//! on the App through the GUI tick's own drain, a select stores the picked
//! book and lands its chapters in the detail cache through the TUI's loader,
//! the frame path's ensure loads the default pick's detail, the presented
//! destination set names the book libraries as Audiobooks and the podcast
//! libraries as Podcasts (row 10.1 presents the Podcasts destination), and
//! the projection and the play, chapter, and queue commands follow the TUI's
//! row, hero, and activation rules. The Audiobookshelf boundary is the lib
//! event channel, so no request leaves the process and no worker outlives
//! the assertion.

use super::*;
use mbv_audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookPage, AudiobookshelfChapter,
};
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;

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

/// A book with its author facts, as the projection's helper seeds them:
/// `(id, title, author display, author sort key, duration seconds, cover)`.
fn authored_book<'a>(
    id: &'a str,
    title: &'a str,
    author_display: Option<&'a str>,
    author_sort_key: &'a str,
    duration: f64,
) -> (
    &'a str,
    &'a str,
    Option<&'a str>,
    &'a str,
    f64,
    Option<&'a str>,
) {
    (id, title, author_display, author_sort_key, duration, None)
}

/// A stub `App` whose book library 0 holds `books` already in the TUI's
/// author-sort order, with `chapters` and `files` cached for the first book
/// and `progress` seeded when `progress_seconds` is some.
fn projection_app(
    books: &[crate::app::app_stub::test_reads::SeededAbsBook<'_>],
    chapters: &[(usize, f64, f64, &str)],
    files: &[(usize, f64)],
    progress_seconds: Option<f64>,
) -> App {
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(0, books);
    if !chapters.is_empty() || !files.is_empty() {
        app.seed_gui_abs_book_detail_for_test(0, books[0].0, chapters, files);
    }
    if let Some(seconds) = progress_seconds {
        app.seed_gui_abs_book_progress_for_test(0, books[0].0, seconds, false);
    }
    app
}

/// The projection carries the book rows in the TUI's author-sort order with
/// their rail letters and the default pick (row 9.1, contract a): the rows
/// are the catalog's in its stored order, the letter is the author sort
/// key's first letter (`#` for a digit-led surname), the author credit is
/// the row's subtitle, the duration gutter rides, and the first book is the
/// Hero default.
#[test]
fn the_projection_carries_the_rows_letters_and_the_default_pick() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app(
        &[
            authored_book(
                "book-a",
                "Austen's End",
                Some("Jane Austen"),
                "austen",
                7200.0,
            ),
            authored_book(
                "book-b",
                "Bovary",
                Some("Gustave Flaubert"),
                "flaubert",
                5400.0,
            ),
            authored_book("book-c", "99 Stories", Some("9 Tales"), "9th street", 600.0),
        ],
        &[],
        &[],
        None,
    );

    let projection = app.gui_audiobooks_projection(0);

    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["book-a", "book-b", "book-c"],
        "the rows are the catalog"
    );
    let letters: Vec<char> = projection.rows.iter().map(|row| row.sort_letter).collect();
    assert_eq!(
        letters,
        ['A', 'F', '#'],
        "the rail letters read the author sort keys"
    );
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("Jane Austen"),
        "the author credit rides the row's subtitle"
    );
    assert_eq!(
        projection.rows[0].duration, "2:00",
        "the duration gutter rides"
    );
    assert!(!projection.loading && !projection.failed);
    assert_eq!(
        projection.selected.as_deref(),
        Some("book-a"),
        "the first book is the Hero default"
    );
}

/// The book Hero projects the TUI's facts and the chapter Workspace (row
/// 9.1, contract a): the shared producer's meta rows — author, duration,
/// and the `N%` progress row — the portrait art shape, and the chapter rows
/// in the TUI's order with their durations.
#[test]
fn the_book_hero_projects_the_tui_facts_and_the_chapter_workspace() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app(
        &[authored_book(
            "book-a",
            "Austen's End",
            Some("Jane Austen"),
            "austen",
            7200.0,
        )],
        &[
            (1, 0.0, 1800.0, "Chapter One"),
            (2, 1800.0, 3600.0, "Chapter Two"),
        ],
        &[],
        Some(1800.0),
    );

    let projection = app.gui_audiobooks_projection(0);

    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(hero.title, "Austen's End");
    assert_eq!(
        hero.meta_rows,
        vec!["Jane Austen", "01:00:00", "50%"],
        "the TUI hero producer's facts ride"
    );
    assert_eq!(hero.duration_row, Some(1));
    assert_eq!(hero.progress_row, Some(2));
    assert_eq!(
        hero.art_shape,
        GuiArtShape::Portrait,
        "the cover is portrait"
    );
    assert!(hero.art.is_none(), "no cover path declares no art source");
    assert!(
        !projection.workspace_loading,
        "the seeded detail has landed"
    );
    let workspace = hero
        .workspace
        .expect("the seeded detail holds the Workspace");
    let titles: Vec<&str> = workspace
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(titles, ["Chapter One", "Chapter Two"]);
    assert_eq!(
        workspace.rows[0].chapter, 1,
        "the discriminator is the chapter id"
    );
    assert_eq!(workspace.rows[0].duration, "30:00");
    assert!(workspace.rows.iter().all(|row| !row.current));
}

/// The Workspace falls back to the `Part N` rows over the audio files when
/// the book holds no chapters (row 9.1, contract a), and no `Part N` row
/// ever carries the current mark.
#[test]
fn the_workspace_falls_back_to_part_rows_without_chapters() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app(
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
        &[],
        &[(0, 3600.0), (1, 3600.0)],
        None,
    );

    let projection = app.gui_audiobooks_projection(0);

    let workspace = projection
        .hero
        .and_then(|hero| hero.workspace)
        .expect("the seeded detail holds the Workspace");
    let titles: Vec<&str> = workspace
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["Part 0", "Part 1"],
        "the audio files are the fallback rows"
    );
    assert_eq!(
        workspace.rows[0].chapter, 0,
        "the discriminator is the file index"
    );
    assert_eq!(workspace.rows[0].duration, "1:00");
    assert!(workspace.rows.iter().all(|row| !row.current));
}

/// The current chapter is marked while that book plays (row 9.1): the
/// Owner's active item is the book and the playback position sits in its
/// second chapter, derived exactly as the TUI's `WorkspaceNowPlaying::Chapter`
/// projection derives it, and no other row carries the mark.
#[test]
fn the_playing_chapter_is_marked_in_the_workspace() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = projection_app(
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
        &[
            (1, 0.0, 1800.0, "Chapter One"),
            (2, 1800.0, 3600.0, "Chapter Two"),
        ],
        &[],
        None,
    );
    app.playback_queue_mut().adopt_queue_items_with_active(
        vec![mbv_queue::QueueItem::Audiobookshelf(
            mbv_queue::AudiobookshelfItem::Book(mbv_queue::AudiobookshelfBookQueueItem {
                library_item_id: "book-a".into(),
                title: "Austen's End".into(),
                author: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                is_finished: false,
                cover_path: None,
            }),
        )],
        0,
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.position_ticks = mbv_emby_model::seconds_to_ticks(2000.0);
    });

    let projection = app.gui_audiobooks_projection(0);

    let workspace = projection
        .hero
        .and_then(|hero| hero.workspace)
        .expect("the seeded detail holds the Workspace");
    let currents: Vec<bool> = workspace.rows.iter().map(|row| row.current).collect();
    assert_eq!(
        currents,
        [false, true],
        "the chapter the position sits in is the marked one"
    );
}

/// The play command replaces the queue with the book at its saved position
/// (row 9.1, contract b): the TUI's book play path resolves the book's
/// queue item with the cached progress as the resume position, and the
/// Owner's admission is honoured.
#[test]
fn the_play_command_replaces_the_queue_with_the_book_at_its_saved_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(
        0,
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
    );
    app.seed_gui_abs_book_progress_for_test(0, "book-a", 300.0, false);
    let cmd_rx = super::live_owner_channel(&mut app);

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsPlay {
        lib_idx: 0,
        item_id: "book-a".into(),
    });

    let replaced = cmd_rx.try_iter().find_map(|command| match command {
        mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, .. } => match &*items {
            [mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(book))] => {
                Some((book.library_item_id.clone(), book.position_ticks))
            }
            _ => None,
        },
        _ => None,
    });
    let (id, position_ticks) = replaced.expect("the Owner received a queue replacement");
    assert_eq!(id, "book-a");
    assert_eq!(
        position_ticks, 3_000_000_000,
        "the saved position is the resume position"
    );
}

/// The queue command appends the book at the tail (row 9.1, contract b).
#[test]
fn the_queue_command_appends_the_book() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(
        0,
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
    );
    let cmd_rx = super::live_owner_channel(&mut app);

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsQueue {
        lib_idx: 0,
        item_id: "book-a".into(),
    });

    let appended: Vec<String> = cmd_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                [
                    mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(book)),
                ] => Some(book.library_item_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(appended, vec!["book-a".to_string()]);
}

/// The chapter command runs the TUI's `ActivateChapter` rule (row 9.1,
/// contract b): on the active book it is one absolute seek to the chapter's
/// start on the merged timeline, and no queue replacement is submitted.
#[test]
fn the_chapter_command_seeks_on_the_active_book() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(
        0,
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
    );
    app.seed_gui_abs_book_detail_for_test(
        0,
        "book-a",
        &[
            (1, 0.0, 100.0, "Chapter One"),
            (2, 200.0, 300.0, "Chapter Two"),
        ],
        &[],
    );
    let cmd_rx = super::live_owner_channel(&mut app);
    app.playback_queue_mut().adopt_queue_items_with_active(
        vec![mbv_queue::QueueItem::Audiobookshelf(
            mbv_queue::AudiobookshelfItem::Book(mbv_queue::AudiobookshelfBookQueueItem {
                library_item_id: "book-a".into(),
                title: "Austen's End".into(),
                author: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                is_finished: false,
                cover_path: None,
            }),
        )],
        0,
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
    });
    while cmd_rx.try_recv().is_ok() {}

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsPlayChapter {
        lib_idx: 0,
        book_id: "book-a".into(),
        chapter: 2,
    });

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::SeekAbsolute(200.0))
        ),
        "the active book's chapter activation is one absolute seek"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "no queue replacement is submitted"
    );
}

/// A refusal of the Owner's Audiobookshelf admission flashes the TUI's
/// owner-unavailable toast and submits nothing (row 9.1): both the play and
/// the queue command gate on the admission, so a denied owner gets no queue
/// change and the same toast the TUI paints.
#[test]
fn a_denied_owner_flashes_the_owner_unavailable_toast_and_submits_nothing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(
        0,
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
    );
    let cmd_rx = super::live_owner_channel(&mut app);
    app.set_abs_admission_for_test(false);

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsPlay {
        lib_idx: 0,
        item_id: "book-a".into(),
    });
    assert_eq!(
        app.status, "Audiobookshelf playback owner is unavailable",
        "the play's refusal flashes the owner-unavailable toast"
    );
    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsQueue {
        lib_idx: 0,
        item_id: "book-a".into(),
    });
    assert!(
        cmd_rx.try_recv().is_err(),
        "a denied owner receives no queue change"
    );
}

/// Stale ids change nothing (row 9.1): a play or a queue command whose book
/// the published catalog does not carry submits nothing and flashes no
/// toast, and a chapter command whose discriminator the detail no longer
/// holds resolves through the TUI's activation rule's own refusal arm.
#[test]
fn stale_book_and_chapter_ids_change_nothing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(
        0,
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
    );
    app.seed_gui_abs_book_detail_for_test(0, "book-a", &[(1, 0.0, 100.0, "One")], &[]);
    let cmd_rx = super::live_owner_channel(&mut app);

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsPlay {
        lib_idx: 0,
        item_id: "gone".into(),
    });
    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsQueue {
        lib_idx: 0,
        item_id: "gone".into(),
    });
    assert!(cmd_rx.try_recv().is_err(), "a stale id submits nothing");
    assert!(app.status.is_empty(), "a stale id flashes no toast");

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsPlayChapter {
        lib_idx: 0,
        book_id: "book-a".into(),
        chapter: 9,
    });
    assert_eq!(
        app.status, "That chapter is no longer available",
        "the activation rule's refusal arm resolves the stale chapter"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "a stale chapter submits nothing"
    );
}

/// The select command stores the pick the Hero shows (row 9.1): the command
/// arm routes through the App's own select entry, whose stale arm the data
/// path's test owns.
#[test]
fn the_select_command_stores_the_pick() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = abs_app();
    app.seed_gui_abs_books_for_test(
        0,
        &[authored_book(
            "book-a",
            "Austen's End",
            None,
            "austen",
            7200.0,
        )],
    );

    app.apply_gui_library_command(crate::app::gui_library::GuiLibraryCommand::AbsSelect {
        lib_idx: 0,
        row: "book-a".into(),
    });

    assert_eq!(
        app.gui_abs_picked_book_for_test(0),
        Some("book-a"),
        "the select command stored the pick"
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
