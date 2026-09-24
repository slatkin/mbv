//! Prefix mode through the live shell (tasks 6.1, 6.3).
//!
//! Proves the full prefix state machine through real `Application::tick()`
//! runs (inject port → UiRoot observer → router fold → shell transitions):
//! arming consumes the prefix chord, an armed chord never reaches any
//! component, mapped prefix chords dispatch and disarm, unmapped chords
//! swallow and disarm, the double prefix re-arms, and any mouse event
//! silently disarms without altering the event's own handling.

use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::dispatch::action::Command;
use crate::app::components::home_content::HomeContent;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, UserEvent};
use crate::app::input::router::RouterOutcome;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, TabSelection};
use mbv_core::keybinds::{RawKeybinds, RawSection};

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

/// A `[keys]` configuration with prefix `Ctrl+b` and the given
/// prefix-namespace assignments, grouped by each action's declared section
/// and validated through the registry loader.
fn prefix_config(assignments: &[(&str, &str)]) -> crate::config::Config {
    let mut sections: Vec<(String, RawSection)> = Vec::new();
    for (id, chord) in assignments {
        let section = mbv_core::keybinds::action_by_id(id)
            .expect("declared action")
            .section
            .name()
            .to_ascii_lowercase();
        let index = sections
            .iter()
            .position(|(name, _)| *name == section)
            .unwrap_or_else(|| {
                sections.push((section.clone(), RawSection::default()));
                sections.len() - 1
            });
        sections[index].1.prefix.push(((*id).into(), (*chord).into()));
    }
    crate::config::Config {
        keybinds: mbv_core::keybinds::load(&RawKeybinds {
            prefix: Some("Ctrl+b".into()),
            sections,
        })
        .expect("valid keys configuration"),
        ..Default::default()
    }
}

fn harness_with(config: crate::config::Config) -> TickHarness {
    let app = make_app_stub();
    *app.config.lock().unwrap() = config;
    TickHarness::new(app)
}

/// The Home content owner inside the mounted `LibraryPanel` — its local
/// cursor is the component counter proving an armed chord reached nothing.
fn home_owner(harness: &TickHarness) -> &HomeContent {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&crate::app::components::library_panel::LibraryKey::Home)
        .and_then(|owner| owner.as_any().downcast_ref::<HomeContent>())
        .expect("Home owner installed")
}

fn home_harness(count: usize, config: crate::config::Config) -> TickHarness {
    let mut app = make_app_stub();
    *app.config.lock().unwrap() = config;
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().home_content.continue_items = (0..count)
        .map(|i| {
            let mut item = make_item(&format!("Home Item {i}"), "Movie");
            item.id = format!("home-{i}");
            item
        })
        .collect();
    harness.model_mut().home_content.loading = false;
    harness.model_mut().push_home_content();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 160, 30);
    harness
}

fn wheel() -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    })
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("test terminal");
    terminal
        .draw(|frame| {
            harness.model_mut().draw_frame(frame, false, false);
        })
        .expect("test draw");
    harness.model_mut().sync_mounted_surfaces();
}

/// Task 6.1: arming consumes the prefix chord — the shell arms, the chord
/// dispatches nothing, and the observer's key event reaches no component.
#[test]
fn arming_consumes_the_prefix_chord() {
    let mut harness = harness_with(prefix_config(&[]));

    harness.inject(key(Key::Char('b')).tap_ctrl());
    let outcome = harness.step();
    assert_eq!(outcome.router, RouterOutcome::PrefixArm);
    assert!(
        harness.model().app.prefix_armed,
        "the router outcome arms prefix mode"
    );
    assert!(
        outcome.messages.iter().all(|m| matches!(m, Msg::TerminalEvent(_))),
        "arming discards the observer key event; nothing reaches a component"
    );
}

