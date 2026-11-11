//! The Audiobooks destination's contracts (add-gui-library-panel design D12,
//! row 9.1): the presented tabs name both Audiobookshelf destinations — the
//! book libraries as Audiobooks and the podcast libraries as Podcasts (row
//! 10.1) — and the open-destination command activates the book library through
//! the same presented-position gate every destination shares.

use mbv::ClientTickHost;
use mbv_ctrl::CtrlCmd;
use mbv_gui::bridge::frame::library::GuiLibraryFrame;
use mbv_queue::AudiobookshelfItem;
use mbv_ui_model::tab_selection::TabSelection;

use super::{library_of, send_library};

/// A core thread over an App whose catalog holds one Audiobookshelf book
/// library and one podcast library, with no Emby library: the Audiobooks and
/// Podcasts destinations are the presented ones (rows 9.1 and 10.1).
fn audiobooks_core() -> crate::TestCore {
    let mut test = crate::test_core(0);
    test.core.app_mut().seed_gui_abs_libraries_for_test();
    test
}

/// The Audiobooks core with one book's catalog and the other's chapter
/// detail seeded, as landed fetches leave them, over the recording stub
/// Owner the queue commands' tests read.
fn seeded_audiobooks_core() -> (crate::TestCore, std::sync::mpsc::Receiver<CtrlCmd>) {
    let (app, cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = crate::test_core_with(app);
    test.core.app_mut().seed_gui_abs_libraries_for_test();
    test.core.app_mut().seed_gui_abs_books_for_test(
        0,
        &[
            (
                "book-a",
                "Austen's End",
                Some("Jane Austen"),
                "austen",
                7200.0,
                None,
            ),
            (
                "book-b",
                "Bovary",
                Some("Gustave Flaubert"),
                "flaubert",
                5400.0,
                None,
            ),
        ],
    );
    test.core.app_mut().seed_gui_abs_book_detail_for_test(
        0,
        "book-b",
        &[
            (1, 0.0, 1800.0, "Chapter One"),
            (2, 1800.0, 3600.0, "Chapter Two"),
        ],
        &[],
    );
    test.core
        .app_mut()
        .seed_gui_abs_book_detail_for_test(0, "book-a", &[], &[(0, 7200.0)]);
    (test, cmd_rx)
}

/// The Audiobooks section a frame carries (row 9.1).
fn audiobooks_of(frame: GuiLibraryFrame) -> mbv_gui::bridge::frame::library::GuiAudiobooksFrame {
    frame
        .content
        .and_then(|content| content.audiobooks)
        .expect("the frame carries the Audiobooks section")
}

/// The Audiobooks book id a queue command names, from the recording stub
/// Owner's channel: the replacement's or the append's single book item.
fn commanded_book_id(cmd_rx: &std::sync::mpsc::Receiver<CtrlCmd>) -> String {
    cmd_rx
        .try_iter()
        .find_map(|command| match command {
            CtrlCmd::UnifiedQueueReplace { items, .. }
            | CtrlCmd::UnifiedQueueAppend { items, .. } => match &*items {
                [mbv_queue::QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book))] => {
                    Some(book.library_item_id.clone())
                }
                _ => None,
            },
            _ => None,
        })
        .expect("the Owner received the book's queue command")
}

/// The presented tabs carry both Audiobookshelf destinations (row 9.1,
/// contract c; row 10.1 presents Podcasts): the Audiobooks destination joins
/// the presented set with its library's label and the podcast library joins
/// with its own, so the top bar paints both and the launch restore's fallback
/// reads the same set.
#[test]
fn the_presented_tabs_carry_the_book_library_and_the_podcast_library() {
    let mut test = audiobooks_core();

    let tabs = test.core.app_mut().gui_library_tabs();
    let labels: Vec<&str> = tabs.tabs.iter().map(|tab| tab.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Audiobooks", "Podcasts"],
        "each Audiobookshelf library presents its own destination"
    );
}

/// The open-destination command activates the book library (row 9.1): the
/// Audiobooks tab's position opens it, and the active destination's kind is
/// the Audiobooks one the frame path and the open step read.
#[test]
fn the_open_destination_command_opens_the_audiobooks_tab() {
    let mut test = audiobooks_core();

    let position = test
        .core
        .app_mut()
        .gui_library_tabs()
        .tabs
        .iter()
        .find(|tab| tab.label == "Audiobooks")
        .map(|tab| tab.position)
        .expect("the presented tabs carry the Audiobooks tab");
    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::OpenDestination { position },
    );

    let app = test.core.app_mut();
    assert_eq!(
        app.tab_for_test(),
        TabSelection::AudiobookshelfLibrary(0),
        "the Audiobooks destination is active"
    );
    assert_eq!(
        app.gui_active_destination()
            .and_then(mbv::GuiDestinationKind::library_index),
        Some(0),
        "the active destination's kind is the Audiobooks one"
    );
}

