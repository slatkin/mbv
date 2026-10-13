use super::*;
use mbv_audiobookshelf::{AudiobookshelfBook, AudiobookshelfLibrary};
use mbv_render::components::media_list::ActiveProgress;
use tuirealm::event::KeyModifiers;

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
fn book_keys_emit_play_activate_and_ctrl_a_select_all_intents() {
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
    // Ctrl+A multi-selects the entire book list; enqueue stays on the
    // context menu.
    assert!(matches!(
        owner.on_key(&key(Key::Char('a'), KeyModifiers::CONTROL)),
        Some(Msg::Shell(request))
            if matches!(request.as_ref(), ShellRequest::SelectionProjection(summary) if summary.count == 1)
    ));
}

/// Context-menu spec, "Audiobookshelf row is selected": `.` on the selected
/// book row opens the menu anchored to the selection (no pointer anchor),
/// targeting exactly that book.
#[test]
fn dot_opens_the_selected_book_context_menu_anchored_to_the_selection() {
    let mut owner = BookContent::new();
    owner.set_content(
        &state_with_books(vec![book("book-a", "Abbey"), book("book-b", "Adams")]),
        false,
    );

    assert!(matches!(
        owner.on_key(&key(Key::Char('.'), KeyModifiers::NONE)),
        Some(Msg::Shell(ref shell_boxed))
            if matches!(
                shell_boxed.as_ref(),
                ShellRequest::RowContextMenu(ContextMenuTargets::Audiobookshelf(targets), None)
                    if targets.as_slice() == [AudiobookshelfMenuTarget::Book {
                        library_item_id: "book-a".into(),
                    }]
            )
    ));
}

/// Media-list-multi-select, "one entry path for every list": the same `.`
/// gesture over a Visual multi-selection emits every marked book in display
/// order.
#[test]
fn dot_over_a_visual_selection_emits_every_marked_book_in_list_order() {
    let mut owner = BookContent::new();
    // Both books share one surname bucket, so both rows paint together.
    owner.set_content(
        &state_with_books(vec![book("book-a", "Abbey"), book("book-b", "Adams")]),
        false,
    );

    // Shift+V begins Visual mode anchored on the selected book; one Down
    // extends the marked range to the next book.
    owner.on_key(&key(Key::Char('V'), KeyModifiers::SHIFT));
    owner.on_key(&key(Key::Down, KeyModifiers::NONE));
    assert!(matches!(
        owner.on_key(&key(Key::Char('.'), KeyModifiers::NONE)),
        Some(Msg::Shell(ref shell_boxed))
            if matches!(
                shell_boxed.as_ref(),
                ShellRequest::RowContextMenu(ContextMenuTargets::Audiobookshelf(targets), None)
                    if targets.as_slice() == [
                        AudiobookshelfMenuTarget::Book { library_item_id: "book-a".into() },
                        AudiobookshelfMenuTarget::Book { library_item_id: "book-b".into() },
                    ]
            )
    ));
}

#[test]
fn enter_plays_the_book_even_when_chapters_are_loaded() {
    // Enter on a selected book plays it — the global Enter-replaces-queue
    // behavior shared with podcasts and Emby (chapter activation was
    // previously unreachable on a book that was not already playing).
    let mut owner = BookContent::new();
    let mut state = state_with_books(vec![book("book-a", "Adams")]);
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![mbv_audiobookshelf::AudiobookshelfChapter {
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
            ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::Activate,)
        )))
    );
}

#[test]
fn tab_focuses_chapters_when_the_selected_book_has_chapters() {
    let mut owner = BookContent::new();
    let mut state = state_with_books(vec![book("book-a", "Adams")]);
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![mbv_audiobookshelf::AudiobookshelfChapter {
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
        owner.on_key(&key(Key::Tab, KeyModifiers::NONE)),
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::FocusChapters,)
        )))
    );
}

#[test]
fn launch_snapshot_uses_surname_bucket_identity_and_no_item() {
    // Books keep their persisted pill (the surname bucket) but never record
    // the selected book: restoration always lands on the first row.
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
            None,
        )
    );

    owner.select_bucket(1);
    assert_eq!(
        owner.launch_snapshot().0,
        Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::BookBucket(AudiobookshelfBookBucket::DToF),
        })
    );

    // A completed refresh that still lists the selected book replaces the
    // catalog in place (design D2): the selection and its bucket are
    // retained across the content push.
    owner.set_content(
        &state_with_books(vec![book("book-a", "Adams"), book("book-d", "Dover")]),
        false,
    );
    assert_eq!(owner.selected_bucket, 1);
    assert_eq!(owner.selected_book_id(), Some("book-d"));
}

