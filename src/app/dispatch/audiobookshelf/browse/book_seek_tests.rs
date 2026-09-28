use super::*;
use crate::app::tests::make_app_stub;
use mbv_queue::AudiobookshelfBookQueueItem;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;
use mbv_ui_model::tab_selection::TabSelection;

fn library() -> mbv_audiobookshelf::AudiobookshelfLibrary {
    mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Books".into(),
        media_type: "book".into(),
    }
}

fn book_queue_item(id: &str) -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(
        AudiobookshelfBookQueueItem {
            library_item_id: id.into(),
            title: "Book".into(),
            author: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: None,
        },
    ))
}

/// 7.4: the shell resolves the stable book-qualified chapter target the
/// component emitted (book identity + owner row discriminator) against the
/// book's chapter list and issues one absolute seek to the resolved chapter's
/// start, with no numeric display-position re-derivation.
#[test]
fn activating_book_qualified_chapter_target_seeks_to_that_chapter() {
    let mut app = make_app_stub();
    let rx = app.player.spy_on_commands();
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.selected_id = Some("book-1".into());
    state.detail_cache.insert(
        "book-1".into(),
        (
            vec![
                mbv_audiobookshelf::AudiobookshelfChapter {
                    id: 0,
                    start: 0.0,
                    end: 60.0,
                    title: "Intro".into(),
                },
                mbv_audiobookshelf::AudiobookshelfChapter {
                    id: 1,
                    start: 120.0,
                    end: 300.0,
                    title: "Later".into(),
                },
            ],
            Vec::new(),
        ),
    );
    app.audiobookshelf_book_browse.push(state);
    let mint = app.player_tab.queue.revision_mint();
    app.player_tab.queue =
        mbv_queue::PlaybackQueue::from_queue_items(vec![book_queue_item("book-1")], Some(0), mint);

    app.activate_audiobookshelf_book_row_target(Some(mbv_ui_msg::BookChapterTarget::new(
        "book-1".into(),
        1,
    )));

    assert!(
        matches!(
            rx.try_recv(),
            Ok(mbv_ctrl::player::PlayerCommand::SeekAbsolute(120.0))
        ),
        "the stable book-qualified target must resolve the chapter's own start"
    );
}
