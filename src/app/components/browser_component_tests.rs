use super::browser::BrowserComponent;
use crate::app::components::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::{make_item, make_items};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers as CrosstermKeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Local keyboard navigation routes through typed `ShellRequest`s (task
/// 5.3d): while focused, the component moves its own cursor exactly the way
/// the legacy `App::move_lib_cursor_rows`/`jump_lib_cursor` bindings move
/// the App cursor, and returns the typed request in place of the raw
/// `Msg::Legacy` key so the shell drives the App cursor through the same
/// App methods (never in addition — no double movement). A 40-item flat
/// list rendered 100 columns wide packs two items per row and pages
/// `(height - 1) * cols = 9 * 2 = 18` items — every case below lands on
/// the legacy stride, and the two clamp cases pin the ends.
#[test]
fn browser_local_navigation_mirrors_legacy_flat_movement() {
    let cases = [
        // (key, from, expected)
        (KeyCode::Down, 0, 2),
        (KeyCode::Char('j'), 0, 2),
        (KeyCode::Up, 4, 2),
        (KeyCode::Char('k'), 4, 2),
        (KeyCode::Left, 4, 3),
        (KeyCode::Char('h'), 4, 3),
        (KeyCode::Right, 4, 5),
        (KeyCode::Char('l'), 4, 5),
        (KeyCode::Down, 39, 39),  // clamp at the last item
        (KeyCode::Up, 1, 0),      // clamp at the first item
        (KeyCode::Left, 0, 0),    // clamp at the left edge
        (KeyCode::Right, 39, 39), // clamp at the right edge
        // PageDown/PageUp stride (height - 1) * cols — the page excludes
        // the count/search header line, not the full painted height.
        (KeyCode::PageDown, 10, 28),
        (KeyCode::PageUp, 28, 10),
        (KeyCode::Home, 39, 0),
        (KeyCode::End, 0, 39),
    ];
    for (key, from, expected) in cases {
        let mut browser = BrowserComponent::new();
        browser.set_content(LibraryListRenderCtx::from_items(make_items(40), 0, 0), true);
        browser.set_cursor_for_test(from);
        let mut terminal = Terminal::new(TestBackend::new(100, 10)).unwrap();
        terminal
            .draw(|frame| browser.view(frame, frame.area()))
            .unwrap();
        let message = browser.handle_crossterm_key(KeyEvent::new(key, CrosstermKeyModifiers::NONE));
        assert_eq!(
            browser.cursor(),
            expected,
            "{key:?} from cursor {from} in a two-column flat list"
        );
        assert_eq!(
            message,
            Some(Msg::Shell(expected_movement_request(key))),
            "{key:?} must return the typed movement request in place of the raw legacy key"
        );
    }

    // Unfocused (Queue/playback own panel focus): the movement keys do not
    // mutate the component cursor and the raw key is consumed (no legacy
    // forwarding), keeping those surfaces authoritative.
    let mut browser = BrowserComponent::new();
    browser.set_content(
        LibraryListRenderCtx::from_items(make_items(40), 0, 0),
        false,
    );
    browser.set_cursor_for_test(7);
    let mut terminal = Terminal::new(TestBackend::new(100, 10)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();
    for key in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::Char('h'),
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Char('l'),
    ] {
        let message = browser.handle_crossterm_key(KeyEvent::new(key, CrosstermKeyModifiers::NONE));
        assert_eq!(
            browser.cursor(),
            7,
            "unfocused {key:?} must not move the cursor"
        );
        assert!(
            matches!(message, Some(Msg::Legacy(LegacyTerminalEvent::NoOp))),
            "unfocused {key:?} must be consumed (no legacy fallthrough)"
        );
    }
}

/// The typed movement request the focused two-column browser must return
/// for each navigation key (task 5.3d), asserted against the emitted
/// `Msg::Shell` payload by `browser_local_navigation_mirrors_legacy_flat_movement`.
/// The page payload is the painted display-row stride `(height - 1) = 9`
/// the 100-wide, 10-tall test list reports via `page_rows()` — the App
/// applies its own column count to that stride, exactly like the legacy arm.
fn expected_movement_request(key: KeyCode) -> ShellRequest {
    match key {
        KeyCode::Up | KeyCode::Char('k') => ShellRequest::BrowserMoveRows { rows: -1 },
        KeyCode::Down | KeyCode::Char('j') => ShellRequest::BrowserMoveRows { rows: 1 },
        KeyCode::PageUp => ShellRequest::BrowserMoveRows { rows: -9 },
        KeyCode::PageDown => ShellRequest::BrowserMoveRows { rows: 9 },
        KeyCode::Home => ShellRequest::BrowserJumpCursor { to_end: false },
        KeyCode::End => ShellRequest::BrowserJumpCursor { to_end: true },
        KeyCode::Left | KeyCode::Char('h') => ShellRequest::BrowserMoveColumn { delta: -1 },
        KeyCode::Right | KeyCode::Char('l') => ShellRequest::BrowserMoveColumn { delta: 1 },
        _ => unreachable!("{key:?} must be a browsed navigation key"),
    }
}

/// Letter-grouped lists (60 items render bucketed rows with a header row
/// between buckets and a ragged trailing row per bucket) striding one
/// PAINTED item row per Up/Down, exactly like `App::letter_vertical_delta`:
/// headers do not participate and a ragged target row falls back to its
/// last item. The painted (2-column) item rows are
///   A\u{2013}C: [0,1]..[26,27],[28]   (ragged: item 28 alone)
///   D\u{2013}F: [29,30]..[43,44]
///   G\u{2013}I: [45,46]..[57,58],[59] (ragged: item 59 alone)
/// Flat arithmetic (the pre-align +1 and the naive +2) lands on a
/// different item in every bracketed case, so each assertion is decisive.
#[test]
fn browser_local_navigation_skips_letter_headers_and_ragged_rows() {
    let mut items = Vec::new();
    for i in 0..15 {
        let mut item = make_item(&format!("Alpha {i}"), "Movie");
        item.id = format!("a{i}");
        items.push(item);
    }
    for i in 0..14 {
        let mut item = make_item(&format!("Beta {i}"), "Movie");
        item.id = format!("b{i}");
        items.push(item);
    }
    for i in 0..16 {
        let mut item = make_item(&format!("Delta {i}"), "Movie");
        item.id = format!("d{i}");
        items.push(item);
    }
    for i in 0..15 {
        let mut item = make_item(&format!("Gamma {i}"), "Movie");
        item.id = format!("g{i}");
        items.push(item);
    }
    assert_eq!(items.len(), 60);

    let cases = [
        // (key, from, expected) — letter-grouped 2-column layout
        (KeyCode::Down, 27, 28), // ragged target row [28]: fall back to its last item
        (KeyCode::Down, 28, 29), // across the D–F header: next *item* row is [29,30]
        (KeyCode::Up, 29, 28),   // back across the header
        (KeyCode::Down, 59, 59), // clamp at the very last item
        (KeyCode::Up, 0, 0),     // clamp at the very first item
        (KeyCode::Home, 59, 0),  // sorted order first
        (KeyCode::End, 0, 59),   // sorted order last
        (KeyCode::Left, 4, 3),   // sorted-order ±1 (column adjacency)
        (KeyCode::Right, 4, 5),
    ];
    for (key, from, expected) in cases {
        let mut browser = BrowserComponent::new();
        browser.set_content(LibraryListRenderCtx::from_items(items.clone(), 0, 0), true);
        browser.set_cursor_for_test(from);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| browser.view(frame, frame.area()))
            .unwrap();
        browser.handle_crossterm_key(KeyEvent::new(key, CrosstermKeyModifiers::NONE));
        assert_eq!(
            browser.cursor(),
            expected,
            "{key:?} from cursor {from} in the letter-grouped list"
        );
    }
}

