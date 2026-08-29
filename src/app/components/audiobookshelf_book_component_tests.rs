use super::audiobookshelf_book::AudiobookshelfBookComponent;
use super::msg::{AudiobookshelfBookIntent, AudiobookshelfBookMove, Msg, ShellRequest};
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
            AudiobookshelfBookMove::Book(1)
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
    assert_eq!(unrelated, None);

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

#[test]
fn abs_book_component_returns_none_when_unfocused_without_mutating_state() {
    let state = book_state(4, true);
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true, false);
    // Focus the chapter list locally so there is interaction state to guard.
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.chapter_selection(), Some(0));
    component.set_content(&state, false, false);

    for (code, modifiers) in [
        (Key::Down, KeyModifiers::NONE),
        (Key::PageDown, KeyModifiers::NONE),
        (Key::Enter, KeyModifiers::NONE),
        (Key::Char('a'), KeyModifiers::CONTROL),
    ] {
        let message = component.on(&Event::Keyboard(KeyEvent { code, modifiers }));
        assert_eq!(message, None);
        assert_eq!(component.selected_book_id(), Some("book-0"));
        assert_eq!(component.selected_bucket(), 0);
        assert_eq!(component.chapter_selection(), Some(0));
    }
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
fn abs_book_component_does_not_focus_hidden_chapters_on_narrow_left() {
    let state = book_state(1, true);
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true, false);

    for rendered in [false, true] {
        if rendered {
            let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
            terminal
                .draw(|frame| component.view(frame, frame.area()))
                .unwrap();
        }
        let focus = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Left,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(focus, None);
        assert!(component.chapter_selection().is_none());
    }

    let movement = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        movement,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(_)
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
            AudiobookshelfBookMove::ChapterFocus(Some(0))
        )))
    ));
    let chapter_move = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        chapter_move,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::ChapterFocus(Some(_))
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
            AudiobookshelfBookMove::Book(_)
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

/// split-audiobookshelf-cursor-ownership D1 / task 1.2: the page stride comes
/// from the component's own painted list geometry, and nothing else. A taller
/// painted area pages further than a shorter one, and PageDown carries the
/// resolved book index it landed on.
#[test]
fn abs_book_component_page_stride_comes_from_painted_geometry() {
    let page_jump = |height: u16| {
        let state = book_state(30, false);
        let mut component = AudiobookshelfBookComponent::new();
        component.set_content(&state, true, false);
        let mut terminal = Terminal::new(TestBackend::new(60, height)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let page = component.on(&Event::Keyboard(KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        }));
        let Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(AudiobookshelfBookMove::Book(
            index,
        )))) = page
        else {
            panic!("PageDown must carry the resolved book index, got {page:?}");
        };
        index
    };

    let short = page_jump(8);
    let tall = page_jump(24);
    assert!(
        short >= 1,
        "a page jump advances past a single row: {short}"
    );
    assert!(
        tall > short,
        "a taller painted list pages further: tall={tall} short={short}"
    );
}

/// split-audiobookshelf-cursor-ownership D4 / task 1.3 → 5.2: when a content
/// push drops the book the component had selected, the component resets its
/// own `chapter_selection` / `browser_offset` / `selected_bucket` rather than
/// adopting the shell snapshot's copies.
#[test]
fn abs_book_component_drops_stale_chapter_focus_when_selection_vanishes() {
    let state = book_state(1, true);
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&state, true, false);
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(!component.geometry().chapter_rows.is_empty());

    // Focus the chapter list locally.
    let focus = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        focus,
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::ChapterFocus(Some(0))
        )))
    ));
    assert_eq!(component.chapter_selection(), Some(0));

    // New content in which the selected book is gone: the component resets
    // its own chapter focus (the projected type cannot carry one).
    let mut replacement = book_state(1, true);
    replacement.books[0].library_item_id = "book-99".into();
    replacement.selected_id = Some("book-99".into());
    replacement.buckets =
        crate::app::types_audiobookshelf_browse::build_surname_buckets(&replacement.books);
    component.set_content(&replacement, true, false);

    assert_eq!(
        component.chapter_selection(),
        None,
        "stale chapter focus must reset when the selected book vanishes"
    );
}

#[test]
fn abs_book_component_unmatched_shift_bracket_stays_unclaimed() {
    let mut component = AudiobookshelfBookComponent::new();
    component.set_content(&book_state(2, false), true, false);
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::SHIFT,
    }));
    assert_eq!(message, None);
}
