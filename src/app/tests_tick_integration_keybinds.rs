//! Configured keybinds route through the live shell (task 3.3).
//!
//! The compiled `[keys]` configuration is read once at `Model` construction;
//! this file proves a loaded router-scope override fires through the real
//! `Application::tick()` path (inject port → UiRoot observer → router fold),
//! not just through the pure policy seam.

use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::components::{OverlayId, UserEvent};
use crate::app::router::RouterOutcome;
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::TickHarness;

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

/// A configured rebind fires through `Application::tick()`: `help_open`
/// rebound from F1 to F9 opens Help on F9, and the declared default F1 is
/// inert.
#[test]
fn configured_rebind_fires_through_tick() {
    let app = make_app_stub();
    let config = crate::config::Config {
        keybinds: mbv_core::keybinds::load(&mbv_core::keybinds::RawKeybinds {
            prefix: None,
            sections: vec![(
                "global".into(),
                mbv_core::keybinds::RawSection {
                    router: vec![("help_open".into(), "F9".into())],
                    prefix: vec![],
                },
            )],
        })
        .expect("valid keys configuration"),
        ..Default::default()
    };
    *app.config.lock().unwrap() = config.clone();
    let mut harness = TickHarness::new(app);

    // The configured chord opens Help through the live tick path. The
    // harness mirrors the run loop: `Command` outcomes are dispatched by the
    // caller (shell_run), so the test dispatches the resolved command.
    harness.inject(key(Key::Function(9)));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(crate::app::action::Command::OpenHelp),
        "the configured chord fires the rebound action through tick()"
    );
    harness
        .model_mut()
        .dispatch_router_command(crate::app::action::Command::OpenHelp);
    assert!(
        harness.model().application.mounted(&crate::app::components::ComponentId::Overlay(
            OverlayId::Help
        )),
        "Help must be mounted after the configured chord"
    );

    // A fresh model with the same configuration: the declared default is
    // inert (F1 reaches no policy layer and mounts nothing).
    let app = make_app_stub();
    *app.config.lock().unwrap() = config.clone();
    let mut harness = TickHarness::new(app);
    harness.inject(key(Key::Function(1)));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::FallThrough,
        "the declared default of a rebound action is inert"
    );
    assert!(!harness
        .model()
        .application
        .mounted(&crate::app::components::ComponentId::Overlay(
            OverlayId::Help
        )));
}

/// Crossterm delivers Shift+Tab as `BackTab` with SHIFT set; the default
/// `previous_library_tab` binding is the bare `BackTab` chord. The pressed
/// chord must still fire through the live tick path (P1 regression).
#[test]
fn shift_tab_backtab_shift_encoding_fires_previous_library_tab_through_tick() {
    let app = make_app_stub();
    let mut harness = TickHarness::new(app);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::BackTab,
        modifiers: KeyModifiers::SHIFT,
    }));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(crate::app::action::Command::PreviousLibraryTab),
        "Shift+Tab must fire previous_library_tab through tick()"
    );
}

/// Task 5.1: a rebound transport action fires through the live tick path on
/// its configured chord, and its declared default becomes inert. `volume_up`
/// is ungated, so no player state is needed.
#[test]
fn rebound_transport_action_fires_through_tick_and_default_is_inert() {
    let app = make_app_stub();
    let config = crate::config::Config {
        keybinds: mbv_core::keybinds::load(&mbv_core::keybinds::RawKeybinds {
            prefix: None,
            sections: vec![(
                "playback".into(),
                mbv_core::keybinds::RawSection {
                    router: vec![("volume_up".into(), "k".into())],
                    prefix: vec![],
                },
            )],
        })
        .expect("valid keys configuration"),
        ..Default::default()
    };
    *app.config.lock().unwrap() = config.clone();
    let mut harness = TickHarness::new(app);

    harness.inject(key(Key::Char('k')));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(crate::app::action::Command::AdjustVolume(5)),
        "the configured chord fires the rebound transport action through tick()"
    );

    // The declared default `+` (and its `=` alias) no longer resolves.
    let app = make_app_stub();
    *app.config.lock().unwrap() = config;
    let mut harness = TickHarness::new(app);
    harness.inject(key(Key::Char('+')));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::FallThrough,
        "the declared default of a rebound transport action is inert"
    );
}

/// Task 5.1: the crossterm-case default for `next_track` — Shift+n is
/// delivered as `Char('N')` + SHIFT — fires through the live tick path with
/// an active player.
#[test]
fn shift_n_default_fires_next_track_through_tick() {
    let app = make_app_stub();
    app.player
        .status
        .lock()
        .unwrap()
        .active = true;
    let mut harness = TickHarness::new(app);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('N'),
        modifiers: KeyModifiers::SHIFT,
    }));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(crate::app::action::Command::NextTrack),
        "Shift+N must fire next_track through tick()"
    );
}
