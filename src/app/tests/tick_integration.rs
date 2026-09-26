use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::application::PollStrategy;
use tuirealm::component::AppComponent;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::msg::{ConfirmIntent, PlaybackRequest, ServiceRequest};
use crate::app::components::{
    ComponentId, ModalId, Msg, OverlayId, QueueRequest, SearchSidebarComponent, ShellRequest,
    TerminalObserverEvent, UserEvent,
};
use crate::app::dispatch::action::Command;
use crate::app::input::router::RouterOutcome;
use crate::app::shell::fold_keyboard_messages;
use crate::app::state::types::confirm::{ConfirmAction, ConfirmModal};
use crate::app::state::types::overlay::OverlayRequest;
use crate::app::tests::make_app_stub;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::{PanelFocus, PanelMode, SidebarId, TabSelection};

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

/// The Ctrl chord of a key: the panel-focus switch (user decision 2026-09-20).
fn ctrl_key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::CONTROL,
    })
}

/// Task 1.1: the saved-tab restore runs in the sync pass, before any draw.
/// After one tick() + sync pass and without drawing, the pending tab is
/// resolved; `render_main` no longer writes `self.tab`.
#[test]
fn sync_pass_resolves_a_pending_library_tab_without_a_draw() {
    let mut app = crate::app::render::make_movie_app();
    app.library_tab_pending = 1;
    let mut harness = TickHarness::new(app);

    // One production tick (no events reach the app) + the sync pass; no draw
    // in between.
    harness.inject(Event::User(UserEvent::Clock(Instant::now())));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        harness.model().app.library_tab_pending,
        0,
        "the pending tab is consumed"
    );
    assert_eq!(
        harness.model().app.tab,
        TabSelection::EmbyLibrary(0),
        "the pending position resolves onto the loaded library"
    );
}

fn queue_focused_harness() -> TickHarness {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    TickHarness::new(app)
}

fn active_queue_harness() -> TickHarness {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.player.status.lock().unwrap().active = true;
    TickHarness::new(app)
}

/// Unhandled `Space` fires the playback candidate on the press itself: no
/// repeated-press window arms, and every unhandled press fires again
/// (semantic-input-arbitration, "Unhandled Space fires immediately").
#[test]
fn live_tick_unhandled_space_fires_playback_on_the_press() {
    let mut harness = active_queue_harness();

    harness.inject(key(Key::Char(' ')));
    let first = harness.step();
    assert!(matches!(first.router, RouterOutcome::Deferred(_)));
    assert!(first.deferred_fired);

    harness.inject(key(Key::Char(' ')));
    let second = harness.step();
    assert!(matches!(second.router, RouterOutcome::Deferred(_)));
    assert!(
        second.deferred_fired,
        "a later unhandled press fires again as a first press"
    );
}

/// The double-Esc stop (see `Model::router_outcome`): the first Esc falls
/// through (the leaf's own consumption — e.g. the playback panel's `Stop` —
/// claims first; see the Visual-mode records in `tests/tick_integration/home.rs`),
/// and the second press inside the window fires the stop candidate.
#[test]
fn live_tick_unhandled_escape_fires_stop_on_the_second_press() {
    let mut harness = active_queue_harness();

    harness.inject(key(Key::Esc));
    let first = harness.step();
    assert_eq!(first.router, RouterOutcome::FallThrough);
    assert!(!first.deferred_fired);

    harness.inject(key(Key::Esc));
    let second = harness.step();
    assert_eq!(
        second.router,
        RouterOutcome::Deferred(crate::app::dispatch::action::Command::Stop)
    );
    assert!(second.deferred_fired);
}

