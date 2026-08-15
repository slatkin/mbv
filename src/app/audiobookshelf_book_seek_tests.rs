use super::super::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use super::super::types_tab_selection::TabSelection;
use super::*;
use crate::app::tests::make_app_stub;

fn library() -> mbv_core::audiobookshelf::AudiobookshelfLibrary {
    mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Books".into(),
        media_type: "book".into(),
    }
}

fn book_queue_item(id: &str) -> QueueItem {
    QueueItem::AudiobookshelfBook(AudiobookshelfBookQueueItem {
        library_item_id: id.into(),
        title: "Book".into(),
        author: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    })
}

/// A book with a chapter at 120s on the merged timeline; activating that
/// row on the active book issues one absolute seek to `chapters[].start`.
#[test]
fn activating_active_book_chapter_seeks_absolute_on_merged_timeline() {
    let mut app = make_app_stub();
    let rx = app.player.spy_on_commands();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.selected_id = Some("book-1".into());
    state.chapter_selection = Some(0);
    state.detail_cache.insert(
        "book-1".into(),
        (
            vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                id: 0,
                start: 120.0,
                end: 300.0,
                title: "Later".into(),
            }],
            Vec::new(),
        ),
    );
    app.audiobookshelf_book_browse.push(state);
    // Active queue slot is the book being browsed.
    app.player_tab.queue = mbv_core::playback_queue::PlaybackQueue::from_queue_items(
        vec![book_queue_item("book-1")],
        Some(0),
    );

    app.activate_audiobookshelf_book_row();

    assert!(
        matches!(
            rx.try_recv(),
            Ok(mbv_core::player::PlayerCommand::SeekAbsolute(120.0))
        ),
        "chapter activation must seek to chapters[].start on the merged timeline"
    );
}

/// A chapter row on a book that is NOT the active queue slot must not
/// touch the player — the seek is only meaningful for the active book.
#[test]
fn activating_foreign_book_chapter_does_not_seek() {
    let mut app = make_app_stub();
    let rx = app.player.spy_on_commands();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.selected_id = Some("book-2".into());
    state.chapter_selection = Some(0);
    state.detail_cache.insert(
        "book-2".into(),
        (
            vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 60.0,
                title: "Chapter".into(),
            }],
            Vec::new(),
        ),
    );
    app.audiobookshelf_book_browse.push(state);
    app.player_tab.queue = mbv_core::playback_queue::PlaybackQueue::from_queue_items(
        vec![book_queue_item("other-book")],
        Some(0),
    );

    app.activate_audiobookshelf_book_row();

    assert!(
        rx.try_recv().is_err(),
        "a foreign book's chapter row must not seek the active slot"
    );
}