#[test]
fn reanchor_launch_state_falls_back_to_first_bucket_and_book() {
    let mut owner = BookContent::new();
    owner.set_content(
        &state_with_books(vec![book("book-a", "Adams"), book("book-d", "Dover")]),
        false,
    );
    let state = mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::BookBucket(AudiobookshelfBookBucket::VToZ),
        }),
        item: Some(LibraryItemIdentity::Audiobookshelf { id: "gone".into() }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(owner.selected_bucket, 0);
    assert_eq!(owner.selected_book_id(), Some("book-a"));
}

/// Contract (hero-workspace-now-playing D3/D4/D5): the
/// named chapter row carries `NowPlaying` with the chapter-relative progress
/// the projection supplies, every other chapter keeps its derived state, and
/// an identical re-pushed projection skips `set_content` (the chapter rows
/// guard), so the painted frame stays claimable.
#[test]
fn set_now_playing_marks_the_playing_chapter_row_and_dedups_identical_projection() {
    use mbv_ui_model::now_playing::WorkspaceNowPlaying;

    let mut owner = BookContent::new();
    let mut state = state_with_books(vec![book("book-a", "Adams")]);
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![
                mbv_audiobookshelf::AudiobookshelfChapter {
                    id: 0,
                    start: 0.0,
                    end: 30.0,
                    title: "Chapter one".into(),
                },
                mbv_audiobookshelf::AudiobookshelfChapter {
                    id: 1,
                    start: 30.0,
                    end: 60.0,
                    title: "Chapter two".into(),
                },
            ],
            Vec::new(),
        ),
    );
    owner.set_content(&state, false);

    let playing = || WorkspaceNowPlaying::Chapter {
        library_item_id: "book-a".into(),
        chapter_id: 1,
        progress: 42,
    };
    owner.set_now_playing(Some(playing()));

    let states = |rows: &[MediaListRow<usize>]| -> Vec<(usize, MediaSemanticState)> {
        rows.iter()
            .filter_map(|row| match row {
                MediaListRow::Item {
                    target,
                    semantic_state,
                    ..
                } => Some((*target, semantic_state.clone())),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        states(owner.chapter_list.rows()),
        vec![
            (0, MediaSemanticState::Ordinary),
            (
                1,
                MediaSemanticState::NowPlaying {
                    progress: Some(ActiveProgress::new(42)),
                },
            ),
        ],
        "the named chapter row lights with the chapter-relative progress; the other chapter keeps its derived state"
    );

    // Observable set_content skip: a completed paint is retained until the
    // carrier's set_content invalidates it.
    let list = ratatui::layout::Rect::new(0, 0, 40, 3);
    let geometry = owner.chapter_list.wide().row_geometry(3);
    owner
        .chapter_list
        .wide_mut()
        .finish_view(list, list, &geometry, None);
    assert!(owner.chapter_list.current_content_rect().is_some());

    // Identical input skips set_content (painted frame stays claimable).
    owner.set_now_playing(Some(playing()));
    assert!(
        owner.chapter_list.current_content_rect().is_some(),
        "an identical projection must not re-run set_content"
    );

    // Control: changed input rebuilds (and invalidates the paint).
    owner.set_now_playing(Some(WorkspaceNowPlaying::Chapter {
        library_item_id: "book-a".into(),
        chapter_id: 1,
        progress: 43,
    }));
    assert!(owner.chapter_list.current_content_rect().is_none());
}

/// #745: reset returns the book owner to its first surname bucket and first
/// book, clears chapter focus and both lists, and a refresh that lands
/// afterwards preserves that selection instead of re-adopting the pre-reset
/// bucket or book.
#[test]
fn reset_presentation_returns_to_first_bucket_and_survives_a_later_refresh() {
    let books = || {
        state_with_books(vec![
            book("book-a", "Adams"),
            book("book-d", "Dover"),
            book("book-z", "Zed"),
        ])
    };
    let mut owner = BookContent::new();
    owner.set_content(&books(), false);
    owner.select_bucket(2);
    owner.chapter_focused = true;
    owner.carrier.set_scroll(2);
    owner.carrier.toggle_selection(&"book-z".to_string());
    assert_eq!(owner.selected_bucket, 2);
    assert_eq!(owner.selected_book_id(), Some("book-z"));

    owner.reset_presentation();

    assert_eq!(owner.selected_bucket, 0);
    assert_eq!(owner.selected_book_id(), Some("book-a"));
    assert!(!owner.chapter_focused);
    assert_eq!(owner.carrier.scroll(), 0);
    assert_eq!(owner.carrier.multi_selection().len(), 0);

    // A refresh completing after the reset preserves the reset selection.
    owner.set_content(&books(), false);
    assert_eq!(owner.selected_bucket, 0);
    assert_eq!(owner.selected_book_id(), Some("book-a"));
}
