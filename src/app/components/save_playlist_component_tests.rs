use super::msg::{Msg, SavePlaylistIntent, ShellRequest};
use super::save_playlist::SavePlaylistComponent;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

fn key(code: Key) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
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
