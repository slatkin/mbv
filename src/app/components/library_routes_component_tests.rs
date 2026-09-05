//! Mouse tests for `LibraryRoutesComponent` (task 5.1): row click selects
//! (Up/Down), row double-click enters (Enter), outside click follows the
//! Esc path (`LibraryRoutesEsc`), for both picker stages.

use super::library_routes::LibraryRoutesComponent;
use super::msg::{Msg, ShellRequest};
use crate::app::types_context_menu::LibraryRoutePopup;
use mbv_core::remote_player::DaemonEndpoint;
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

fn draw(component: &mut LibraryRoutesComponent) {
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
}

fn library_popup() -> LibraryRoutePopup {
    LibraryRoutePopup {
        stage: crate::app::types_context_menu::LibraryRouteStage::PickLibrary {
            items: vec![
                ("movies".into(), "Movies".into(), None),
                ("music".into(), "Music".into(), None),
            ],
        },
        cursor: 0,
    }
}

#[test]
fn library_routes_row_click_selects_like_the_arrow_keys() {
    let mut component = LibraryRoutesComponent::new();
    component.set_content(&library_popup());
    draw(&mut component);
    let rows = component.test_rows().regions().to_vec();
    assert_eq!(rows.len(), 2);

    component.reset_mouse_gestures_for_test();
    let (rect, index) = rows[1];
    let msg = component.on(&click(rect.x, rect.y));
    assert_eq!(msg, None, "selection is a local cursor move, no Msg");
    assert_eq!(component.cursor(), index);
}

#[test]
fn library_routes_row_double_click_enters_like_enter_key() {
    let mut component = LibraryRoutesComponent::new();
    component.set_content(&library_popup());
    draw(&mut component);
    let rows = component.test_rows().regions().to_vec();

    let (rect, _) = rows[1];
    assert_eq!(component.on(&click(rect.x, rect.y)), None);
    assert_eq!(
        component.on(&click(rect.x, rect.y)),
        Some(Msg::Shell(ShellRequest::LibraryRoutesEnter))
    );
    assert_eq!(component.cursor(), 1);
}

#[test]
fn library_routes_outside_click_follows_the_esc_path() {
    let mut component = LibraryRoutesComponent::new();
    component.set_content(&library_popup());
    draw(&mut component);
    let frame = component.test_frame();
    assert!(frame.x > 0, "popup must be inset for an outside point");

    component.reset_mouse_gestures_for_test();
    assert_eq!(
        component.on(&click(frame.x - 1, frame.y - 1)),
        Some(Msg::Shell(ShellRequest::LibraryRoutesEsc))
    );
}

#[test]
fn library_routes_pick_device_rows_skip_the_info_lines() {
    let mut component = LibraryRoutesComponent::new();
    component.set_content(&LibraryRoutePopup {
        stage: crate::app::types_context_menu::LibraryRouteStage::PickDevice {
            library_lower: "music".into(),
            library_display: "Music".into(),
            devices: vec![(
                "living-room".into(),
                Some(DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap())),
            )],
        },
        cursor: 0,
    });
    draw(&mut component);
    let rows = component.test_rows().regions().to_vec();
    assert_eq!(rows.len(), 2, "Local (no route) plus one device");

    component.reset_mouse_gestures_for_test();
    let (rect, index) = rows[1];
    assert_eq!(component.on(&click(rect.x, rect.y)), None);
    assert_eq!(component.cursor(), index);
    assert_eq!(component.cursor(), 1);
}

#[test]
fn library_routes_keyboard_paths_still_work_alongside_mouse() {
    let mut component = LibraryRoutesComponent::new();
    component.set_content(&library_popup());
    assert_eq!(
        component.on(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::LibraryRoutesEnter))
    );
    assert_eq!(
        component.on(&key(Key::Esc)),
        Some(Msg::Shell(ShellRequest::LibraryRoutesEsc))
    );
}

#[test]
fn library_routes_outside_double_click_emits_nothing_after_the_first_esc() {
    let mut component = LibraryRoutesComponent::new();
    component.set_content(&library_popup());
    draw(&mut component);
    let frame = component.test_frame();
    assert!(frame.x > 0, "popup must be inset for an outside point");

    component.reset_mouse_gestures_for_test();
    assert_eq!(
        component.on(&click(frame.x - 1, frame.y - 1)),
        Some(Msg::Shell(ShellRequest::LibraryRoutesEsc))
    );
    // The double-click arm must not re-fire the Esc path (or anything else):
    // in the real flow the first click already closed the popup.
    assert_eq!(component.on(&click(frame.x - 1, frame.y - 1)), None);
}
