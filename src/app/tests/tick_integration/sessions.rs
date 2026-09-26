use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::SessionsComponent;
use crate::app::components::{ComponentId, Msg, OverlayId, ShellRequest, UserEvent};
use crate::app::dispatch::action::Command;
use crate::app::state::panel_targets::{PanelTarget, SessionTargetKey};
use crate::app::tests::tick_integration::harness::{StepOutcome, TickHarness};
use crate::app::tests::{make_app_stub, make_session};

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn sessions(ids: &[&str]) -> Vec<PanelTarget> {
    ids.iter()
        .map(|id| {
            let mut session = make_session(id, "mbv");
            session.id = (*id).into();
            PanelTarget::Emby(Box::new(session))
        })
        .collect()
}

fn apply_messages(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

fn open_sessions(harness: &mut TickHarness) {
    harness.inject(key(Key::Function(3)));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        crate::app::input::router::RouterOutcome::Command(Command::OpenSessions)
    );
    harness
        .model_mut()
        .dispatch_router_command(&Command::OpenSessions);
    apply_messages(harness, outcome);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Overlay(OverlayId::Sessions))
    );
    let (width, height) = (
        harness.model().app.terminal_width,
        harness.model().app.terminal_height,
    );
    draw(harness, width, height);
}

fn sessions_component_mut(harness: &mut TickHarness) -> &mut SessionsComponent {
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Overlay(OverlayId::Sessions))
        .expect("Sessions sidebar mounted")
        .as_any_mut()
        .downcast_mut::<SessionsComponent>()
        .expect("Sessions component")
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
}

fn enter_request(harness: &mut TickHarness) -> SessionTargetKey {
    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    outcome
        .raw_messages
        .iter()
        .find_map(|message| match message {
            Msg::Shell(shell_boxed) => match shell_boxed.as_ref() {
                ShellRequest::SelectSession(key) => Some((*key).clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("Enter emits an identity-bearing activation")
}

#[rstest]
#[case::narrow(42)]
#[case::ordinary(100)]
fn tick_sessions_focus_selection_and_responsive_clamp(#[case] width: u16) {
    let mut app = make_app_stub();
    app.terminal_width = width;
    app.terminal_height = 14;
    app.panel_targets = sessions(&["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7"]);
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);
    draw(&mut harness, width, 14);

    for _ in 0..6 {
        harness.inject(key(Key::Down));
        let outcome = harness.step();
        assert!(outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::TerminalEvent(crate::app::components::TerminalObserverEvent::KeyClaimed)
        )));
    }
    assert_eq!(
        enter_request(&mut harness),
        SessionTargetKey::Emby("s6".into())
    );

    // A resize reprojects the existing selection and clamps its item viewport.
    let resized_width = if width == 42 { 100 } else { 42 };
    draw(&mut harness, resized_width, 14);
    assert_eq!(
        sessions_component_mut(&mut harness).selection_and_offset_for_test(),
        (Some(SessionTargetKey::Emby("s6".into())), 4),
        "selection remains keyed and scroll stays within the list after resize"
    );
}

#[test]
fn tick_sessions_pointer_selects_then_reclick_activates() {
    let mut app = make_app_stub();
    app.terminal_width = 100;
    app.terminal_height = 24;
    app.panel_targets = sessions(&["first", "second"]);
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);
    draw(&mut harness, 100, 24);

    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(ComponentId::Overlay(OverlayId::Sessions)).collect()
    );
    let click = |harness: &mut TickHarness, item: u16| {
        let content = sessions_component_mut(harness)
            .content_area_for_test()
            .expect("painted content geometry");
        let point = ratatui::layout::Position {
            x: content.x + 1,
            y: content.y + item * 3,
        };
        assert_eq!(
            sessions_component_mut(harness).target_at_for_test(point),
            Some(SessionTargetKey::Emby(
                if item == 0 { "first" } else { "second" }.into()
            ))
        );
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: point.x,
            row: point.y,
            modifiers: KeyModifiers::NONE,
        })
    };
    let first_click_event = click(&mut harness, 1);
    harness.inject(first_click_event);
    let first_click = harness.step();
    assert_eq!(
        first_click.pre_fold_focus,
        Some(ComponentId::Overlay(OverlayId::Sessions))
    );
    assert!(first_click
        .messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(_)))));
    assert_eq!(
        sessions_component_mut(&mut harness)
            .selection_and_offset_for_test()
            .0,
        Some(SessionTargetKey::Emby("second".into())),
        "first click raw messages: {:?}",
        first_click.raw_messages
    );

    sessions_component_mut(&mut harness).reset_mouse_gestures_for_test();
    draw(&mut harness, 100, 24);
    let second_click_event = click(&mut harness, 1);
    harness.inject(second_click_event);
    let second_click = harness.step();
    assert!(second_click.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(SessionTargetKey::Emby(id)) if id == "second"))));
}