/// The frame carries the book list after the open (row 9.1): the opened
/// destination's section names the library index, carries the books in the
/// TUI's order with their rail letters, and the first book is the Hero
/// default with the TUI hero producer's facts.
#[test]
fn the_frame_carries_the_book_list_after_the_open() {
    let (mut test, _cmd_rx) = seeded_audiobooks_core();

    let position = test
        .core
        .app_mut()
        .gui_library_tabs()
        .tabs
        .iter()
        .find(|tab| tab.label == "Audiobooks")
        .map(|tab| tab.position)
        .expect("the presented tabs carry the Audiobooks tab");
    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::OpenDestination { position },
    );

    let audiobooks = audiobooks_of(library_of(test.next_frame()));
    assert_eq!(audiobooks.lib_idx, 0);
    assert!(!audiobooks.loading && !audiobooks.failed);
    let ids: Vec<&str> = audiobooks.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["book-a", "book-b"],
        "the rows are the catalog in its order"
    );
    let letters: Vec<char> = audiobooks.rows.iter().map(|row| row.sort_letter).collect();
    assert_eq!(
        letters,
        ['A', 'F'],
        "the rail letters read the author sort keys"
    );
    assert_eq!(
        audiobooks.selected.as_deref(),
        Some("book-a"),
        "the first book is the Hero default"
    );
    let hero = audiobooks.hero.expect("the Hero paints");
    assert_eq!(hero.title, "Austen's End");
    assert_eq!(
        hero.meta_rows,
        ["Jane Austen", "02:00:00"],
        "the TUI hero producer's facts ride"
    );
}

/// A select moves the Hero and its Workspace (row 9.1): the select command
/// stores the pick, the next frame's Hero shows that book, and the seeded
/// detail's chapters ride the Workspace with no current mark.
#[test]
fn the_select_command_moves_the_hero_and_workspace() {
    let (mut test, _cmd_rx) = seeded_audiobooks_core();

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::AbsSelect {
            lib_idx: 0,
            row: "book-b".into(),
        },
    );

    let audiobooks = audiobooks_of(library_of(test.next_frame()));
    assert_eq!(audiobooks.selected.as_deref(), Some("book-b"));
    let hero = audiobooks.hero.expect("the Hero paints");
    assert_eq!(hero.title, "Bovary");
    let workspace = hero
        .workspace
        .expect("the seeded detail holds the Workspace");
    let titles: Vec<&str> = workspace
        .rows
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(titles, ["Chapter One", "Chapter Two"]);
    assert!(workspace.rows.iter().all(|row| !row.current));
}

/// The play and queue commands reach the Owner through the admission (row
/// 9.1): the play replaces the Owner's queue with the book and the queue
/// command appends it.
#[test]
fn the_play_and_queue_commands_reach_the_owner_through_the_admission() {
    let (mut test, cmd_rx) = seeded_audiobooks_core();

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::AbsPlay {
            lib_idx: 0,
            item_id: "book-a".into(),
        },
    );
    assert_eq!(
        commanded_book_id(&cmd_rx),
        "book-a",
        "the play replaced the Owner's queue with the book"
    );

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::AbsQueue {
            lib_idx: 0,
            item_id: "book-a".into(),
        },
    );
    assert_eq!(
        commanded_book_id(&cmd_rx),
        "book-a",
        "the queue command appended the book"
    );
}

/// A denial of the Owner's Audiobookshelf admission submits nothing and
/// flashes the TUI's owner-unavailable toast (row 9.1).
#[test]
fn a_denied_owner_submits_nothing_and_flashes_the_owner_unavailable_toast() {
    let (mut test, _cmd_rx) = seeded_audiobooks_core();
    test.core.app_mut().set_abs_admission_for_test(false);

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::AbsPlay {
            lib_idx: 0,
            item_id: "book-a".into(),
        },
    );

    let toast = test
        .next_frame()
        .status
        .toast
        .expect("the refusal flashed a toast");
    assert_eq!(toast.text, "Audiobookshelf playback owner is unavailable");
}

/// Stale ids change nothing (row 9.1): a play command whose book the
/// published catalog does not carry submits nothing and flashes no toast.
#[test]
fn a_stale_book_id_submits_nothing() {
    let (mut test, cmd_rx) = seeded_audiobooks_core();

    send_library(
        &mut test,
        mbv_gui::bridge::library::GuiLibraryCommand::AbsPlay {
            lib_idx: 0,
            item_id: "gone".into(),
        },
    );

    assert!(cmd_rx.try_recv().is_err(), "a stale id submits nothing");
    assert!(test.next_frame().status.toast.is_none());
}
