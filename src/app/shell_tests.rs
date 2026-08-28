use super::*;
use crate::app::components::{OverlayId, TvWorkspaceComponent};
use crate::app::images::CachedImage;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_app_stub;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuirealm::component::AppComponent;
use tuirealm::event::{
    Event, Key as TuiKey, KeyEvent as TuiKeyEvent, KeyModifiers as TuiKeyModifiers,
};

#[test]
fn ui_root_router_command_opens_help() {
    let mut model = Model::new(make_app_stub());
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key))];
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
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key))];

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
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key))];

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
        vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key))],
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
    let mut quit = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::Resize,
        Some(&ComponentId::Playback),
        &mut music_resize,
        &mut tv_resize,
        &mut quit,
    );
    assert!(model.app.force_clear);
    assert!(model.app.card_image_states.is_empty());
    assert!(model.app.card_image_loading.is_empty());
    assert!(music_resize && tv_resize);
    assert!(!quit);
}

#[test]
fn terminal_focus_observer_preserves_refocus_side_effects() {
    let mut model = Model::new(make_app_stub());
    let mut music_resize = false;
    let mut tv_resize = false;
    let mut quit = false;
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusGained,
        Some(&ComponentId::Playback),
        &mut music_resize,
        &mut tv_resize,
        &mut quit,
    );
    assert!(model.app.refocus_at.is_some());
    apply_terminal_observer(
        &mut model,
        TerminalObserverEvent::FocusLost,
        Some(&ComponentId::Playback),
        &mut music_resize,
        &mut tv_resize,
        &mut quit,
    );
    assert!(model.app.refocus_at.is_none());
    assert!(!quit);
}

#[test]
fn global_view_key_from_media_surface_is_claimed_by_router() {
    let mut surface = TvWorkspaceComponent::new();
    surface.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
        None,
        None,
        0,
        None,
        true,
        false,
    ));
    let Some(Msg::Shell(ShellRequest::GlobalViewKey(key))) =
        surface.on(&Event::Keyboard(TuiKeyEvent {
            code: TuiKey::Function(1),
            modifiers: TuiKeyModifiers::NONE,
        }))
    else {
        panic!("unmatched media-surface key did not use the global adapter");
    };

    let mut model = Model::new(make_app_stub());
    let messages = vec![
        Msg::Shell(ShellRequest::GlobalViewKey(key)),
        Msg::TerminalEvent(TerminalObserverEvent::Key(key)),
    ];
    let outcome = model.router_outcome(&messages);
    assert_eq!(outcome, RouterOutcome::Command(Command::OpenHelp));
    assert!(apply_router_outcome(messages, Some(&ComponentId::Playback), &outcome).is_empty());
    assert!(!model.dispatch_router_command(Command::OpenHelp));
    assert!(model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::Help)));
}
