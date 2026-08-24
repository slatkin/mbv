use super::browser::BrowserComponent;
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::make_item;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;

#[test]
fn browser_keeps_cursor_local_between_shell_syncs() {
    let mut browser = BrowserComponent::new();
    browser.set_content(
        LibraryListRenderCtx::from_items(
            vec![make_item("one", "Movie"), make_item("two", "Movie")],
            0,
            0,
        ),
        true,
    );

    browser.handle_crossterm_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    browser.set_content(
        LibraryListRenderCtx::from_items(
            vec![make_item("one", "Movie"), make_item("two", "Movie")],
            0,
            0,
        ),
        true,
    );

    assert_eq!(browser.cursor(), 1);
}

#[test]
fn browser_renders_the_shared_generic_rows() {
    let mut browser = BrowserComponent::new();
    browser.set_content(
        LibraryListRenderCtx::from_items(vec![make_item("Movie one", "Movie")], 0, 0),
        true,
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();

    let rendered: String = (0..40)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect();
    assert!(rendered.contains("Movie one"));
}