#[test]
fn live_tick_local_mutation_precedes_root_observation() {
    let mut harness = TickHarness::new(make_app_stub());
    harness.model_mut().mount_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(key(Key::Char('a')));
    let first = harness.step();
    harness.inject(key(Key::Char('b')));
    let second = harness.step();

    assert!(matches!(second.router, RouterOutcome::FallThrough));
    assert!(second
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::TerminalEvent(TerminalObserverEvent::Key(_)))));
    assert!(second.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)
    )));
    assert!(second.messages.iter().all(|message| !matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)
    )));
    assert_eq!(
        second.raw_messages.len(),
        2,
        "leaf claim and root observation"
    );
    assert_eq!(
        search_component_mut(&mut harness)
            .debounce_pending
            .as_deref(),
        Some("ab"),
        "the focused search component mutated its local query before root observation"
    );
    assert_eq!(
        first.raw_messages.len(),
        2,
        "each local key yields a leaf claim and root observation"
    );
}

pub(super) fn search_component_mut(harness: &mut TickHarness) -> &mut SearchSidebarComponent {
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Overlay(OverlayId::Search))
        .expect("search sidebar mounted")
        .as_any_mut()
        .downcast_mut::<SearchSidebarComponent>()
        .expect("search sidebar type")
}

fn arm_search_query(harness: &mut TickHarness, query: &str) {
    for c in query.chars() {
        let message = search_component_mut(harness).on(&key(Key::Char(c)));
        assert!(
            matches!(
                message,
                Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
            ),
            "typing search chars is locally consumed"
        );
    }
}

/// Phase 1 delivery proof (task 2.7): with Queue focused, a click on the
/// seek-bar row still reaches the unfocused `LibraryPlaybackPanel` through its
/// `mouse_sub()` subscription, and the component resolves the column against
/// its own painted `seekbar_area` into a 0.0..=1.0 fraction. No other eligible
/// surface claims the event (D2 exclusivity). The strip renders only where
/// `RootFrame` places it (task 3.5), so this frame is library-only.
#[test]
fn tick_delivers_seekbar_click_to_unfocused_playback_as_a_fraction() {
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.connected_session_id = Some("session-1".into());
    app.layout.root_frame.library_playback = Some(Rect::new(10, 5, 40, 4));
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal
        .draw(|frame| {
            harness
                .model_mut()
                .render_library_playback_panel_at(frame, Rect::new(10, 5, 40, 4));
        })
        .unwrap();

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 30,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();

    assert_eq!(outcome.pre_fold_focus, Some(ComponentId::Library));
    let seeks: Vec<f64> = outcome
        .raw_messages
        .iter()
        .filter_map(|msg| match msg {
            Msg::Playback(PlaybackRequest::SeekTo(f)) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(seeks.len(), 1, "exactly one surface claims the click");
    assert!(
        (seeks[0] - 0.5).abs() < 1e-6,
        "column 30 of x10..w40 is 0.5"
    );
}

#[test]
fn tick_delivers_key_to_focused_queue_before_root_observer_once() {
    let mut harness = queue_focused_harness();
    harness.inject(key(Key::Char('[')));

    let outcome = harness.step();

    assert_eq!(outcome.pre_fold_focus, Some(ComponentId::Queue));
    assert!(matches!(outcome.router, RouterOutcome::FallThrough));
    assert_eq!(outcome.raw_messages.len(), 2, "one leaf and one observer");
    assert!(matches!(
        outcome.raw_messages.first(),
        Some(Msg::Queue(QueueRequest::Scope(
            crate::app::QueueScope::Local
        )))
    ));
    assert!(matches!(
        outcome.raw_messages.get(1),
        Some(Msg::TerminalEvent(TerminalObserverEvent::Key(_)))
    ));
    assert_eq!(
        outcome
            .raw_messages
            .iter()
            .filter(|msg| matches!(msg, Msg::Queue(QueueRequest::Scope(_))))
            .count(),
        1
    );
    assert_eq!(
        outcome
            .raw_messages
            .iter()
            .filter(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::Key(_))))
            .count(),
        1
    );
    assert_eq!(outcome.messages.len(), 1, "observer key is fold-only");

    harness.inject(key(Key::Char('[')));
    let next = harness.step();
    assert_eq!(next.raw_messages.len(), 2);
    assert_eq!(next.messages.len(), 1);
}

