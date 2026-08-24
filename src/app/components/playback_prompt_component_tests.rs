use super::msg::{Msg, ShellRequest};
use super::playback_prompt::PlaybackPromptComponent;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
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
fn prompt_keys_are_typed_shell_requests() {
    let mut component = PlaybackPromptComponent::new();

    assert!(matches!(
        component.on(&Event::Keyboard(key(Key::Enter))),
        Some(Msg::Shell(ShellRequest::PlaybackPromptKey(_)))
    ));
}

#[test]
fn prompt_renders_without_app_state() {
    let mut component = PlaybackPromptComponent::new();
    component.set_content("Skip intro? (Y/n)", true, Rect::new(5, 2, 30, 1));
    let mut terminal = Terminal::new(TestBackend::new(40, 5)).unwrap();

    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(output.contains("Skip intro? (Y/n)"));
}
