//! Headless tests of the Audiobooks Hero's Workspace (D16): the contracts of
//! `audiobooks-workspace.test.ts` — the marked chapter, the chapter the
//! pointer's marker names, the chapter command that marker sends, and the book
//! commands the selected book names. The current chapter is the window's
//! derivation from the playhead (gui-change-driven-sections design D5): the
//! tests pass the predicted position explicitly, so no clock runs. The harness
//! builds a `DioxusDocument` whose root creates the store and runs each
//! scenario in one effect over a recording core link, so no test opens a
//! window or sleeps, and no IPC runs.

use std::sync::{Arc, Mutex, mpsc};

use blitz_dom::Document as _;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksHeroFrame,
    GuiAudiobooksWorkspaceFrame, GuiMetaKindFrame, GuiMetaSegmentFrame,
};
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::clock::{Clock, RecordingClock};
use crate::window::sections::Sections;

use super::{
    abs_chapter_at, current_chapter_row, marked_chapter_of, play_abs_book, play_abs_chapter,
    queue_abs_book,
};
use crate::window::library::state::LibraryState;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(move || {
        // The playhead's predicted position: none, so only the assertions that
        // name a position mark a chapter.
        let playhead_position = Memo::new(|| 0.0);
        let clock: Arc<dyn Clock> = Arc::new(RecordingClock::new());
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position, clock)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "audiobooks-workspace-harness" }
    }
}

/// Run one scenario and return the facts it recorded and the commands the core
/// link received. Bounded by construction: the effect runs once and the poll
/// loop stops when the document goes idle.
fn run(
    scenario: impl Fn(&LibraryState, &Facts) + 'static,
) -> (Vec<String>, mpsc::Receiver<CoreInput>) {
    let (tx, rx) = mpsc::channel::<CoreInput>();
    let facts: Facts = Arc::new(Mutex::new(Vec::new()));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            harness,
            (
                tx,
                Arc::clone(&facts),
                std::rc::Rc::new(move |state: &LibraryState, recorded: &Facts| {
                    scenario(state, recorded);
                }),
            ),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    poll_until_idle(&mut doc);
    let recorded = facts.lock().expect("facts").clone();
    (recorded, rx)
}

/// Poll the document until its effects have run and their re-renders landed.
fn poll_until_idle(doc: &mut DioxusDocument) {
    for _ in 0..8 {
        if !doc.poll(None) {
            return;
        }
    }
    panic!("document did not become idle");
}

/// The Library commands the scenario sent, as the core thread receives them.
fn commands(rx: &mpsc::Receiver<CoreInput>) -> Vec<mbv::GuiLibraryCommand> {
    let mut commands = Vec::new();
    while let Ok(input) = rx.try_recv() {
        if let CoreInput::Library(command) = input {
            commands.push(command);
        }
    }
    commands
}

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
const SECOND: i64 = 10_000_000;

/// The same second, in f64, so the assertions compare without a cast.
const SECOND_F: f64 = 10_000_000.0;

/// A book's chapters on the merged timeline: the first spans 0–21:00, the
/// second 21:00–45:00 (design D5's `startTicks`/`endTicks`).
fn chapters() -> Vec<GuiAudiobooksChapterRowFrame> {
    vec![
        chapter(1, "First Contact", "21:00", 0, 21 * 60 * SECOND),
        chapter(
            2,
            "Fallen Angel",
            "24:00",
            21 * 60 * SECOND,
            45 * 60 * SECOND,
        ),
    ]
}

/// One chapter row, as the frame carries it.
fn chapter(
    number: usize,
    title: &str,
    duration: &str,
    start_ticks: i64,
    end_ticks: i64,
) -> GuiAudiobooksChapterRowFrame {
    GuiAudiobooksChapterRowFrame {
        chapter: number,
        title: title.to_owned(),
        duration: duration.to_owned(),
        start_ticks,
        end_ticks,
    }
}

