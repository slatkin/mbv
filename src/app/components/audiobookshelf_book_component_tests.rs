use super::audiobookshelf_book::AudiobookshelfBookComponent;
use super::msg::{
    AudiobookshelfBookIntent, AudiobookshelfBookMove, LegacyTerminalEvent, Msg, ShellRequest,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfLibrary};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn abs_book_component_keeps_local_cursor_and_renders_without_app_state() {
    let library = AudiobookshelfLibrary {
        id: "books".into(),
        name: "Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library);
    state.books = vec![
        AudiobookshelfBook {
            library_item_id: "one".into(),
            title: "Book One".into(),
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
        },
        AudiobookshelfBook {
            library_item_id: "two".into(),
            title: "Book Two".into(),
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
        },
    ];
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    state.selected_id = Some("one".into());

    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true, false);
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::NextBookRow
        )))
    ));

    let play = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char(' '),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        play,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
            AudiobookshelfBookIntent::Play
        )))
    ));

    let unrelated = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('z'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        unrelated,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(key)))
            if key.code == crossterm::event::KeyCode::Char('z')
    ));

    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(output.contains("Book One"), "output: {output:?}");
}

fn book_state(count: usize, with_chapters: bool) -> AudiobookshelfBookBrowseState {
    let library = AudiobookshelfLibrary {
        id: "books".into(),
        name: "Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library);
    state.books = (0..count)
        .map(|index| {
            let id = format!("book-{index}");
            let chapters = with_chapters.then(|| {
                vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 0,
                    start: 0.0,
                    end: 60.0,
                    title: "Chapter 1".into(),
                }]
            });
            AudiobookshelfBook {
                library_item_id: id,
                title: format!("Book {index}"),
                author_display: Some("Author".into()),
                author_sort_key: "Author".into(),
                cover_path: None,
                duration_seconds: 60.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: chapters.clone().unwrap_or_default(),
                audio_files: Vec::new(),
            }
        })
        .collect();
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    state.selected_id = state.books.first().map(|book| book.library_item_id.clone());
    if with_chapters {
        state.detail_cache.insert(
            "book-0".into(),
            (
                vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 0,
                    start: 0.0,
                    end: 60.0,
                    title: "Chapter 1".into(),
                }],
                Vec::new(),
            ),
        );
    }
    state
}

#[test]
fn abs_book_component_gates_chapter_focus_after_wide_to_narrow_resize() {
    let state = book_state(1, true);
    assert_eq!(state.visible_rows("book-0").len(), 1);
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true, false);
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(!component.geometry().chapter_rows.is_empty());

    let focus = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        focus,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::FocusChapters
        )))
    ));
    let chapter_move = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        chapter_move,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::NextChapter
        )))
    ));

    let mut narrow = Terminal::new(TestBackend::new(60, 20)).unwrap();
    narrow
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let movement = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        movement,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::NextBookRow
        )))
    ));
    let activate = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        activate,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
            AudiobookshelfBookIntent::Activate
        )))
    ));
    let enqueue = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::CONTROL,
    }));
    assert!(matches!(
        enqueue,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
            AudiobookshelfBookIntent::Enqueue
        )))
    ));
}

#[test]
fn abs_book_component_page_stride_is_independent_of_inline_painted_rows() {
    let state = book_state(6, false);
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true, false);
    component.set_page_size(3);
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    let page = component.on(&Event::Keyboard(KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        page,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::NextBookPage
        )))
    ));
    let mut output = Terminal::new(TestBackend::new(60, 8)).unwrap();
    output
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let rendered: String = output
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(rendered.contains("Book 3"), "output: {rendered:?}");
}

#[test]
fn abs_book_component_shift_bracket_stays_on_legacy_bridge() {
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&book_state(2, false), true, false);
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::SHIFT,
    }));
    assert!(matches!(
        message,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(key)))
            if key.code == crossterm::event::KeyCode::Char('[')
                && key.modifiers == crossterm::event::KeyModifiers::SHIFT
    ));
}
