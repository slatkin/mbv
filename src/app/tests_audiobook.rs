use super::*;
use crate::app::tests::*;
use crate::app::types_selection_modal::SelectionModalSource;

fn audiobookshelf_book_app() -> App {
    let mut app = make_app_stub();
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state =
        super::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new(library.clone());
    state.books = vec![mbv_core::audiobookshelf::AudiobookshelfBook {
        library_item_id: "book-a".into(),
        title: "Book A".into(),
        author_display: Some("Pierce".into()),
        author_sort_key: "Pierce".into(),
        cover_path: None,
        duration_seconds: 0.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
            id: 0,
            start: 0.0,
            end: 3600.0,
            title: "Chapter 1".into(),
        }],
        audio_files: Vec::new(),
    }];
    state.selected_id = Some("book-a".into());
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![mbv_core::audiobookshelf::AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 3600.0,
                title: "Chapter 1".into(),
            }],
            Vec::new(),
        ),
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

/// Task 6.2: the book library tab exposure — its `media_type` forks to the
/// book browse kind, and the podcast browse state at the same index stays
/// untouched. Chapter focus is covered only in the wide workspace.
#[test]
fn audiobookshelf_book_tab_dispatches_to_book_kind_not_podcast() {
    let mut app = audiobookshelf_book_app();
    app.layout.main.audiobookshelf_book_wide_right_area = ratatui::layout::Rect::new(40, 0, 20, 15);
    assert_eq!(
        app.audiobookshelf_kind_at(0),
        Some(super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book)
    );

    let left = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Left,
        crossterm::event::KeyModifiers::NONE,
    );
    assert_eq!(
        app.handle_key_view_dispatch(left),
        Some(false),
        "Left arrow must be consumed by the book handler"
    );
    assert_eq!(
        app.audiobookshelf_book_browse[0].chapter_selection,
        Some(0),
        "Left arrow on a selected book must focus the hero's chapter list"
    );
    assert!(
        app.audiobookshelf_browse.is_empty(),
        "book tab must not consult the podcast browse state"
    );
    assert_eq!(app.player_tab.total_queue_len(), 0);
}

#[test]
fn wide_book_chapter_target_changes_chapter_focus() {
    let mut app = audiobookshelf_book_app();
    app.layout.main.browse_destination = Some(app.tab);
    app.layout.main.audiobookshelf_book_wide_right_area = ratatui::layout::Rect::new(40, 0, 20, 15);
    app.layout.main.audiobookshelf_book_chapter_rows =
        vec![(ratatui::layout::Rect::new(42, 4, 16, 1), 0)];

    assert!(app.click_set_cursor(43, 4));
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, Some(0));
}

#[test]
fn narrow_book_parent_double_click_opens_the_chapter_modal() {
    let mut app = audiobookshelf_book_app();
    app.layout.main.left_area = ratatui::layout::Rect::new(10, 10, 20, 5);
    app.layout.main.inline_hero_area = app.layout.main.left_area;
    app.layout.main.browse_destination = Some(app.tab);
    app.refocus_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));

    let click = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: 12,
        row: 11,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    app.handle_mouse(click);

    assert!(
        matches!(
            app.selection_modal.as_ref().map(|modal| &modal.source),
            Some(SelectionModalSource::Book { book_id }) if book_id == "book-a"
        ),
        "narrow parent double-click must open the same chapter modal as Enter"
    );
    assert_eq!(
        app.audiobookshelf_book_browse[0].chapter_selection, None,
        "narrow parent double-click must not enter chapter focus"
    );
}

#[test]
fn book_modal_activation_resolves_stable_chapter_id_after_reorder() {
    let mut app = audiobookshelf_book_app();
    app.audiobookshelf_book_browse[0].detail_cache.insert(
        "book-a".into(),
        (
            vec![
                mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 10,
                    start: 0.0,
                    end: 10.0,
                    title: "First".into(),
                },
                mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 20,
                    start: 10.0,
                    end: 20.0,
                    title: "Second".into(),
                },
            ],
            Vec::new(),
        ),
    );
    app.open_audiobookshelf_book_selection_modal();
    app.selection_modal.as_mut().unwrap().cursor = 1;

    app.audiobookshelf_book_browse[0]
        .detail_cache
        .get_mut("book-a")
        .unwrap()
        .0
        .reverse();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert_eq!(
        app.audiobookshelf_book_browse[0].chapter_selection,
        Some(0),
        "activation must resolve the selected chapter ID, not its old row index"
    );
}

