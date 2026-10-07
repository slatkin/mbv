//! Chapter-row activation (book-playback spec, "Chapter rows seek the merged
//! book timeline"): activation on the active book is one absolute seek on the
//! merged timeline, and activation on a book that is not the active slot
//! replaces the queue with that book at the chapter's offset and starts it.
//! Regression for the fixed silent no-op: activation on a book that was not
//! already playing did nothing, so chapters were unreachable there.

use super::*;
use mbv_audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
use mbv_ctrl::{CtrlCmd, WireCommand};
use mbv_queue::{AudiobookshelfBookQueueItem, AudiobookshelfItem, QueueItem};
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;

fn book(id: &str, title: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: title.into(),
        author_display: None,
        author_sort_key: "author".into(),
        cover_path: None,
        duration_seconds: 0.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

fn chapter(id: usize, start: f64, end: f64) -> AudiobookshelfChapter {
    AudiobookshelfChapter {
        id,
        start,
        end,
        title: format!("Chapter {id}"),
    }
}

fn book_queue_item(id: &str) -> AudiobookshelfBookQueueItem {
    AudiobookshelfBookQueueItem {
        library_item_id: id.into(),
        title: "Book".into(),
        author: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    }
}

/// A book-browse tab holding `book-a` and `book-b`, with `book-a` selected
/// and its chapter detail cached (three chapters at 0/100/200 seconds).
fn book_browse_app() -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library.clone());
    state.append_page_books(
        0,
        2,
        vec![book("book-a", "Book A"), book("book-b", "Book B")],
    );
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![
                chapter(0, 0.0, 100.0),
                chapter(1, 100.0, 200.0),
                chapter(2, 200.0, 300.0),
            ],
            Vec::new(),
        ),
    );
    state.detail_cache.insert(
        "book-b".into(),
        (
            vec![chapter(0, 0.0, 50.0), chapter(1, 50.0, 150.0)],
            Vec::new(),
        ),
    );
    app.audiobookshelf_book_browse.push(state);
    app.audiobookshelf_libraries.push(library);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app
}

#[test]
fn chapter_activation_on_the_active_book_seeks_without_resubmitting() {
    // Book-playback spec: chapter activation on the active book is one
    // absolute seek on the merged timeline and SHALL NOT restart the queue
    // slot or session — the queue is left untouched.
    let mut app = book_browse_app();
    let cmd_rx = live_owner_channel(&mut app);
    app.playback_queue_mut().adopt_queue_items_with_active(
        vec![QueueItem::Audiobookshelf(AudiobookshelfItem::Book(
            book_queue_item("book-a"),
        ))],
        0,
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
    });
    while cmd_rx.try_recv().is_ok() {}

    app.activate_audiobookshelf_book_row_target(Some(mbv_ui_msg::BookChapterTarget::new(
        "book-a".into(),
        2,
    )));

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            CtrlCmd::PlayerCmd(WireCommand::SeekAbsolute(200.0))
        ),
        "the active book's chapter activation is one absolute seek at the chapter's start"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the active book's chapter activation submits no queue replacement"
    );
}

#[test]
fn chapter_activation_on_a_book_that_is_not_active_replaces_and_plays_at_the_chapter_offset() {
    // Book-playback spec: activating a chapter on a book that is not the
    // active queue slot replaces the queue with that book starting at the
    // chapter's `start` offset and starts playback — the targeted book, not
    // the browse selection (`book-a`).
    let mut app = book_browse_app();
    let cmd_rx = live_owner_channel(&mut app);
    while cmd_rx.try_recv().is_ok() {}

    app.activate_audiobookshelf_book_row_target(Some(mbv_ui_msg::BookChapterTarget::new(
        "book-b".into(),
        1,
    )));

    let cmd = cmd_rx.try_recv().unwrap();
    let CtrlCmd::UnifiedQueueReplace {
        items, start_idx, ..
    } = &cmd
    else {
        panic!("expected a queue replacement, got {cmd:?}");
    };
    assert_eq!(start_idx, &Some(0), "the replacement starts playing");
    let book = items
        .first()
        .and_then(QueueItem::as_audiobookshelf_book)
        .expect("the replacement holds the targeted book");
    assert_eq!(book.library_item_id, "book-b");
    assert_eq!(
        book.position_ticks,
        50 * mbv_emby_model::TICKS_PER_SECOND,
        "the book starts at the activated chapter's offset"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "the chapter activation submits exactly the one replace-and-start"
    );
}
