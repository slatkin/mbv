//! Mouse tests for `MultiselectComponent` (task 5.1): row click toggles the
//! item under the cursor (Space), row double-click commits (Enter), and an
//! outside click commits exactly like Esc — this popup's only dismiss path
//! *is* a commit.

use super::msg::{Msg, ShellRequest};
use super::multiselect::MultiselectComponent;
use crate::app::types_context_menu::{MultiSelectKind, MultiSelectPopup};
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

fn draw(component: &mut MultiselectComponent) {
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
}

fn popup() -> MultiSelectPopup {
    MultiSelectPopup {
        kind: MultiSelectKind::HiddenLibraries,
        items: vec![
            ("movies".into(), "Movies".into(), false),
            ("shows".into(), "Shows".into(), true),
        ],
        cursor: 0,
    }
}

#[test]
fn multiselect_row_click_toggles_like_space_and_moves_the_cursor() {
    let mut component = MultiselectComponent::new();
    component.set_content(&popup());
    draw(&mut component);
    let rows = component.test_rows().regions().to_vec();
    assert_eq!(rows.len(), 2);

    component.reset_mouse_gestures_for_test();
    let (rect, index) = rows[0];
    let msg = component.on(&click(rect.x, rect.y));
    assert_eq!(msg, None, "toggling is a local mutation, no Msg");
    assert_eq!(component.test_cursor(), index);
    assert!(
        component.test_items()[0].2,
        "Movies toggled from false to true"
    );

    // Second row, separately: Shows toggles from checked back to unchecked.
    component.reset_mouse_gestures_for_test();
    let (rect, index) = rows[1];
    component.on(&click(rect.x, rect.y));
    assert_eq!(component.test_cursor(), index);
    assert!(!component.test_items()[1].2);
}

#[test]
fn multiselect_row_double_click_commits_like_enter() {
    let mut component = MultiselectComponent::new();
    component.set_content(&popup());
    draw(&mut component);
    let (rect, _) = component.test_rows().regions()[0];

    // First click toggles; the second (double-click) commits.
    assert_eq!(component.on(&click(rect.x, rect.y)), None);
    assert!(
        matches!(
            component.on(&click(rect.x, rect.y)),
            Some(Msg::Shell(ShellRequest::MultiselectCommit { kind, items }))
                if kind == MultiSelectKind::HiddenLibraries && items.len() == 2
        ),
        "double-click must emit the Enter-equivalent commit"
    );
}

#[test]
fn multiselect_outside_click_commits_like_esc() {
    let mut component = MultiselectComponent::new();
    component.set_content(&popup());
    draw(&mut component);
    let frame = component.test_frame();
    assert!(frame.x > 0);

    component.reset_mouse_gestures_for_test();
    assert!(
        matches!(
            component.on(&click(frame.x - 1, frame.y - 1)),
            Some(Msg::Shell(ShellRequest::MultiselectCommit { .. }))
        ),
        "outside click must follow the popup's only dismiss path, which is a commit"
    );
    // The single click must not have toggled anything: the first commit is
    // the click itself.
    assert!(!component.test_items()[0].2);
    assert!(component.test_items()[1].2);
}

#[test]
fn multiselect_inside_click_off_the_rows_is_a_noop() {
    let mut component = MultiselectComponent::new();
    component.set_content(&popup());
    draw(&mut component);
    let frame = component.test_frame();

    component.reset_mouse_gestures_for_test();
    // The hint row at the top of the frame is painted chrome with no
    // keyboard equivalent to mirror.
    assert_eq!(component.on(&click(frame.x, frame.y)), None);
    assert!(!component.test_items()[0].2);
}

#[test]
fn multiselect_keyboard_paths_still_work_alongside_mouse() {
    let mut component = MultiselectComponent::new();
    component.set_content(&popup());
    assert_eq!(component.on(&key(Key::Char(' '))), None);
    assert!(component.test_items()[0].2);
    assert!(matches!(
        component.on(&key(Key::Esc)),
        Some(Msg::Shell(ShellRequest::MultiselectCommit { .. }))
    ));
}

#[test]
fn multiselect_outside_double_click_does_not_commit_twice() {
    let mut component = MultiselectComponent::new();
    component.set_content(&popup());
    draw(&mut component);
    let frame = component.test_frame();
    assert!(frame.x > 0);

    component.reset_mouse_gestures_for_test();
    assert!(matches!(
        component.on(&click(frame.x - 1, frame.y - 1)),
        Some(Msg::Shell(ShellRequest::MultiselectCommit { .. }))
    ));
    // The double-click arm must not produce a second MultiselectCommit: in
    // the real flow the first click already closed the popup.
    assert_eq!(component.on(&click(frame.x - 1, frame.y - 1)), None);
}
