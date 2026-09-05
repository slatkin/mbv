use super::*;
use crate::app::images::CachedImage;
use crate::app::tests::make_app_stub;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn ui_root_router_command_opens_help() {
    let mut model = Model::new(make_app_stub());
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Command(Command::OpenHelp)
    );
    assert!(apply_router_outcome(
        messages,
        Some(&ComponentId::UiRoot),
        &RouterOutcome::Command(Command::OpenHelp)
    )
    .is_empty());
    assert!(!model.dispatch_router_command(Command::OpenHelp));
    assert!(model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::Help)));
}

#[test]
fn router_records_first_space_for_second_claim() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    model.application.active(&ComponentId::Playback).unwrap();
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
    assert!(model.app.last_space_press.is_some());
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Command(Command::TogglePlayPause)
    );
    assert!(model.app.last_space_press.is_none());
}

#[test]
fn router_records_first_esc_for_second_claim() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    model.application.active(&ComponentId::Playback).unwrap();
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
    assert!(model.app.last_esc_press.is_some());
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Command(Command::Stop)
    );
    assert!(model.app.last_esc_press.is_none());
}

#[test]
fn converted_surface_skips_observer_key_but_retains_redraw_signal() {
    let focused = ComponentId::Playback;
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    // Leaf focused, empty policy: the fold drops the observer's Key trigger
    // (the leaf already got the event) but keeps non-key observer signals.
    let router = RouterOutcome::FallThrough;
    let routed = apply_router_outcome(
        vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))],
        Some(&focused),
        &router,
    );
    assert!(routed.is_empty());
    let routed = apply_router_outcome(
        vec![Msg::TerminalEvent(TerminalObserverEvent::NoOp)],
        Some(&focused),
        &router,
    );
    assert!(matches!(
        routed.as_slice(),
        [Msg::TerminalEvent(TerminalObserverEvent::NoOp)]
    ));
}

#[test]
fn terminal_resize_observer_preserves_layout_side_effects() {
    let mut model = Model::new(make_app_stub());
    model.app.force_clear = false;
    model
        .app
        .card_image_states
        .insert("stale".into(), CachedImage::empty());
    model.app.card_image_loading.insert("stale".into());
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::Resize {
            width: 80,
            height: 24,
        },
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.force_clear);
    assert!(model.app.card_image_states.is_empty());
    assert!(model.app.card_image_loading.is_empty());
    assert!(music_resize && tv_resize);
}

#[test]
fn terminal_resize_observer_applies_new_size_before_paint() {
    let mut model = Model::new(make_app_stub());
    model.app.terminal_width = 60;
    model.app.terminal_height = 24;
    assert!(!model.app.is_right_panel_wide());
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::Resize {
            width: 150,
            height: 24,
        },
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.terminal_width, 150);
    assert_eq!(model.app.terminal_height, 24);
    assert!(model.app.is_right_panel_wide());
}

#[test]
fn terminal_focus_observer_preserves_refocus_side_effects() {
    let mut model = Model::new(make_app_stub());
    let mut music_resize = false;
    let mut tv_resize = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusGained,
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.refocus_at.is_some());
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusLost,
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.refocus_at.is_none());
}