#[test]
fn tick_sessions_changed_snapshot_invalidates_stale_hits_until_repaint() {
    let mut app = make_app_stub();
    app.panel_targets = sessions(&["removed", "kept"]);
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let old_point = {
        let content = sessions_component_mut(&mut harness)
            .content_area_for_test()
            .expect("painted content geometry");
        ratatui::layout::Position::new(content.x + 1, content.y)
    };
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(old_point),
        Some(SessionTargetKey::Emby("removed".into()))
    );

    harness.model_mut().app.panel_targets = sessions(&["replacement", "kept"]);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(old_point),
        None,
        "changed target keys invalidate the previously painted hit before repaint"
    );

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: old_point.x,
        row: old_point.y,
        modifiers: KeyModifiers::NONE,
    }));
    let stale_click = harness.step();
    assert!(stale_click
        .raw_messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(_)))));

    draw(&mut harness, 100, 24);
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(old_point),
        Some(SessionTargetKey::Emby("replacement".into())),
        "repaint publishes geometry for the new snapshot"
    );
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: old_point.x,
        row: old_point.y,
        modifiers: KeyModifiers::NONE,
    }));
    let repainted_click = harness.step();
    assert!(repainted_click.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(SessionTargetKey::Emby(id)) if id == "replacement"))));
}

#[test]
fn tick_sessions_unchanged_sync_keeps_painted_pointer_target_without_redraw() {
    let mut app = make_app_stub();
    app.panel_targets = sessions(&["first", "second"]);
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let point = {
        let content = sessions_component_mut(&mut harness)
            .content_area_for_test()
            .expect("painted content geometry");
        ratatui::layout::Position::new(content.x + 1, content.y + 3)
    };
    let second = SessionTargetKey::Emby("second".into());
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(point),
        Some(second.clone())
    );

    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        sessions_component_mut(&mut harness).target_at_for_test(point),
        Some(second.clone()),
        "an unchanged shell sync must retain the last painted hit"
    );
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: point.x,
        row: point.y,
        modifiers: KeyModifiers::NONE,
    }));
    let click = harness.step();
    assert_eq!(
        sessions_component_mut(&mut harness)
            .selection_and_offset_for_test()
            .0,
        Some(second)
    );
    assert!(click
        .raw_messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::SelectSession(_)))));
}

#[test]
fn tick_sessions_reordered_snapshot_activates_selected_identity() {
    let mut app = make_app_stub();
    app.panel_targets = sessions(&["first", "selected"]);
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    harness.inject(key(Key::Down));
    harness.step();
    harness.model_mut().app.panel_targets = sessions(&["selected", "first"]);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        sessions_component_mut(&mut harness)
            .selection_and_offset_for_test()
            .0,
        Some(SessionTargetKey::Emby("selected".into()))
    );

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("selected".into()));
    harness.model_mut().app.panel_targets = sessions(&["selected", "first"]);
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().app.connected_session_id.as_deref(),
        Some("selected")
    );
}

#[test]
fn tick_sessions_removed_target_request_is_noop_not_former_index_replacement() {
    let mut app = make_app_stub();
    app.panel_targets = sessions(&["selected", "replacement"]);
    let mut harness = TickHarness::new(app);
    open_sessions(&mut harness);

    let request = enter_request(&mut harness);
    assert_eq!(request, SessionTargetKey::Emby("selected".into()));
    harness.model_mut().app.panel_targets = sessions(&["replacement"]);
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::SelectSession(request))),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(harness.model().app.connected_session_id.is_none());
    assert!(!harness.model().app.is_cast_attached());
}

#[test]
fn tick_sessions_selection_delivers_through_focused_tick_after_f3() {
    let mut harness = TickHarness::new(make_app_stub());
    open_sessions(&mut harness);
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    assert_eq!(
        outcome.pre_fold_focus,
        Some(ComponentId::Overlay(OverlayId::Sessions))
    );
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(crate::app::components::TerminalObserverEvent::KeyClaimed)
    )));
}
