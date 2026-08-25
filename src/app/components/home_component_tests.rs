use super::home::HomeComponent;
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use mbv_core::playback_queue::QueueItem;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[test]
fn home_down_moves_the_component_cursor_without_app_state() {
    let mut home = HomeComponent::new();
    home.set_focused(true);
    home.set_content(
        vec![QueueItem::Emby(Box::new(crate::app::tests::make_item(
            "one", "Movie",
        )))],
        vec![(
            "Movies".into(),
            crate::app::types_playback::HomeLatestSource::Emby("movies".into()),
            vec![QueueItem::Emby(Box::new(crate::app::tests::make_item(
                "two", "Movie",
            )))],
        )],
        false,
    );

    let msg = home.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(
        home.cursor(),
        0,
        "Home movement stays within the selected section"
    );
    assert_eq!(home.section(), 0);
    assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
}

fn two_section_home() -> HomeComponent {
    let mut home = HomeComponent::new();
    // Home keyboard ownership requires the Library panel to be focused; the
    // keyboard tests below exercise that focused state.
    home.set_focused(true);
    home.set_content(
        vec![
            QueueItem::Emby(Box::new(crate::app::tests::make_item("cw1", "Movie"))),
            QueueItem::Emby(Box::new(crate::app::tests::make_item("cw2", "Movie"))),
        ],
        vec![(
            "Movies".into(),
            crate::app::types_playback::HomeLatestSource::Emby("movies".into()),
            vec![QueueItem::Emby(Box::new(crate::app::tests::make_item(
                "latest1", "Movie",
            )))],
        )],
        false,
    );
    home
}

fn key(code: Key) -> Event<crate::app::components::UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn home_keys_fall_through_while_the_queue_panel_is_focused() {
    // Unfocused (Queue panel focused): Home must not claim or mutate
    // anything — every key falls through to the legacy dispatch, where the
    // queue handler owns it. Local navigation and typed effects alike.
    let mut home = two_section_home();
    home.set_focused(false);

    let msg = home.on(&key(Key::Down));
    assert!(matches!(
        msg,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(k)))
            if k.code == crossterm::event::KeyCode::Down
    ));
    assert_eq!(
        home.cursor(),
        0,
        "queue-focused Down must not move Home's cursor"
    );

    let msg = home.on(&key(Key::Char(']')));
    assert!(matches!(
        msg,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(k)))
            if k.code == crossterm::event::KeyCode::Char(']')
    ));
    assert_eq!(
        home.section(),
        0,
        "queue-focused ] must not move Home's pill"
    );

    let msg = home.on(&key(Key::Enter));
    assert!(matches!(
        msg,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(k)))
            if k.code == crossterm::event::KeyCode::Enter
    ));
    assert_eq!(home.cursor(), 0, "queue-focused Enter must not act on Home");
}

#[test]
fn enter_emits_typed_play_at_the_flat_cursor() {
    let mut home = two_section_home();
    home.on(&key(Key::Down));
    let msg = home.on(&key(Key::Enter));
    assert_eq!(msg, Some(Msg::Shell(ShellRequest::HomePlay(1))));
}

#[test]
fn delete_emits_typed_remove_at_the_flat_cursor() {
    let mut home = two_section_home();
    let msg = home.on(&key(Key::Delete));
    assert_eq!(msg, Some(Msg::Shell(ShellRequest::HomeDelete(0))));
}

#[test]
fn section_bracket_moves_into_the_next_section_and_persists() {
    let mut home = two_section_home();
    let msg = home.on(&key(Key::Char(']')));
    assert_eq!(home.section(), 1);
    assert_eq!(home.cursor(), 2, "cursor lands in the new section's range");
    assert_eq!(msg, Some(Msg::Shell(ShellRequest::HomeSectionSelected(1))));
}

#[test]
fn unmatched_key_bounces_to_the_legacy_dispatch() {
    let mut home = two_section_home();
    let msg = home.on(&key(Key::Char('v')));
    assert!(matches!(
        msg,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(k))) if k.code == crossterm::event::KeyCode::Char('v')
    ));
}

#[test]
fn ctrl_w_emits_toggle_watched_without_a_cursor_payload() {
    let mut home = two_section_home();
    let msg = home.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::CONTROL,
    }));
    assert_eq!(msg, Some(Msg::Shell(ShellRequest::HomeToggleWatched)));
}

#[test]
fn home_renders_content_without_app_state() {
    let mut home = two_section_home();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();

    terminal
        .draw(|frame| home.view(frame, frame.area()))
        .unwrap();

    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(output.contains("cw1"));
}

#[test]
fn home_right_click_uses_the_rendered_row_target() {
    let mut home = two_section_home();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| home.view(frame, frame.area()))
        .unwrap();

    // Right-click on a rendered row resolves the painted target and moves
    // the component-local cursor to it, so the emitted `ContextMenu` region
    // and `home.cursor()` agree on the row under the click.
    let (rect, target) = home.test_hitmap()[1];
    let row_message = home.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        home.cursor(),
        target,
        "right-click moves the local cursor to the painted row"
    );
    assert!(
        matches!(row_message, Some(Msg::Shell(ShellRequest::HomeClick {
        region: super::msg::HomeHitRegion::ContextMenu(index), ..
    })) if index == target)
    );

    // A right-click on rendered blank space inside the list (the rows below
    // the last painted hitmap row) opens the menu at the current cursor and
    // leaves the cursor unchanged.
    let cursor_before = home.cursor();
    let blank_y = home
        .test_hitmap()
        .iter()
        .map(|(r, _)| r.bottom())
        .max()
        .unwrap();
    let blank_message = home.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 0,
        row: blank_y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        home.cursor(),
        cursor_before,
        "blank-space right-click leaves the cursor unchanged"
    );
    assert!(
        matches!(blank_message, Some(Msg::Shell(ShellRequest::HomeClick {
        region: super::msg::HomeHitRegion::ContextMenu(index), ..
    })) if index == cursor_before)
    );
}