#[test]
fn full_sync_sequence_leaves_focus_on_queue_or_library_destination() {
    let mut queue_harness = queue_focused_harness();
    queue_harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        queue_harness.model().application.focus(),
        Some(&ComponentId::Queue)
    );

    let mut library_app = crate::app::render::make_movie_app();
    library_app.tab = TabSelection::EmbyLibrary(0);
    library_app.panel_focus = PanelFocus::Library;
    library_app.panel_mode = PanelMode::Both;
    let mut library_harness = TickHarness::new(library_app);
    library_harness.model_mut().sync_mounted_surfaces();
    // Task 6.1: Movies moved into the mounted `LibraryPanel` as the
    // `EmbyLibraryContent` owner, so its library tab routes to the panel (the
    // old-destination-child focus for un-migrated libraries is covered by
    // `tests_tick_integration_library_panel::library_panel_focus_follows_the_active_library`).
    assert_eq!(
        library_harness.model().application.focus(),
        Some(&ComponentId::Library)
    );

    let mut stub_app = make_app_stub();
    // Task 1.1: the sync pass normalizes a stale Service-library destination
    // before the focus pass routes, so this stub's index-0 tab (no libraries
    // loaded) resolves to Home in the pass, and focus follows the mounted
    // Home destination instead of falling through to UiRoot.
    stub_app.tab = TabSelection::EmbyLibrary(0);
    stub_app.panel_focus = PanelFocus::Library;
    let mut stub_harness = TickHarness::new(stub_app);
    stub_harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        stub_harness.model().app.tab,
        TabSelection::Home,
        "the sync pass normalizes the stale destination before routing"
    );
    assert_eq!(
        stub_harness.model().application.focus(),
        Some(&ComponentId::Library),
        "the Home owner is installed with the panel, so the normalized Home tab routes to the Library panel (task 5.11)"
    );
}

/// Panel switching is the Ctrl+Left/Ctrl+Right chord (user decision
/// 2026-09-20, replacing the bare arrows, which now reach the tree's own
/// parent/child chords): the router fold must claim the Ctrl chords and move
/// panel focus, proven here through the real tick + sync pass rather than a
/// direct `Component::on`.
#[test]
fn tick_ctrl_arrows_switch_main_panels() {
    // Queue focused: Ctrl+Right moves panel focus to Library.
    let mut harness = queue_focused_harness();
    harness.inject(ctrl_key(Key::Right));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(Command::FocusPanel(PanelFocus::Library))
    );
    if let RouterOutcome::Command(command) = outcome.router {
        harness.model_mut().dispatch_router_command(&command);
    }
    assert_eq!(harness.model().app.panel_focus, PanelFocus::Library);

    // Library focused in Both mode: Ctrl+Left moves panel focus to Queue.
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    let mut both = TickHarness::new(app);
    both.inject(ctrl_key(Key::Left));
    let outcome = both.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::Command(Command::FocusPanel(PanelFocus::Queue))
    );
    if let RouterOutcome::Command(command) = outcome.router {
        both.model_mut().dispatch_router_command(&command);
    }
    assert_eq!(both.model().app.panel_focus, PanelFocus::Queue);

    // The bare arrows no longer move panel focus: Library focus in Both mode
    // keeps the panel and falls through to the tree's own chord.
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    let mut plain = TickHarness::new(app);
    plain.inject(key(Key::Left));
    let outcome = plain.step();
    assert_ne!(
        outcome.router,
        RouterOutcome::Command(Command::FocusPanel(PanelFocus::Queue)),
        "plain Left is the tree's chord, not a panel switch"
    );
    assert_eq!(
        plain.model().app.panel_focus,
        PanelFocus::Library,
        "plain Left leaves panel focus where it was"
    );
}

