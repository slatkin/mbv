use super::browser::BrowserComponent;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::make_item;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers as CrosstermKeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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

    browser.handle_crossterm_key(KeyEvent::new(KeyCode::Down, CrosstermKeyModifiers::NONE));
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

#[test]
fn browser_mouse_uses_the_painted_two_column_cell_for_left_and_right_clicks() {
    let mut browser = BrowserComponent::new();
    browser.set_content(
        LibraryListRenderCtx::from_items(
            vec![make_item("first", "Movie"), make_item("second", "Movie")],
            0,
            0,
        ),
        true,
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 6)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();
    let layout = browser.test_layout();
    let area = layout.left_area;
    let cell_width = library_cell_width(area, 2);
    let position = (area.x + cell_width + LIBRARY_COLUMN_GAP, area.y);

    let left = browser.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.0,
        row: position.1,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        left,
        Some(crate::app::components::msg::Msg::Shell(
            crate::app::components::msg::ShellRequest::BrowserClick {
                region: crate::app::components::msg::BrowserHitRegion::LeftRow(1),
                ..
            }
        ))
    ));

    let right = browser.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: position.0,
        row: position.1,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        right,
        Some(crate::app::components::msg::Msg::Shell(
            crate::app::components::msg::ShellRequest::BrowserClick {
                region: crate::app::components::msg::BrowserHitRegion::ContextMenu(1),
                ..
            }
        ))
    ));
}
