use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn mouse_down(column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn painted_settings(destination: SettingsDestination) -> SettingsComponent {
    let mut component = SettingsComponent::new();
    let (rows, services): (Vec<SettingsRow>, Vec<ServiceRow>) = match destination {
        SettingsDestination::Services => (
            Vec::new(),
            vec![
                ServiceRow {
                    name: "Emby".into(),
                    detail: "Connected".into(),
                    muted: false,
                },
                ServiceRow {
                    name: "Audiobookshelf".into(),
                    detail: "Not configured".into(),
                    muted: false,
                },
            ],
        ),
        _ => (
            vec![
                SettingsRow {
                    label: "Section".into(),
                    value: String::new(),
                    section: true,
                    cursor: None,
                },
                SettingsRow {
                    label: "Stay alive".into(),
                    value: "off".into(),
                    section: false,
                    cursor: Some(0),
                },
            ],
            Vec::new(),
        ),
    };
    component.set_content(SettingsSnapshot {
        destination,
        rows,
        services,
        keys: Vec::new(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component
}

mod keys_navigation;
mod mouse_and_setup;