/// The selected book's Hero facts with a chapter Workspace.
fn hero(rows: Vec<GuiAudiobooksChapterRowFrame>) -> GuiAudiobooksHeroFrame {
    GuiAudiobooksHeroFrame {
        title: "The Caves of Steel".to_owned(),
        meta_rows: vec![
            vec![GuiMetaSegmentFrame {
                text: "Isaac Asimov".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
            vec![GuiMetaSegmentFrame {
                text: "9 h 51 m".to_owned(),
                kind: GuiMetaKindFrame::Time,
            }],
            vec![GuiMetaSegmentFrame {
                text: "17%".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            }],
        ],
        duration_row: Some(1),
        progress_row: Some(2),
        overview: None,
        art_shape: GuiArtShapeFrame::Portrait,
        art: None,
        workspace: Some(GuiAudiobooksWorkspaceFrame { rows }),
    }
}

/// The Audiobooks library index the book commands name.
const LIB_IDX: usize = 4;

/// A detail part whose selected book's Hero carries `workspace`, with the
/// book the playhead plays when one does.
fn audiobooks_detail(
    rows: Vec<GuiAudiobooksChapterRowFrame>,
    selected: Option<&str>,
    playing_book: Option<&str>,
) -> GuiAudiobooksDetail {
    GuiAudiobooksDetail {
        workspace_loading: false,
        selected: selected.map(str::to_owned),
        playing_book_id: playing_book.map(str::to_owned),
        hero: selected.map(|_| hero(rows)),
    }
}

// The marked chapter follows the predicted position while the Hero's book
// plays: an earlier position marks the first chapter.
#[test]
fn the_marked_chapter_follows_the_predicted_position_while_the_hero_book_plays() {
    let detail = audiobooks_detail(chapters(), Some("book1"), Some("book1"));

    let marked = marked_chapter_of(&detail, Some(30.0 * 60.0 * SECOND_F));
    assert_eq!(marked.map(|row| row.chapter), Some(2));
    let marked = marked_chapter_of(&detail, Some(5.0 * 60.0 * SECOND_F));
    assert_eq!(marked.map(|row| row.chapter), Some(1));
}

// A different playing book marks no chapter.
#[test]
fn a_different_playing_book_marks_no_chapter() {
    let detail = audiobooks_detail(chapters(), Some("book1"), Some("book2"));

    assert!(marked_chapter_of(&detail, Some(30.0 * 60.0 * SECOND_F)).is_none());
    let not_playing = audiobooks_detail(chapters(), Some("book1"), None);
    assert!(marked_chapter_of(&not_playing, Some(30.0 * 60.0 * SECOND_F)).is_none());
}

// The span's end is exclusive: a position at a chapter's end marks the next
// chapter.
#[test]
fn a_position_at_a_chapter_end_marks_the_next_chapter() {
    let detail = audiobooks_detail(chapters(), Some("book1"), Some("book1"));

    let marked = marked_chapter_of(&detail, Some(21.0 * 60.0 * SECOND_F));
    assert_eq!(marked.map(|row| row.chapter), Some(2));
}

// An empty span never holds a position.
#[test]
fn an_empty_span_never_holds_a_position() {
    let parts = vec![
        chapter(1, "Part 1", "21:00", 0, 0),
        chapter(2, "Part 2", "24:00", 0, 0),
    ];

    assert!(current_chapter_row(&parts, 0.0).is_none());
    assert!(current_chapter_row(&parts, 10.0 * SECOND_F).is_none());
}

// A marker names the chapter its number carries.
#[test]
fn a_marker_names_the_chapter_its_number_carries() {
    let detail = audiobooks_detail(chapters(), Some("book1"), None);

    let chapter = abs_chapter_at(&detail, "2");
    assert_eq!(chapter.map(|row| row.chapter), Some(2));
}

// A marker that names no painted chapter names nothing: a chapter the book
// does not carry, and a marker that is no chapter number at all.
#[test]
fn a_marker_that_names_no_painted_chapter_names_nothing() {
    let detail = audiobooks_detail(chapters(), Some("book1"), None);

    assert!(abs_chapter_at(&detail, "7").is_none());
    assert!(abs_chapter_at(&detail, "first").is_none());
}

// A Workspace with no chapter resolves no marker.
#[test]
fn a_workspace_with_no_chapter_resolves_no_marker() {
    let detail = audiobooks_detail(Vec::new(), Some("book1"), None);

    assert!(abs_chapter_at(&detail, "1").is_none());
}

// The chapter command names the chapter and the selected book.
#[test]
fn the_chapter_command_names_the_chapter_and_the_selected_book() {
    let (_facts, rx) = run(|state, _facts| {
        play_abs_chapter(
            state,
            LIB_IDX,
            &audiobooks_detail(chapters(), Some("book1"), None),
            2,
        );
    });

    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsPlayChapter {
            lib_idx: 4,
            book_id: "book1".to_owned(),
            chapter: 2,
        }]
    );
}

// The chapter command sends nothing while the frame selects no book.
#[test]
fn the_chapter_command_sends_nothing_while_the_frame_selects_no_book() {
    let (_facts, rx) = run(|state, _facts| {
        play_abs_chapter(
            state,
            LIB_IDX,
            &audiobooks_detail(chapters(), None, None),
            2,
        );
    });

    assert_eq!(commands(&rx), []);
}

// Play sends the play command for the selected book.
#[test]
fn play_sends_the_play_command_for_the_selected_book() {
    let (_facts, rx) = run(|state, _facts| {
        play_abs_book(
            state,
            LIB_IDX,
            &audiobooks_detail(chapters(), Some("book1"), None),
        );
    });

    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsPlay {
            lib_idx: 4,
            item_id: "book1".to_owned(),
        }]
    );
}

// Add to queue sends the enqueue command for the selected book.
#[test]
fn add_to_queue_sends_the_enqueue_command_for_the_selected_book() {
    let (_facts, rx) = run(|state, _facts| {
        queue_abs_book(
            state,
            LIB_IDX,
            &audiobooks_detail(chapters(), Some("book1"), None),
        );
    });

    assert_eq!(
        commands(&rx),
        [mbv::GuiLibraryCommand::AbsQueue {
            lib_idx: 4,
            item_id: "book1".to_owned(),
        }]
    );
}

// A book command sends nothing while the frame selects no book.
#[test]
fn a_book_command_sends_nothing_while_the_frame_selects_no_book() {
    let (_facts, rx) = run(|state, _facts| {
        play_abs_book(state, LIB_IDX, &audiobooks_detail(chapters(), None, None));
        queue_abs_book(state, LIB_IDX, &audiobooks_detail(chapters(), None, None));
    });

    assert_eq!(commands(&rx), []);
}
