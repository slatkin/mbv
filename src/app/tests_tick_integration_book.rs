use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::msg::AudiobookshelfBookMove;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::TabSelection;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn harness() -> TickHarness {
    let mut app = audiobookshelf_app();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "Books".into(),
        media_type: "book".into(),
    };
    let mut state = crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new(
        library.clone(),
    );
    // More books than the painted list shows, so the claimed wheel can step
    // the viewport (design D1).
    state.books = (0..20)
        .map(|i| AudiobookshelfBook {
            library_item_id: format!("book-{i}"),
            title: format!("Book {i}"),
            author_display: Some("Author".into()),
            author_sort_key: "Author".into(),
            cover_path: None,
            duration_seconds: 60.0,
            narrator: None,
            published_year: None,
            genres: vec![],
            description: None,
            series_name: None,
            chapters: vec![AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 60.0,
                title: "Chapter".into(),
            }],
            audio_files: vec![],
        })
        .collect();
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    state.selected_id = Some("book-0".into());
    state.detail_cache.insert(
        "book-0".into(),
        (
            vec![AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 60.0,
                title: "Chapter".into(),
            }],
            vec![],
        ),
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(
        crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new(
            AudiobookshelfLibrary {
                id: "unused".into(),
                name: "Unused".into(),
                media_type: "book".into(),
            },
        ),
    );
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(1);
    app.terminal_width = 140;
    app.terminal_height = 24;
    let mut h = TickHarness::new(app);
    h.model_mut().sync_mounted_surfaces();
    h
}

fn draw(h: &mut TickHarness) {
    draw_at(h, 140, 24);
}

fn draw_at(h: &mut TickHarness, width: u16, height: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| h.model_mut().draw_frame(f, false, false))
        .unwrap();
}

#[test]
fn books_narrow_hero_workspace_completes_empty_and_reanchors_stably() {
    let mut h = harness();
    {
        let browse = &mut h.model_mut().app.audiobookshelf_book_browse[1];
        browse.books[0].chapters.clear();
        browse.books[0].audio_files.clear();
        browse.detail_cache.clear();
        browse.detail_loading = true;
        browse.detail_loading_ids.insert("book-0".into());
    }
    h.model_mut().push_audiobookshelf_book_content();
    h.model_mut().app.terminal_width = 80;
    h.model_mut().app.terminal_height = 24;
    h.model_mut().sync_mounted_surfaces();
    draw_at(&mut h, 80, 24);

    // Enter travels through the mounted Library panel and opens the parent
    // Hero while the provider-owned chapter Workspace is loading.
    h.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    h.step();
    draw_at(&mut h, 80, 24);
    let panel = h
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel");
    assert!(panel.test_hero_overlay_open());
    assert!(panel.test_overlay_geometry().is_some());
    assert!(h.model().app.audiobookshelf_book_browse[1].detail_loading);
    assert!(h.model().test_abs_book_owner().chapter_list.rows().is_empty());

    // Complete the provider request directly at its state boundary. The
    // existing owner updates its Workspace without being recreated.
    let chapter = AudiobookshelfChapter {
        id: 0,
        start: 0.0,
        end: 60.0,
        title: "Chapter Ready".into(),
    };
    let browse = &mut h.model_mut().app.audiobookshelf_book_browse[1];
    browse.detail_loading = false;
    browse.detail_loading_ids.clear();
    browse.detail_cache.insert("book-0".into(), (vec![chapter], Vec::new()));
    h.model_mut().push_audiobookshelf_book_content();
    h.model_mut().sync_mounted_surfaces();
    draw_at(&mut h, 80, 24);
    assert!(!h.model().app.audiobookshelf_book_browse[1].detail_loading);
    assert_eq!(
        h.model().test_abs_book_owner().chapter_list.selected_target(),
        Some(&0)
    );

    // A refresh whose selected_id is stale retains the canonical parent and
    // chapter target by stable identity while the overlay stays open.
    let browse = &mut h.model_mut().app.audiobookshelf_book_browse[1];
    browse.books.rotate_left(1);
    browse.selected_id = Some("book-1".into());
    browse.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&browse.books);
    h.model_mut().push_audiobookshelf_book_content();
    h.model_mut().sync_mounted_surfaces();
    draw_at(&mut h, 80, 24);
    assert!(h
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .is_some_and(|panel| panel.test_hero_overlay_open()));
    assert_eq!(h.model().test_abs_book_owner().selected_book_id(), Some("book-0"));
    assert_eq!(
        h.model().test_abs_book_owner().chapter_list.selected_target(),
        Some(&0)
    );

    // Empty completion clears the old child rows and leaves an explicit empty
    // detail result in the retained owner.
    let browse = &mut h.model_mut().app.audiobookshelf_book_browse[1];
    browse.detail_cache.insert("book-0".into(), (Vec::new(), Vec::new()));
    browse.detail_loading = false;
    h.model_mut().push_audiobookshelf_book_content();
    h.model_mut().sync_mounted_surfaces();
    draw_at(&mut h, 80, 24);
    assert!(h.model().test_abs_book_owner().chapter_list.rows().is_empty());
    assert!(h
        .model()
        .test_abs_book_owner()
        .state
        .detail_cache
        .get("book-0")
        .is_some_and(|detail| detail.0.is_empty()));
}

#[test]
fn books_panel_is_focused_and_mouse_eligible() {
    let mut h = harness();
    assert_eq!(h.model().application.focus(), Some(&ComponentId::Library));
    draw(&mut h);
    let rect = h
        .model()
        .application
        .get_component(&ComponentId::Library)
        .unwrap()
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .unwrap()
        .test_list_rect()
        .unwrap();
    h.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: rect.x + 1,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    // The wheel steps the book list's viewport (design D1); the selection sat
    // on the window's top edge, so the drag rule rides it and the dragged
    // step reports the book position echo.
    assert!(h.step().raw_messages.iter().any(|m| matches!(
        m,
        Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(Some(_))
        ))
    )));
    assert_eq!(
        h.model().test_abs_book_owner().carrier.scroll(),
        1,
        "the window moved one display row"
    );
}

#[test]
fn chapter_row_click_routes_through_panel_owner() {
    let mut h = harness();
    draw(&mut h);
    let panel = h
        .model()
        .application
        .get_component(&ComponentId::Library)
        .unwrap()
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .unwrap();
    let hero = panel.test_wide_geometry().unwrap().hero;
    h.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hero.x + 1,
        row: hero.y + 1,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = h.step();
}

#[test]
fn book_owner_is_retained_across_tab_change() {
    let mut h = harness();
    h.model_mut()
        .test_abs_book_owner_mut()
        .carrier
        .select_target(&"book-1".into());
    h.model_mut().app.tab = TabSelection::Home;
    h.model_mut().sync_mounted_surfaces();
    h.model_mut().app.tab = TabSelection::AudiobookshelfLibrary(1);
    h.model_mut().sync_mounted_surfaces();
    assert_eq!(
        h.model().test_abs_book_owner().selected_book_id(),
        Some("book-1")
    );
}
