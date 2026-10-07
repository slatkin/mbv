//! Configured keybinds route through the live shell (task 3.3).
//!
//! The compiled `[keys]` configuration is read once at `Model` construction;
//! this file proves a loaded router-scope override fires through the real
//! `Application::tick()` path (inject port → `UiRoot` observer → router fold),
//! not just through the pure policy seam.

use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::input::router::RouterOutcome;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{
    QueueViewTestExt, make_app_stub, make_item, make_items, make_local_daemon_app_stub_with_cmd_rx,
};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection};
use mbv_ui_model::browse::ServerRows;
use mbv_ui_msg::{OverlayId, UserEvent};

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
        keybinds: mbv_keybinds::load(&mbv_keybinds::RawKeybinds {
            prefix: None,
            sections: vec![(
                "global".into(),
                mbv_keybinds::RawSection {
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
        RouterOutcome::Command(crate::app::dispatch::action::Command::OpenHelp),
        "the configured chord fires the rebound action through tick()"
    );
    harness
        .model_mut()
        .dispatch_router_command(&crate::app::dispatch::action::Command::OpenHelp);
    assert!(
        harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Overlay(OverlayId::Help)),
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
    assert!(
        !harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Overlay(OverlayId::Help))
    );
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
        RouterOutcome::Command(crate::app::dispatch::action::Command::PreviousLibraryTab),
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
        keybinds: mbv_keybinds::load(&mbv_keybinds::RawKeybinds {
            prefix: None,
            sections: vec![(
                "playback".into(),
                mbv_keybinds::RawSection {
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
        RouterOutcome::Command(crate::app::dispatch::action::Command::AdjustVolume(5)),
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
    app.player.update_status(|status| status.active = true);
    let mut harness = TickHarness::new(app);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('N'),
        modifiers: KeyModifiers::SHIFT,
    }));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(crate::app::dispatch::action::Command::NextTrack),
        "Shift+N must fire next_track through tick()"
    );
}

/// #745: F5 is a global data refresh whose effect comes from the selected
/// browse destination, not Panel focus. Driven through the live tick path
/// (the harness mirrors `shell_run` by dispatching the resolved command):
/// with Queue focused and an Emby library selected, the selected library is
/// the refresh target and no owner Queue refresh is sent.
#[test]
fn f5_with_queue_focus_refreshes_the_selected_library_through_tick() {
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(2));
    while cmd_rx.try_recv().is_ok() {}
    // A non-empty viewed queue makes the removed Queue-refresh branch
    // observable: that branch would have sent UnifiedQueueRefresh here.
    app.local_view.adopt_items(make_items(2), 0);
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::new(1),
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: make_items(1),
            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Queue;
    let mut harness = TickHarness::new(app);

    harness.inject(key(Key::Function(5)));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(crate::app::dispatch::action::Command::RefreshCurrentView),
        "F5 resolves the global refresh command under Queue focus"
    );
    // Drop anything the tick's own sync emitted; the assertion below is about
    // the dispatched refresh only.
    while cmd_rx.try_recv().is_ok() {}
    harness
        .model_mut()
        .dispatch_router_command(&crate::app::dispatch::action::Command::RefreshCurrentView);

    assert!(
        harness.model().app.libs[0].nav_stack[0].loading,
        "the selected Emby library is the refresh target under Queue focus"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "F5 must not send an owner Queue refresh"
    );
}