#[test]
fn search_clock_user_event_reaches_mounted_search_component() {
    let mut harness = TickHarness::new(make_app_stub());
    harness.model_mut().mount_sidebar(SidebarId::Search);
    arm_search_query(&mut harness, "ab");
    // Expire the deadline directly instead of sleeping out the 300ms
    // wall-clock debounce: the duration is not under test, only that a
    // past-due deadline dispatches on Clock.
    search_component_mut(&mut harness).debounce_deadline = Some(
        Instant::now()
            .checked_sub(Duration::from_millis(1))
            .expect("a 1ms-back deadline only underflows before process start"),
    );

    harness.inject(Event::User(UserEvent::Clock(Instant::now())));
    let raw_messages = harness
        .model_mut()
        .application
        .tick(tuirealm::application::PollStrategy::Once(
            Duration::from_millis(500),
        ))
        .expect("tick user clock");

    assert!(raw_messages.iter().any(|msg| {
        matches!(
            msg,
            Msg::Service(ServiceRequest::SearchQuery(query)) if query == "ab"
        )
    }));
    let component = search_component_mut(&mut harness);
    assert!(component.debounce_pending.is_none());
    assert!(component.debounce_deadline.is_none());
}

#[test]
fn search_clock_sweep_dispatches_debounce_on_step() {
    let mut harness = TickHarness::new(make_app_stub());
    harness.model_mut().mount_sidebar(SidebarId::Search);
    arm_search_query(&mut harness, "ab");
    assert!(harness
        .model_mut()
        .tick_search_clock(Instant::now())
        .is_none());

    // Expire the deadline directly instead of sleeping out the 300ms
    // wall-clock debounce, and invoke the run-loop sweep directly: step()'s
    // trailing application.tick would block its full 500ms poll with no
    // event queued, and this test asserts nothing about that tick part
    // (raw_messages is expected empty).
    search_component_mut(&mut harness).debounce_deadline = Some(
        Instant::now()
            .checked_sub(Duration::from_millis(1))
            .expect("a 1ms-back deadline only underflows before process start"),
    );
    let dispatched = harness
        .model_mut()
        .tick_search_clock(Instant::now())
        .expect("sweep must dispatch a past-due debounce");
    assert!(
        matches!(
            dispatched,
            Msg::Service(ServiceRequest::SearchQuery(ref query)) if query == "ab"
        ),
        "sweep must dispatch the armed query, got {dispatched:?}"
    );
    let component = search_component_mut(&mut harness);
    assert!(component.debounce_pending.is_none());
    assert!(component.debounce_deadline.is_none());
}

/// Mini view keeps `effective_panel_focus` on Queue, so `sync_queue` must not
/// re-activate Queue on the tick after a sidebar mounts, stealing the Esc that
/// would close it. The sync passes must yield focus while an overlay is up.
#[test]
fn esc_closes_a_sidebar_in_mini_view() {
    let mut app = make_app_stub();
    app.terminal_width = 70;
    let mut harness = TickHarness::new(app);
    harness.model_mut().mount_sidebar(SidebarId::Sessions);
    let id = ComponentId::Overlay(OverlayId::Sessions);

    // The sync pass that previously stole focus back to Queue.
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().application.focus(), Some(&id));

    harness.inject(key(Key::Esc));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().application.mounted(&id));
}

#[test]
fn blocking_confirm_overlay_keeps_focus_and_receives_input() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.pending_overlay = Some(OverlayRequest::Confirm(ConfirmModal {
        title: "Clear queue?".into(),
        message: "Remove queued items".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: ConfirmAction::ClearQueue,
    }));
    let mut harness = TickHarness::new(app);

    harness.model_mut().sync_mounted_surfaces();
    let confirm_id = ComponentId::Modal(ModalId::Confirm);
    assert_eq!(harness.model().application.focus(), Some(&confirm_id));

    harness.inject(key(Key::Char('y')));
    let outcome = harness.step();
    assert_eq!(outcome.pre_fold_focus, Some(confirm_id.clone()));
    assert!(matches!(outcome.router, RouterOutcome::FallThrough));
    assert!(matches!(
       outcome.raw_messages.first(),
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
           ConfirmIntent::Accept
       ))));

    harness
        .model_mut()
        .application
        .active(&ComponentId::Queue)
        .expect("activate lower queue for swallow guard");
    harness.inject(key(Key::Char('c')));
    let pre_fold_focus = harness.model().application.focus().cloned();
    let raw_messages = harness
        .model_mut()
        .application
        .tick(PollStrategy::Once(Duration::from_millis(500)))
        .expect("tick lower focused queue");
    let router = harness.model_mut().router_outcome(&raw_messages);
    let messages = fold_keyboard_messages(raw_messages, pre_fold_focus.as_ref(), &router);
    assert_eq!(pre_fold_focus, Some(ComponentId::Queue));
    assert!(matches!(router, RouterOutcome::Swallow));
    assert!(messages.is_empty());
}

