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

fn book(id: &str) -> mbv_core::audiobookshelf::AudiobookshelfBook {
    mbv_core::audiobookshelf::AudiobookshelfBook {
        library_item_id: id.into(),
        title: "Book".into(),
        author_display: Some("Author".into()),
        author_sort_key: "Author".into(),
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

fn chapter(
    id: usize,
    start: f64,
    end: f64,
    title: &str,
) -> mbv_core::audiobookshelf::AudiobookshelfChapter {
    mbv_core::audiobookshelf::AudiobookshelfChapter {
        id,
        start,
        end,
        title: title.into(),
    }
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

    app.activate_audiobookshelf_book_row(Some(0));

    assert!(
        matches!(
            rx.try_recv(),
            Ok(mbv_core::player::PlayerCommand::SeekAbsolute(120.0))
        ),
        "chapter activation must seek to chapters[].start on the merged timeline"
    );
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
                mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 0,
                    start: 0.0,
                    end: 60.0,
                    title: "Intro".into(),
                },
                mbv_core::audiobookshelf::AudiobookshelfChapter {
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
    app.player_tab.queue = mbv_core::playback_queue::PlaybackQueue::from_queue_items(
        vec![book_queue_item("book-1")],
        Some(0),
    );

    app.activate_audiobookshelf_book_row_target(Some(
        crate::app::components::msg::BookChapterTarget::new("book-1".into(), 1),
    ));

    assert!(
        matches!(
            rx.try_recv(),
            Ok(mbv_core::player::PlayerCommand::SeekAbsolute(120.0))
        ),
        "the stable book-qualified target must resolve the chapter's own start"
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

    app.activate_audiobookshelf_book_row(Some(0));

    assert!(
        rx.try_recv().is_err(),
        "a foreign book's chapter row must not seek the active slot"
    );
}

/// 7.4 + D4: the chapter owner's target carries the stable Service chapter
/// number, so a detail refresh that re-composes `visible_rows` (a chapter
/// arriving late ahead of the selected one) cannot make the absolute seek land
/// on a different chapter. The target is produced by the shared chapter owner
/// before the refresh, then resolved after it.
#[test]
fn chapter_seek_target_survives_late_chapter_recomposition() {
    use crate::app::components::{
        msg::{AudiobookshelfBookIntent, Msg, ShellRequest},
        AudiobookshelfBookComponent,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::{AppComponent, Component};
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    let mut app = make_app_stub();
    let rx = app.player.spy_on_commands();
    app.tab = TabSelection::AudiobookshelfLibrary(0);

    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.selected_id = Some("book-1".into());
    state.books = vec![book("book-1")];
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    // First composition: the selected chapter has Service id 7 at display
    // position 1.
    state.detail_cache.insert(
        "book-1".into(),
        (
            vec![
                chapter(3, 0.0, 60.0, "Intro"),
                chapter(7, 120.0, 300.0, "Selected"),
            ],
            Vec::new(),
        ),
    );

    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    // Enter the wide chapter pane, move to the second row, and activate it to
    // capture the target the shared owner emits.
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let activate = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
        AudiobookshelfBookIntent::ActivateChapter(Some(target)),
    ))) = activate
    else {
        panic!("expected a book-qualified chapter target, got {activate:?}");
    };
    assert_eq!(
        target.row_discriminator(),
        7,
        "the target must carry the Service chapter number, not the display position"
    );

    // A chapter arrives late ahead of the selected one: `visible_rows` is
    // re-composed and display position 1 now names Service id 5. With a
    // position target the seek would jump to that chapter.
    state.detail_cache.insert(
        "book-1".into(),
        (
            vec![
                chapter(3, 0.0, 60.0, "Intro"),
                chapter(5, 60.0, 120.0, "Inserted"),
                chapter(7, 120.0, 300.0, "Selected"),
            ],
            Vec::new(),
        ),
    );
    component.set_content(&state, false);
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(component.chapter_focused());
    let activate = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
        AudiobookshelfBookIntent::ActivateChapter(Some(target)),
    ))) = activate
    else {
        panic!("expected the preserved book-qualified chapter target, got {activate:?}");
    };
    assert_eq!(
        target.row_discriminator(),
        7,
        "the owner preserves the stable discriminator across a re-composition"
    );

    app.audiobookshelf_book_browse.push(state);
    app.player_tab.queue = mbv_core::playback_queue::PlaybackQueue::from_queue_items(
        vec![book_queue_item("book-1")],
        Some(0),
    );
    app.activate_audiobookshelf_book_row_target(Some(target));

    assert!(
        matches!(
            rx.try_recv(),
            Ok(mbv_core::player::PlayerCommand::SeekAbsolute(seconds)) if seconds == 120.0
        ),
        "the seek must resolve the selected chapter's own start, not the inserted chapter's"
    );
}
