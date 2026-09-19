use super::*;
use crate::app::images::CachedImage;
use crate::app::tests::make_app_stub;
use crate::app::PanelFocus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn loaded_keys_override_reaches_the_model() {
    // The compiled `[keys]` configuration is read once, at Model
    // construction, from the config the App was built with (U2 row 2.3).
    let app = make_app_stub();
    let config = crate::config::Config {
        keybinds: mbv_core::keybinds::load(&mbv_core::keybinds::RawKeybinds {
            prefix: Some("Ctrl+b".into()),
            sections: vec![(
                "global".into(),
                mbv_core::keybinds::RawSection {
                    router: vec![("help_open".into(), "F9".into())],
                    prefix: vec![],
                },
            )],
        })
        .unwrap(),
        ..Default::default()
    };
    *app.config.lock().unwrap() = config;

    let model = Model::new(app);
    assert_eq!(
        model.keybinds.prefix,
        Some(mbv_core::keybinds::Chord::parse("Ctrl+b").unwrap())
    );
    assert_eq!(
        model.keybinds.router_override("help_open"),
        Some(mbv_core::keybinds::Chord::parse("F9").unwrap())
    );
    assert!(model.keybinds.router_override("settings_open").is_none());
}

#[test]
fn model_without_keys_configuration_holds_default_keybinds() {
    let model = Model::new(make_app_stub());
    assert_eq!(model.keybinds, mbv_core::keybinds::Keybinds::default());
}

#[test]
fn ui_root_router_command_opens_help() {
    let mut model = Model::new(make_app_stub());
    let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Command(Command::OpenHelp)
    );
    assert!(arbitrate_key(
        messages,
        Some(&ComponentId::UiRoot),
        &RouterOutcome::Command(Command::OpenHelp)
    )
    .0
    .is_empty());
    assert!(!model.dispatch_router_command(Command::OpenHelp));
    assert!(model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::Help)));
}

#[test]
fn unhandled_space_fires_the_playback_candidate_once() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::TogglePlayPause)
    );
    // No candidate timing state exists: an unhandled press fires on that
    // press, and a later unhandled press fires again as a fresh press.
    assert!(
        model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), false)
    );
    assert!(
        model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), false)
    );
}

#[test]
fn consumed_space_cancels_the_playback_candidate_and_leaves_no_state() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    // A focused media list holding an active Visual selection consumes Space
    // for its row-local toggle; the shell sees the leaf's consumption only.
    model.visual_selection = Some((PanelFocus::Library, 2));
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE).into(),
    ))];

    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::TogglePlayPause)
    );
    assert!(
        !model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), true)
    );
    // A later unhandled press still behaves as a first press: no consumed
    // state is inherited.
    assert!(
        model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::TogglePlayPause), false)
    );
}

/// The double-Esc stop (see `Model::router_outcome`): the first Esc falls
/// through to its claimants and dispatches nothing; a second Esc inside
/// [`DOUBLE_ESC_STOP_WINDOW`] resolves the stop candidate. A press on any
/// other key disarms, so Esc-then-Space stays a play/pause toggle.
#[test]
fn first_escape_falls_through_and_second_dispatches_the_stop_candidate() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    // First press: falls through, no candidate, nothing to dispatch.
    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
    // Second press inside the window: the stop candidate, which an unhandled
    // press dispatches.
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::Stop)
    );
    assert!(model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::Stop), false));

    // A non-Esc press between the two disarms: Space stays play/pause.
    let space = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE).into(),
    ))];
    model.router_outcome(&space);
    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
}

#[test]
fn consumed_escape_cancels_the_stop_candidate_and_leaves_no_state() {
    let mut model = Model::new(make_app_stub());
    model.app.player.status.lock().unwrap().active = true;
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let messages = vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))];

    // The first Esc arms the double-Esc window without a candidate.
    assert_eq!(model.router_outcome(&messages), RouterOutcome::FallThrough);
    assert_eq!(
        model.router_outcome(&messages),
        RouterOutcome::Deferred(Command::Stop)
    );
    // A leaf that consumed Esc (dismiss/back claims first) cancels the
    // candidate and records nothing.
    assert!(!model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::Stop), true));
    // A later unhandled press still behaves as a first press.
    assert!(model.apply_deferred_candidate(&RouterOutcome::Deferred(Command::Stop), false));
}

#[test]
fn converted_surface_skips_observer_key_but_retains_redraw_signal() {
    let focused = ComponentId::LibraryPlaybackPanel;
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    // Leaf focused, empty policy: the fold drops the observer's Key trigger
    // (the leaf already got the event) but keeps non-key observer signals.
    let router = RouterOutcome::FallThrough;
    let routed = fold_keyboard_messages(
        vec![Msg::TerminalEvent(TerminalObserverEvent::Key(key.into()))],
        Some(&focused),
        &router,
    );
    assert!(routed.is_empty());
    let routed = fold_keyboard_messages(
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