/// Wide-Movies exact parity (task 5.3d prep): with the shell's
/// `set_wide_movies` projection set on a >=82-wide rendered list, the
/// right rail strides ONE item per row — exactly the legacy
/// `current_library_columns` result (the wide renderer shows the list in
/// the right rail even when that rail is wide enough to pack two columns).
/// Down from 0 lands at 1, not 2, and returns the typed rows request;
/// Left/Right/h/l stay unbound locally (one-column list) while the raw key
/// still forwards as `Msg::Legacy`.
#[test]
fn browser_local_navigation_strides_one_column_for_wide_movies() {
    let mut browser = BrowserComponent::new();
    browser.set_content(LibraryListRenderCtx::from_items(make_items(12), 0, 0), true);
    browser.set_wide_movies(true, false, false);
    let mut terminal = Terminal::new(TestBackend::new(100, 10)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();

    let message =
        browser.handle_crossterm_key(KeyEvent::new(KeyCode::Down, CrosstermKeyModifiers::NONE));
    assert_eq!(
        browser.cursor(),
        1,
        "wide-Movies rail Down must stride one item, not two"
    );
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::BrowserMoveRows { rows: 1 })),
        "wide-Movies Down must return the typed rows request"
    );

    browser.handle_crossterm_key(KeyEvent::new(KeyCode::Down, CrosstermKeyModifiers::NONE));
    assert_eq!(browser.cursor(), 2);
    browser.handle_crossterm_key(KeyEvent::new(KeyCode::Up, CrosstermKeyModifiers::NONE));
    assert_eq!(browser.cursor(), 1);

    for key in [
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Char('h'),
        KeyCode::Char('l'),
    ] {
        let message = browser.handle_crossterm_key(KeyEvent::new(key, CrosstermKeyModifiers::NONE));
        assert_eq!(
            browser.cursor(),
            1,
            "wide-Movies rail {key:?} must stay unbound locally"
        );
        assert!(
            matches!(message, Some(Msg::Legacy(LegacyTerminalEvent::NoOp))),
            "wide-Movies {key:?} must be consumed (no legacy fallthrough)"
        );
    }
}

#[test]
fn browser_syncs_cursor_from_context_on_set_content() {
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
    // Component cursor moved to 1
    assert_eq!(browser.cursor(), 1);

    // set_content with App cursor at 1 (as it would be after the shell handles the request)
    browser.set_content(
        LibraryListRenderCtx::from_items(
            vec![make_item("one", "Movie"), make_item("two", "Movie")],
            1, // App cursor updated to match component
            0,
        ),
        true,
    );
    // Component cursor syncs from context
    assert_eq!(browser.cursor(), 1);

    // set_content with App cursor at 0 (external change like tab switch)
    browser.set_content(
        LibraryListRenderCtx::from_items(
            vec![make_item("one", "Movie"), make_item("two", "Movie")],
            0, // App cursor changed externally
            0,
        ),
        true,
    );
    // Component cursor follows App cursor
    assert_eq!(browser.cursor(), 0);
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
