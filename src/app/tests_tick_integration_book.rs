use crate::app::components::{AudiobookshelfBookComponent, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::components::msg::AudiobookshelfBookMove;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::TabSelection;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfLibrary};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

fn book(harness: &mut TickHarness) -> &mut AudiobookshelfBookComponent {
    harness
        .model_mut()
        .abs_book_component_mut(1)
        .expect("book component mounted")
}

fn harness(width: u16) -> TickHarness {
    let mut app = audiobookshelf_app();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library.clone());
    state.books = (0..8)
        .map(|index| AudiobookshelfBook {
            library_item_id: format!("book-{index}"),
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
            chapters: Vec::new(),
            audio_files: Vec::new(),
        })
        .collect();
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    state.selected_id = Some("book-0".into());
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse
        .push(AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
            id: "unused".into(),
            name: "Unused".into(),
            media_type: "book".into(),
        }));
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(1);
    app.terminal_width = width;
    app.terminal_height = 24;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().push_audiobookshelf_book_content();
    harness
}

fn draw(harness: &mut TickHarness, width: u16) {
    harness.model_mut().app.terminal_width = width;
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

#[test]
fn book_tick_wheel_claims_only_the_active_control_at_both_breakpoints() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut off = harness(width);
        draw(&mut off, width);
        off.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(off.step().raw_messages, vec![Msg::TerminalEvent(TerminalObserverEvent::NoOp)]);
        assert_eq!(book(&mut off).selected_book_id(), Some("book-0"));

        let mut on = harness(width);
        draw(&mut on, width);
        let rect = book(&mut on).geometry().left_area;
        on.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: rect.x + 1,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(on.step().raw_messages.contains(&Msg::Shell(
            ShellRequest::AudiobookshelfBookMove(AudiobookshelfBookMove::Book(1)),
        )));
        assert_eq!(book(&mut on).selected_book_id(), Some("book-1"));
    }
}

#[test]
fn book_tick_navigation_and_blank_click_are_owned_by_painted_control() {
    for width in [crate::app::TWO_COLUMN_THRESHOLD, crate::app::TWO_COLUMN_THRESHOLD - 1] {
        let mut harness = harness(width);
        draw(&mut harness, width);
        assert_eq!(book(&mut harness).selected_book_id(), Some("book-0"));
        harness.inject(Event::Keyboard(KeyEvent { code: Key::Down, modifiers: KeyModifiers::NONE }));
        assert!(!harness.step().raw_messages.is_empty());
        draw(&mut harness, width);
        assert_eq!(book(&mut harness).selected_book_id(), Some("book-1"));

        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(tuirealm::event::MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert_eq!(
            outcome.raw_messages,
            vec![Msg::TerminalEvent(TerminalObserverEvent::MouseClick { column: 0, row: 0 })]
        );
        assert_eq!(book(&mut harness).selected_book_id(), Some("book-1"));
    }
}