/// Task 3.3 (add-mouse-support-option): the Display section's MouseSupport
/// row routes Enter through the shell sync pass to `handle_settings_activate`,
/// flipping the config value and arming the live capture flip for the run
/// loop (item ordinal 7: Services, 4 Playback, ImageProtocol,
/// SystemNotifications).
#[test]
fn settings_mouse_support_row_toggle_flips_config_and_arms_capture() {
    let mut harness = TickHarness::new(make_app_stub());

    harness.inject(key(Key::Function(2)));
    let outcome = harness.step();
    assert!(matches!(
        outcome.router,
        RouterOutcome::Command(Command::ToggleSettings)
    ));
    harness
        .model_mut()
        .dispatch_router_command(&Command::ToggleSettings);
    {
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
    }
    harness.model_mut().sync_mounted_surfaces();
    let settings_id = ComponentId::Overlay(OverlayId::Settings);
    assert!(harness.model().application.mounted(&settings_id));
    assert_eq!(harness.model().application.focus(), Some(&settings_id));

    // Locate the MouseSupport row ordinal instead of hardcoding Down presses:
    // the flat row order is SETTING_SECTIONS order, and rows shift whenever a
    // section is added (the Keys entry moved this row once already).
    let mouse_support_downs = crate::app::state::types::settings::SETTING_SECTIONS
        .iter()
        .flat_map(|(_, keys)| keys.iter())
        .position(|key| *key == crate::app::state::types::settings::SettingKey::MouseSupport)
        .expect("MouseSupport row exists in SETTING_SECTIONS");
    for _ in 0..mouse_support_downs {
        harness.inject(key(Key::Down));
        let outcome = harness.step();
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
    }

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }

    assert!(!harness.model().app.config.lock().unwrap().mouse_support);
    assert_eq!(harness.model().app.mouse_capture_pending, Some(false));
}

mod disconnect;
pub(crate) mod harness;
mod keybinds;
mod mouse;
mod mouse_sidebar;
mod playback_title_parts;
mod prefix_mode;
mod queue_playback;
mod sessions;

/// A function key pressed while the Help overlay is open must dismiss Help and
/// open its sidebar. Help stays mounted otherwise, and (painting after
/// Settings/Playlists in OVERLAY_IDS) it hid the sidebar the key just opened —
/// F2/F4 looked dead while F3 (painted after Help) worked.
#[test]
fn function_keys_from_help_dismiss_help_and_open_their_sidebar() {
    let step_with_key = |harness: &mut TickHarness, code: Key| {
        harness.inject(key(code));
        let outcome = harness.step();
        if let RouterOutcome::Command(command) = &outcome.router {
            harness.model_mut().dispatch_router_command(command);
        }
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
        harness.model_mut().sync_mounted_surfaces();
    };

    for (code, overlay) in [
        (Key::Function(2), OverlayId::Settings),
        (Key::Function(3), OverlayId::Sessions),
        (Key::Function(4), OverlayId::Playlists),
    ] {
        let mut harness = TickHarness::new(make_app_stub());
        step_with_key(&mut harness, Key::Function(1));
        let help_id = ComponentId::Overlay(OverlayId::Help);
        assert!(
            harness.model().application.mounted(&help_id),
            "F1 must open Help first"
        );

        step_with_key(&mut harness, code);
        let target_id = ComponentId::Overlay(overlay);
        assert!(harness.model().application.mounted(&target_id));
        assert_eq!(harness.model().application.focus(), Some(&target_id));
        assert!(
            !harness.model().application.mounted(&help_id),
            "opening a sidebar from Help must dismiss Help; otherwise it paints over the sidebar"
        );
    }
}