#[test]
fn book_modal_projects_pending_detail_as_loading() {
    let mut app = audiobookshelf_book_app();
    let state = &mut app.audiobookshelf_book_browse[0];
    state.detail_cache.remove("book-a");

    app.open_audiobookshelf_book_selection_modal();

    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Loading
    ));
    assert!(app.audiobookshelf_book_browse[0]
        .detail_loading_ids
        .contains("book-a"));
}

#[test]
fn book_modal_projects_empty_detail_as_empty() {
    let mut app = audiobookshelf_book_app();
    app.audiobookshelf_book_browse[0]
        .detail_cache
        .insert("book-a".into(), (Vec::new(), Vec::new()));

    app.open_audiobookshelf_book_selection_modal();

    assert!(matches!(
        app.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Empty
    ));
}

#[test]
fn book_modal_uses_audio_file_rows_when_chapters_are_empty() {
    let mut app = audiobookshelf_book_app();
    app.audiobookshelf_book_browse[0].detail_cache.insert(
        "book-a".into(),
        (
            Vec::new(),
            vec![mbv_core::audiobookshelf::AudiobookshelfAudioFile {
                index: 1,
                ino: "file-1".into(),
                duration: 60.0,
            }],
        ),
    );

    app.open_audiobookshelf_book_selection_modal();

    let modal = app.selection_modal.as_ref().unwrap();
    assert_eq!(modal.state.rows()[0].item_id(), Some("audio-file:file-1"));
}

#[test]
fn book_modal_audio_file_activation_closes_without_seeking() {
    let mut app = audiobookshelf_book_app();
    app.audiobookshelf_book_browse[0].detail_cache.insert(
        "book-a".into(),
        (
            Vec::new(),
            vec![mbv_core::audiobookshelf::AudiobookshelfAudioFile {
                index: 1,
                ino: "file-1".into(),
                duration: 60.0,
            }],
        ),
    );
    app.open_audiobookshelf_book_selection_modal();

    app.activate_selection_modal_item();

    assert!(app.selection_modal.is_none());
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, Some(0));
}

#[test]
fn book_modal_keyboard_navigation_activation_and_cancellation_preserve_parent_position() {
    let mut app = audiobookshelf_book_app();
    app.audiobookshelf_book_browse[0].detail_cache.insert(
        "book-a".into(),
        (
            vec![
                mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 10,
                    start: 0.0,
                    end: 10.0,
                    title: "First".into(),
                },
                mbv_core::audiobookshelf::AudiobookshelfChapter {
                    id: 20,
                    start: 10.0,
                    end: 20.0,
                    title: "Second".into(),
                },
            ],
            Vec::new(),
        ),
    );
    app.audiobookshelf_book_browse[0].scroll = 8;
    app.open_audiobookshelf_book_selection_modal();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 1);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.selection_modal.as_ref().unwrap().cursor, 1);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('['),
        crossterm::event::KeyModifiers::NONE,
    ));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(']'),
        crossterm::event::KeyModifiers::NONE,
    ));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.selection_modal.is_none());
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, Some(1));

    for key in [
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyCode::Backspace,
    ] {
        let mut app = audiobookshelf_book_app();
        app.audiobookshelf_book_browse[0].scroll = 8;
        app.open_audiobookshelf_book_selection_modal();
        app.handle_key(crossterm::event::KeyEvent::new(
            key,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(app.selection_modal.is_none());
        assert_eq!(app.panel_focus, PanelFocus::Library);
        assert_eq!(
            app.audiobookshelf_book_browse[0].selected_id.as_deref(),
            Some("book-a")
        );
        assert_eq!(app.audiobookshelf_book_browse[0].scroll, 8);
    }
}

#[test]
fn book_modal_loading_and_empty_states_ignore_movement_and_activation() {
    let mut loading = audiobookshelf_book_app();
    loading.audiobookshelf_book_browse[0]
        .detail_cache
        .remove("book-a");
    loading.open_audiobookshelf_book_selection_modal();
    assert!(matches!(
        loading.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Loading
    ));
    loading.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    ));
    loading.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(']'),
        crossterm::event::KeyModifiers::NONE,
    ));
    loading.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(loading.selection_modal.is_none());

    let mut empty = audiobookshelf_book_app();
    empty.audiobookshelf_book_browse[0]
        .detail_cache
        .insert("book-a".into(), (Vec::new(), Vec::new()));
    empty.open_audiobookshelf_book_selection_modal();
    assert!(matches!(
        empty.selection_modal.as_ref().unwrap().state,
        SelectionModalListState::Empty
    ));
    empty.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    ));
    empty.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('['),
        crossterm::event::KeyModifiers::NONE,
    ));
    empty.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(empty.selection_modal.is_none());
}