/// Task 6.1: any mouse event silently disarms prefix mode and the event's
/// own handling is unchanged — the same mouse event produces exactly the
/// same tick messages it would have with prefix mode never armed.
#[test]
fn mouse_event_disarms_without_altering_handling() {
    let mut harness = harness_with(prefix_config(&[]));
    harness.inject(key(Key::Char('b')).tap_ctrl());
    let outcome = harness.step();
    assert_eq!(outcome.router, RouterOutcome::PrefixArm);
    assert!(harness.model().app.prefix_armed);

    // The same mouse event, from a harness that was never armed, for the
    // handling comparison below.
    let never_armed = {
        let mut harness = harness_with(prefix_config(&[]));
        harness.inject(wheel());
        harness.step()
    };

    harness.inject(wheel());
    let outcome = harness.step();
    // The run loop dispatches the surviving messages; the mouse-path disarm
    // lives in `handle_terminal_message` → `apply_terminal_observer`.
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages.clone() {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert!(
        !harness.model().app.prefix_armed,
        "the mouse event silently disarms prefix mode"
    );
    assert_eq!(
        outcome.raw_messages, never_armed.raw_messages,
        "the mouse event's delivery is identical to the never-armed case"
    );
    assert_eq!(
        outcome.messages, never_armed.messages,
        "the mouse event's arbitration is identical to the never-armed case"
    );

    // Disarmed for real: a later chord routes normally again.
    let mut harness = harness_with(prefix_config(&[]));
    harness.inject(key(Key::Char('b')).tap_ctrl());
    assert_eq!(harness.step().router, RouterOutcome::PrefixArm);
    harness.inject(wheel());
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages.clone() {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert!(!harness.model().app.prefix_armed);
    harness.inject(key(Key::Char('x')));
    assert_eq!(
        harness.step().router,
        RouterOutcome::Command(Command::CyclePanelMode),
        "after the mouse disarm the keyboard routes as usual"
    );
}

/// Task 6.3: a mapped prefix chord executes its action and disarms, and no
/// chord reaches any component while armed — the Home owner's cursor is the
/// component counter.
#[test]
fn armed_chords_reach_no_component_and_a_mapped_chord_dispatches() {
    // `panel_mode_cycle_x` (Display, ungated) assigned `p` in the prefix
    // namespace: observable shell state, so the dispatch is provable. The
    // Home owner supplies the component counter.
    let mut harness = home_harness(2, prefix_config(&[("panel_mode_cycle_x", "p")]));

    // Arm through the live tick.
    harness.inject(key(Key::Char('b')).tap_ctrl());
    assert_eq!(harness.step().router, RouterOutcome::PrefixArm);

    // While armed, Down reaches nothing: the Home cursor (the counter)
    // stays put and the tick yields no leaf message. An unmapped armed
    // chord swallows and disarms.
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    assert_eq!(outcome.router, RouterOutcome::PrefixSwallow);
    assert!(!harness.model().app.prefix_armed, "an unmapped armed chord disarms");
    assert_eq!(home_owner(&harness).cursor(), 0, "no chord reached the component while armed");
    assert!(
        outcome.messages.iter().all(|m| matches!(m, Msg::TerminalEvent(_))),
        "an armed chord reaches no component"
    );

    // The same chord after disarm moves the cursor — the counter works.
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    assert_eq!(outcome.router, RouterOutcome::FallThrough);
    assert_eq!(home_owner(&harness).cursor(), 1, "disarmed, the chord reaches the component again");
    assert!(!harness.model().app.prefix_armed);

    // Rearm and fire the mapped chord: it executes (the shell dispatches
    // the panel-mode cycle) and disarms.
    harness.inject(key(Key::Char('b')).tap_ctrl());
    assert_eq!(harness.step().router, RouterOutcome::PrefixArm);
    harness.inject(key(Key::Char('p')));
    let outcome = harness.step();
    assert_eq!(
        outcome.router,
        RouterOutcome::PrefixDispatch(Command::CyclePanelMode)
    );
    assert!(!harness.model().app.prefix_armed, "a mapped dispatch disarms");
    harness
        .model_mut()
        .dispatch_router_command(Command::CyclePanelMode);
    assert_ne!(
        harness.model().app.panel_mode,
        crate::app::PanelMode::default(),
        "the mapped prefix action executed through the shell"
    );
}

/// Task 6.3: the double prefix re-arms — the second prefix chord while
/// armed keeps prefix mode armed and consumes the chord.
#[test]
fn double_prefix_re_arms_through_tick() {
    let mut harness = harness_with(prefix_config(&[("next_library_tab", "n")]));

    harness.inject(key(Key::Char('b')).tap_ctrl());
    assert_eq!(harness.step().router, RouterOutcome::PrefixArm);
    assert!(harness.model().app.prefix_armed);

    harness.inject(key(Key::Char('b')).tap_ctrl());
    let outcome = harness.step();
    assert_eq!(outcome.router, RouterOutcome::PrefixArm, "double prefix re-arms");
    assert!(harness.model().app.prefix_armed, "prefix mode stays armed");
    assert!(
        outcome.messages.iter().all(|m| matches!(m, Msg::TerminalEvent(_))),
        "the re-arming chord is consumed, reaching no component"
    );

    // The machine is still armed: the mapped chord now fires.
    harness.inject(key(Key::Char('n')));
    assert_eq!(
        harness.step().router,
        RouterOutcome::PrefixDispatch(Command::NextLibraryTab)
    );
}

/// Task 6.3: an unmapped chord swallowed and disarmed — the chord after it
/// routes normally.
#[test]
fn unmapped_chord_swallows_disarms_and_the_next_chord_routes_normally() {
    let mut harness = harness_with(prefix_config(&[]));

    harness.inject(key(Key::Char('b')).tap_ctrl());
    assert_eq!(harness.step().router, RouterOutcome::PrefixArm);

    harness.inject(key(Key::Char('x')));
    let outcome = harness.step();
    assert_eq!(outcome.router, RouterOutcome::PrefixSwallow);
    assert!(!harness.model().app.prefix_armed, "the unmapped chord disarms");
    assert!(
        outcome.messages.iter().all(|m| matches!(m, Msg::TerminalEvent(_))),
        "the swallowed chord reaches no component"
    );

    harness.inject(key(Key::Char('x')));
    assert_eq!(
        harness.step().router,
        RouterOutcome::Command(Command::CyclePanelMode),
        "after the disarm the chord routes normally"
    );
}

trait TapCtrl {
    fn tap_ctrl(self) -> Self;
}

impl TapCtrl for Event<UserEvent> {
    fn tap_ctrl(mut self) -> Self {
        if let Event::Keyboard(key_event) = &mut self {
            key_event.modifiers |= KeyModifiers::CONTROL;
        }
        self
    }
}
