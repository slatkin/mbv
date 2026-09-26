use super::*;
use mbv_core::audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookProgress, AudiobookshelfLibrary,
};

fn book(id: &str, author_sort_key: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: format!("Book {id}"),
        author_display: Some(format!("Author {id}")),
        author_sort_key: author_sort_key.into(),
        cover_path: Some(format!("{id}-cover")),
        duration_seconds: 3_600.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

fn key(code: Key, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers }
}

fn state_with_books(books: Vec<AudiobookshelfBook>) -> AudiobookshelfBookBrowseState {
    let mut state = AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Books".into(),
        media_type: "book".into(),
    });
    state.append_page_books(0, books.len(), books);
    state.select(0);
    state
}

#[test]
fn book_slot_selector_and_hero_activation_emit_typed_intents() {
    let mut owner = BookContent::new();
    owner.set_content(
        &state_with_books(vec![book("book-a", "Adams"), book("book-z", "Zed")]),
        false,
    );

    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::SelectorPicked(1)),
        Some(Msg::Shell(request)) if matches!(request.as_ref(), ShellRequest::AudiobookshelfBookMove(AudiobookshelfBookMove::Bucket(1)))
    ));
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::HeroActivate),
        Some(Msg::Shell(request)) if matches!(request.as_ref(), ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::Activate))
    ));
}

#[test]
fn book_keys_emit_play_activate_enqueue_and_chapter_focus_intents() {
    let mut owner = BookContent::new();
    owner.set_content(&state_with_books(vec![book("book-a", "Adams")]), false);

    assert_eq!(
        owner.on_key(&key(Key::Char(' '), KeyModifiers::NONE)),
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::Play,)
        )))
    );
    assert_eq!(
        owner.on_key(&key(Key::Enter, KeyModifiers::NONE)),
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::Activate,)
        )))
    );
    assert_eq!(
        owner.on_key(&key(Key::Char('a'), KeyModifiers::CONTROL)),
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::Enqueue,)
        )))
    );
}

#[test]
fn enter_focuses_chapters_when_the_selected_book_has_chapters() {
    let mut owner = BookContent::new();
    let mut state = state_with_books(vec![book("book-a", "Adams")]);
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 30.0,
                title: "Chapter one".into(),
            }],
            Vec::new(),
        ),
    );
    owner.set_content(&state, false);

    assert_eq!(
        owner.on_key(&key(Key::Enter, KeyModifiers::NONE)),
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::FocusChapters,)
        )))
    );
}

#[test]
fn launch_snapshot_uses_surname_bucket_identity_and_book_id() {
    let mut owner = BookContent::new();
    owner.set_content(
        &state_with_books(vec![book("book-a", "Adams"), book("book-d", "Dover")]),
        false,
    );
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::BookBucket(AudiobookshelfBookBucket::AToC),
            }),
            Some(LibraryItemIdentity::Audiobookshelf {
                id: "book-a".into(),
            }),
        )
    );

    owner.select_bucket(1);
    assert_eq!(
        owner.launch_snapshot().0,
        Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::BookBucket(AudiobookshelfBookBucket::DToF),
        })
    );
    assert_eq!(
        owner.launch_snapshot().1,
        Some(LibraryItemIdentity::Audiobookshelf {
            id: "book-d".into(),
        })
    );
}

#[test]
fn reanchor_launch_state_falls_back_to_first_bucket_and_book() {
    let mut owner = BookContent::new();
    owner.set_content(
        &state_with_books(vec![book("book-a", "Adams"), book("book-d", "Dover")]),
        false,
    );
    let state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::BookBucket(AudiobookshelfBookBucket::VToZ),
        }),
        item: Some(LibraryItemIdentity::Audiobookshelf { id: "gone".into() }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(owner.selected_bucket, 0);
    assert_eq!(owner.selected_book_id(), Some("book-a"));
}
