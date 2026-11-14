//! Headless tests of the Audiobooks Hero's Workspace interaction state (D16):
//! the contracts of `audiobooks-workspace.test.ts` — the chapter cursor's
//! movement and clamping, its re-resolution to the marked chapter (or the
//! first chapter) when its chapter leaves the frame, the reset when the
//! selected book changes, the chapter command the cursor names, and the book
//! commands the selected book names. The current chapter is the window's
//! derivation from the playhead (gui-change-driven-sections design D5): the
//! tests pass the predicted position explicitly, so no clock runs. The
//! harness builds a `DioxusDocument` whose root creates the store and runs
//! each scenario in one effect over a recording core link, so no test opens a
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
use crate::window::sections::Sections;

use crate::window::library::list::ListMove;

use super::{
    current_chapter_row, marked_chapter_of, move_abs_chapter_cursor, play_abs_book,
    play_abs_chapter, queue_abs_book, reset_abs_chapter_cursor, resolve_abs_chapter_cursor,
    workspace_of,
};
use crate::window::library::state::LibraryState;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&LibraryState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness(
    (tx, facts, scenario, position): (mpsc::Sender<CoreInput>, Facts, Scenario, f64),
) -> Element {
    let state = use_hook(move || {
        // The playhead's predicted position, held at the scenario's value.
        let playhead_position = Memo::new(move || position);
        LibraryState::new(Sections::new(), CoreLink::new(tx), playhead_position)
    });
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "audiobooks-workspace-harness" }
    }
}

/// Run one scenario at the default position (0 ticks) and return the facts it
/// recorded and the commands the core link received. Bounded by construction:
/// the effect runs once and the poll loop stops when the document goes idle.
fn run(
    scenario: impl Fn(&LibraryState, &Facts) + 'static,
) -> (Vec<String>, mpsc::Receiver<CoreInput>) {
    run_with_position(0.0, scenario)
}

/// Run one scenario at `position` ticks.
fn run_with_position(
    position: f64,
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
                position,
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

/// Put the cursor on `chapter`.
fn cursor_at(state: &LibraryState, chapter: usize) {
    let mut audiobooks_workspace = state.audiobooks_workspace;
    audiobooks_workspace.with_mut(|held| held.cursor = Some(chapter));
}

/// The Workspace's cursor the assertions read.
fn cursor_fact(state: &LibraryState, facts: &Facts) {
    facts.lock().expect("facts").push(format!(
        "cursor={:?}",
        state.audiobooks_workspace.peek().cursor
    ));
}

// A cursor move steps to the next chapter.
#[test]
fn a_cursor_move_steps_to_the_next_chapter() {
    let (facts, rx) = run(|state, facts| {
        cursor_at(state, 1);

        move_abs_chapter_cursor(
            state,
            ListMove::Down,
            &audiobooks_detail(chapters(), Some("book1"), None),
        );
        cursor_fact(state, facts);
    });

    assert_eq!(facts, ["cursor=Some(2)"]);
    assert_eq!(commands(&rx), []);
}

// A cursor move past the last chapter stays there.
#[test]
fn a_cursor_move_past_the_last_chapter_stays_there() {
    let (facts, rx) = run(|state, facts| {
        cursor_at(state, 2);

        move_abs_chapter_cursor(
            state,
            ListMove::Down,
            &audiobooks_detail(chapters(), Some("book1"), None),
        );
        cursor_fact(state, facts);
    });

    assert_eq!(facts, ["cursor=Some(2)"]);
    assert_eq!(commands(&rx), []);
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

// A cursor keeps its chapter while the frame paints it.
#[test]
fn a_cursor_keeps_its_chapter_while_the_frame_paints_it() {
    let detail = audiobooks_detail(chapters(), Some("book1"), Some("book1"));

    let cursor = resolve_abs_chapter_cursor(
        workspace_of(&detail),
        Some(1),
        marked_chapter_of(&detail, Some(30.0 * 60.0 * SECOND_F)),
    );
    assert_eq!(cursor.map(|row| row.chapter), Some(1));
}

// A cursor whose chapter left the frame resolves to the marked chapter.
#[test]
fn a_cursor_whose_chapter_left_the_frame_resolves_to_the_marked_chapter() {
    let detail = audiobooks_detail(chapters(), Some("book1"), Some("book1"));

    let cursor = resolve_abs_chapter_cursor(
        workspace_of(&detail),
        Some(7),
        marked_chapter_of(&detail, Some(30.0 * 60.0 * SECOND_F)),
    );
    assert_eq!(cursor.map(|row| row.chapter), Some(2));
}

// A cursor with no marked chapter resolves to the first chapter.
#[test]
fn a_cursor_with_no_marked_chapter_resolves_to_the_first_chapter() {
    let detail = audiobooks_detail(chapters(), Some("book1"), None);

    let cursor = resolve_abs_chapter_cursor(workspace_of(&detail), Some(7), None);
    assert_eq!(cursor.map(|row| row.chapter), Some(1));
}

// A cursor over a book with no chapters resolves to none.
#[test]
fn a_cursor_over_a_book_with_no_chapters_resolves_to_none() {
    let detail = audiobooks_detail(Vec::new(), Some("book1"), None);

    let cursor = resolve_abs_chapter_cursor(workspace_of(&detail), Some(1), None);
    assert!(cursor.is_none());
}

// A cursor whose chapter left the frame moves on from the marked chapter:
// the position at 30:00 marks the second chapter, so the move starts there
// and clamps at the book's end.
#[test]
fn a_cursor_whose_chapter_left_the_frame_moves_on_from_the_marked_chapter() {
    let (facts, rx) = run_with_position(30.0 * 60.0 * SECOND_F, |state, facts| {
        cursor_at(state, 7);
        let detail = audiobooks_detail(chapters(), Some("book1"), Some("book1"));
        move_abs_chapter_cursor(state, ListMove::Down, &detail);
        cursor_fact(state, facts);
    });

    assert_eq!(facts, ["cursor=Some(2)"]);
    assert_eq!(commands(&rx), []);
}

// A cursor reset starts the Workspace over.
#[test]
fn a_cursor_reset_starts_the_workspace_over() {
    let (facts, rx) = run(|state, facts| {
        cursor_at(state, 2);

        reset_abs_chapter_cursor(state);
        cursor_fact(state, facts);
    });

    assert_eq!(facts, ["cursor=None"]);
    assert_eq!(commands(&rx), []);
}

// The chapter command names the cursor chapter and the selected book.
#[test]
fn the_chapter_command_names_the_cursor_chapter_and_the_selected_book() {
    let (_facts, rx) = run(|state, _facts| {
        cursor_at(state, 2);

        play_abs_chapter(
            state,
            LIB_IDX,
            &audiobooks_detail(chapters(), Some("book1"), None),
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

// The chapter command sends nothing while no chapter carries the cursor.
#[test]
fn the_chapter_command_sends_nothing_while_no_chapter_carries_the_cursor() {
    let (_facts, rx) = run(|state, _facts| {
        play_abs_chapter(
            state,
            LIB_IDX,
            &audiobooks_detail(Vec::new(), Some("book1"), None),
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
