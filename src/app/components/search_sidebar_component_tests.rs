//! Mouse tests for `SearchSidebarComponent` (task 5.1): result-row click
//! selects (Up/Down), result-row double-click activates (Enter), chip click
//! sets the type filter (Tab cycle), and an outside click dismisses (Esc).

use super::msg::{Msg, ServiceRequest, ShellRequest};
use super::search_sidebar::SearchSidebarComponent;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn click(x: u16, y: u16) -> Event<super::user_event::UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

fn key(code: Key) -> Event<super::user_event::UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

/// A 30x16 sidebar panel on a 40x16 frame leaves column 30.. as outside.
fn sidebar_component() -> SearchSidebarComponent {
    let mut comp = SearchSidebarComponent::new();
    comp.sidebar.query = "clip".into();
    comp.sidebar.results = vec![
        make_item("Birthday Clip", "Movie"),
        make_item("Other Clip", "Series"),
        make_item("Third Clip", "Movie"),
    ];
    comp.set_panel_area(Some(ratatui::layout::Rect::new(0, 0, 30, 16)));
    comp
}

fn draw(component: &mut SearchSidebarComponent) {
    let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
}

fn click_event(x: u16, y: u16) -> Event<super::user_event::UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn search_sidebar_result_click_selects_like_the_arrow_keys() {
    let mut comp = sidebar_component();
    comp.sidebar.list_height = 10;
    draw(&mut comp);
    let rows = comp.test_results().regions().to_vec();
    assert_eq!(rows.len(), 3, "all three results must be painted");

    comp.reset_mouse_gestures_for_test();
    let (rect, index) = rows[2];
    let msg = comp.on(&click_event(rect.x, rect.y));
    assert_eq!(msg, None, "selection is a local cursor move, no Msg");
    assert_eq!(comp.sidebar.cursor, index);
}

#[test]
fn search_sidebar_result_double_click_activates_like_enter() {
    let mut comp = sidebar_component();
    comp.sidebar.list_height = 10;
    draw(&mut comp);
    let rows = comp.test_results().regions().to_vec();

    let (rect, index) = rows[1];
    assert_eq!(comp.on(&click_event(rect.x, rect.y)), None);
    let msg = comp.on(&click_event(rect.x, rect.y));
    assert_eq!(comp.sidebar.cursor, index);
    assert!(
        matches!(
            msg,
            Some(Msg::Shell(ShellRequest::SearchActivate { id, item_type }))
                if id == "id" && item_type == "Series"
        ),
        "double-click must emit the Enter-equivalent SearchActivate"
    );
}

#[test]
fn search_sidebar_chip_click_sets_the_type_filter_like_tab() {
    let mut comp = sidebar_component();
    comp.sidebar.list_height = 10;
    draw(&mut comp);
    let chips = comp.test_chips().regions().to_vec();
    assert_eq!(
        chips.len(),
        3,
        "All, Movie, and Series chips must be painted"
    );

    comp.reset_mouse_gestures_for_test();
    let (rect, chip) = chips[1];
    let msg = comp.on(&click_event(rect.x, rect.y));
    assert_eq!(msg, None, "filtering is a local mutation, no Msg");
    assert_eq!(comp.sidebar.type_filter, chip);
    assert_eq!(chip, 1, "chip tags follow the type_filter index space");
    assert_eq!(comp.sidebar.cursor, 0, "filter change resets the cursor");
    assert_eq!(comp.sidebar.scroll, 0);

    // Filtering by Movie leaves only two rows selectable.
    draw(&mut comp);
    assert_eq!(comp.test_results().regions().len(), 2);
}

#[test]
fn search_sidebar_outside_click_dismisses_like_esc() {
    let mut comp = sidebar_component();
    comp.sidebar.list_height = 10;
    draw(&mut comp);
    let frame = comp.test_frame();

    comp.reset_mouse_gestures_for_test();
    assert_eq!(
        comp.on(&click_event(frame.x + frame.width + 1, frame.y + 1)),
        Some(Msg::Shell(ShellRequest::DismissSearch))
    );
}

#[test]
fn search_sidebar_query_row_click_is_a_noop() {
    let mut comp = sidebar_component();
    comp.sidebar.list_height = 10;
    draw(&mut comp);
    let frame = comp.test_frame();

    comp.reset_mouse_gestures_for_test();
    // The query row is the always-focused single input with no cursor-
    // positioning keyboard path, so clicking it must do nothing.
    let msg = comp.on(&click_event(frame.x, frame.y));
    assert_eq!(msg, None);
}

#[test]
fn search_sidebar_clock_paths_still_work_alongside_mouse() {
    use super::user_event::UserEvent;
    use std::time::Duration;
    let mut comp = sidebar_component();
    comp.sidebar.list_height = 10;
    comp.sidebar.query.clear();
    comp.on(&key(Key::Char('a')));
    comp.on(&key(Key::Char('b')));
    let deadline = comp.debounce_deadline.expect("debounce armed");
    let msg = comp.on(&Event::User(UserEvent::Clock(
        deadline + Duration::from_millis(1),
    )));
    assert!(matches!(
        msg,
        Some(Msg::Service(ServiceRequest::SearchQuery(q))) if q == "ab"
    ));
}
