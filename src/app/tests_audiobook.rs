use super::*;
use crate::app::tests::*;

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