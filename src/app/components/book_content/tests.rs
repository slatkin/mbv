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

#[test]
fn launch_snapshot_uses_the_fixed_index_for_non_ascii_led_surnames() {
    let mut owner = BookContent::new();
    owner.set_content(
        &state_with_books(vec![
            book("book-symbol", "'N Sync"),
            book("book-ae", "Ædegaard"),
            book("book-umlaut", "Über"),
        ]),
        false,
    );
    let last_bucket = owner
        .state
        .buckets
        .last()
        .expect("expected a populated surname bucket");
    assert_eq!(last_bucket.index, 1);
    assert_eq!(owner.selected_bucket, owner.state.buckets.len() - 1);
    assert_eq!(
        owner.launch_snapshot().0,
        Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::BookBucket(AudiobookshelfBookBucket::DToF),
        })
    );
}

#[test]
fn launch_snapshot_is_empty_without_book_buckets_or_selected_book() {
    let mut owner = BookContent::new();
    owner.set_content(
        &AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Books".into(),
            media_type: "book".into(),
        }),
        false,
    );
    assert_eq!(owner.launch_snapshot(), (None, None));
}

#[test]
fn content_shows_one_pill_bar_for_the_surname_buckets() {
    let mut owner = BookContent::new();
    owner.set_content(&state_with_books(vec![book("book-1", "Adams")]), false);
    let bucket_count = owner.state.buckets.len();
    let content = owner.content();
    let selector = content.selector.expect("expected a selector row");
    assert_eq!(selector.pills.len(), bucket_count);
    // Secondary-row absence is owned by the shared panel skeleton test;
    // this owner test covers only the surname-bucket selector projection.
}

#[test]
fn content_uses_portrait_artwork_for_a_book() {
    let mut owner = BookContent::new();
    owner.set_content(&state_with_books(vec![book("book-1", "Adams")]), false);
    let content = owner.content();
    assert_eq!(
        content.hero.unwrap().facts.artwork.shape,
        super::super::library_panel::ArtworkShape::Portrait
    );
}

#[test]
fn content_shows_progress_as_a_plain_meta_row() {
    let mut owner = BookContent::new();
    let mut state = state_with_books(vec![book("book-1", "Adams")]);
    state.progress.insert(
        "book-1".into(),
        AudiobookshelfBookProgress {
            library_item_id: "book-1".into(),
            current_time_seconds: 0.0,
            is_finished: true,
        },
    );
    owner.set_content(&state, false);
    let content = owner.content();
    assert!(content
        .hero
        .unwrap()
        .facts
        .meta_rows
        .iter()
        .any(|row| row == "Finished"));
}

#[test]
fn content_exposes_chapters_as_the_workspace() {
    let mut owner = BookContent::new();
    let mut state = state_with_books(vec![book("book-1", "Adams")]);
    state.detail_cache.insert(
        "book-1".into(),
        (
            Vec::new(),
            vec![AudiobookshelfAudioFile {
                index: 0,
                ino: "ino".into(),
                duration: 60.0,
            }],
        ),
    );
    owner.set_content(&state, false);
    let has_workspace = owner
        .content()
        .hero
        .as_ref()
        .is_some_and(|hero| hero.workspace.is_some());
    assert!(has_workspace);
    assert_eq!(owner.chapter_list.rows().len(), 1);
    let MediaListRow::Item { trailing, .. } = &owner.chapter_list.rows()[0] else {
        panic!("chapter rows are items");
    };
    assert_eq!(trailing, &Some(MediaListTrailing::Gutter("1:00".into())));

    let MediaListRow::Item { trailing, .. } = &owner.carrier.rows()[0] else {
        panic!("book rows are items");
    };
    assert_eq!(trailing, &Some(MediaListTrailing::Gutter("1:00".into())));
}
