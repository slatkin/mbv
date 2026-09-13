use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::components::msg::AudiobookshelfBookMove;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::TabSelection;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn harness() -> TickHarness {
    let mut app = audiobookshelf_app();
    let library = AudiobookshelfLibrary { id: "abs-books".into(), name: "Books".into(), media_type: "book".into() };
    let mut state = crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new(library.clone());
    state.books = (0..8).map(|i| AudiobookshelfBook {
        library_item_id: format!("book-{i}"), title: format!("Book {i}"), author_display: Some("Author".into()), author_sort_key: "Author".into(), cover_path: None, duration_seconds: 60.0, narrator: None, published_year: None, genres: vec![], description: None, series_name: None, chapters: vec![AudiobookshelfChapter { id: 0, start: 0.0, end: 60.0, title: "Chapter".into() }], audio_files: vec![],
    }).collect();
    state.buckets = crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
    state.selected_id = Some("book-0".into());
    state.detail_cache.insert(
        "book-0".into(),
        (vec![AudiobookshelfChapter { id: 0, start: 0.0, end: 60.0, title: "Chapter".into() }], vec![]),
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary { id: "unused".into(), name: "Unused".into(), media_type: "book".into() }));
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(1);
    app.terminal_width = 140; app.terminal_height = 24;
    let mut h = TickHarness::new(app);
    h.model_mut().sync_mounted_surfaces();
    h
}

fn draw(h: &mut TickHarness) {
    let mut terminal = Terminal::new(TestBackend::new(140, 24)).unwrap();    terminal.draw(|f| h.model_mut().draw_frame(f, false, false)).unwrap();
}

#[test]
fn books_panel_is_focused_and_mouse_eligible() {
    let mut h = harness();
    assert_eq!(h.model().application.focus(), Some(&ComponentId::Library));
    draw(&mut h);
    let rect = h.model().application.get_component(&ComponentId::Library).unwrap().as_any().downcast_ref::<LibraryPanel>().unwrap().test_list_rect().unwrap();
    h.inject(Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: rect.x + 1, row: rect.y, modifiers: KeyModifiers::NONE }));
    assert!(h.step().raw_messages.iter().any(|m| matches!(m, Msg::Shell(ShellRequest::AudiobookshelfBookMove(AudiobookshelfBookMove::Book(Some(_)))))));
}

#[test]
fn chapter_row_click_routes_through_panel_owner() {
    let mut h = harness(); draw(&mut h);
    h.inject(Event::Keyboard(KeyEvent { code: Key::Left, modifiers: KeyModifiers::NONE }));
    h.step(); draw(&mut h);
    let panel = h.model().application.get_component(&ComponentId::Library).unwrap().as_any().downcast_ref::<LibraryPanel>().unwrap();
    let hero = panel.test_wide_geometry().unwrap().hero;
    h.inject(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: hero.x + 1, row: hero.y + 1, modifiers: KeyModifiers::NONE }));
    let _ = h.step();
}

#[test]
fn book_owner_is_retained_across_tab_change() {
    let mut h = harness();
    h.model_mut().test_abs_book_owner_mut().carrier.select_target(&"book-1".into());
    h.model_mut().app.tab = TabSelection::Home;
    h.model_mut().sync_mounted_surfaces();
    h.model_mut().app.tab = TabSelection::AudiobookshelfLibrary(1);
    h.model_mut().sync_mounted_surfaces();
    assert_eq!(h.model().test_abs_book_owner().selected_book_id(), Some("book-1"));
}
