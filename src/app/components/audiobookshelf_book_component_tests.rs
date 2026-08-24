use super::audiobookshelf_book::AudiobookshelfBookComponent;
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
    assert!(message.is_some());

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
