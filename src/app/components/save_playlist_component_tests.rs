use super::msg::{Msg, SavePlaylistIntent, ShellRequest};
use super::save_playlist::SavePlaylistComponent;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn click(x: u16, y: u16) -> Event<super::user_event::UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

fn draw(component: &mut SavePlaylistComponent) {
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
}

#[test]
fn save_playlist_key_updates_local_input_and_emits_semantic_request() {
    let mut component = SavePlaylistComponent::new();
    component.set_content("Old".into(), false);

    assert_eq!(component.on(&Event::Keyboard(key(Key::Char('!')))), None);
    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Enter))),
        Some(Msg::Shell(ShellRequest::SavePlaylistIntent(
            SavePlaylistIntent::Submit
        )))
    ));

    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(output.contains("Name: Old!"));
}

#[test]
fn save_playlist_render_uses_rename_title_without_app_state() {
    let mut component = SavePlaylistComponent::new();
    component.set_content("Playlist".into(), true);
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();

    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(output.contains("Rename Playlist"));
    assert!(output.contains("Name: Playlist"));
}

#[test]
fn save_playlist_outside_click_dismisses_like_esc() {
    let mut component = SavePlaylistComponent::new();
    component.set_content("Draft".into(), false);
    draw(&mut component);
    let frame = component.test_frame();
    assert!(frame.x > 0, "modal must be inset for an outside point");

    component.reset_mouse_gestures_for_test();
    assert_eq!(
        component.on(&click(frame.x - 1, frame.y - 1)),
        Some(Msg::Shell(ShellRequest::SavePlaylistIntent(
            SavePlaylistIntent::Dismiss
        )))
    );
}

#[test]
fn save_playlist_inside_click_is_a_noop() {
    let mut component = SavePlaylistComponent::new();
    component.set_content("Draft".into(), false);
    draw(&mut component);
    let frame = component.test_frame();

    // The single always-focused name input has no focus/select keyboard
    // path, so an inside click (including on the input row) does nothing:
    // no input mutation, no Msg.
    component.reset_mouse_gestures_for_test();
    assert_eq!(component.on(&click(frame.x + 1, frame.y + 1)), None);
    assert_eq!(component.input(), "Draft");
    component.reset_mouse_gestures_for_test();
    assert_eq!(component.on(&click(frame.x, frame.y)), None);
    assert_eq!(component.input(), "Draft");
}
