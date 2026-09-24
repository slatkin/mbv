//! Routing matrix: blocking precedence and policy rows.

use super::support::*;
use crate::app::components::msg::ConfirmIntent;
use crate::app::components::{ComponentId, ModalId, Msg, ShellRequest};
use crate::app::dispatch::action::Command;
use crate::app::input::router::{
    resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot,
};
use crossterm::event::KeyCode;

#[test]
fn focused_blocking_overlay_keeps_its_own_unbound_chord() {
    let snapshot = RouterSnapshot {
        blocking_overlay_open: true,
        ..RouterSnapshot::default()
    };
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmIntent(
        ConfirmIntent::Dismiss,
    )));
    let out = fold_tick_focused(
        leaf,
        key(KeyCode::Char('x')),
        Some(ComponentId::Modal(ModalId::Confirm)),
        snapshot,
    );
    assert_eq!(out.len(), 1, "the overlay's own request must stand");
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Dismiss))
    ));
}
#[test]
fn focused_blocking_overlay_keeps_its_own_global_chord() {
    let snapshot = RouterSnapshot {
        blocking_overlay_open: true,
        ..RouterSnapshot::default()
    };
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmIntent(
        ConfirmIntent::Accept,
    )));
    let out = fold_tick_focused(
        leaf,
        key(KeyCode::Char('q')),
        Some(ComponentId::Modal(ModalId::Confirm)),
        snapshot,
    );
    assert_eq!(
        out.len(),
        1,
        "a global quit chord must not be swallowed away from the focused overlay"
    );
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept))
    ));
}
#[test]
fn focused_blocking_overlay_falls_through_unmatched_and_global_chords() {
    let snapshot = RouterSnapshot {
        blocking_overlay_open: true,
        ..RouterSnapshot::default()
    };
    let focused = ComponentId::Modal(ModalId::Confirm);

    for code in [KeyCode::Char('z'), KeyCode::Char('q')] {
        assert_eq!(
            resolve_router_outcome_with_focused(
                key(code),
                &snapshot,
                Some(&focused),
                &default_keybinds()
            ),
            RouterOutcome::FallThrough,
            "the focused blocking overlay must keep {code:?}"
        );
    }
}
#[test]
fn injected_swallow_discards_leaf_message() {
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmIntent(
        ConfirmIntent::Dismiss,
    )));
    let out = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Char('x')),
        Some(ComponentId::Modal(ModalId::Confirm)),
        RouterOutcome::Swallow,
    );
    assert!(
        out.is_empty(),
        "Swallow must discard the leaf's message and run nothing"
    );
}
#[test]
fn router_command_discards_focused_leaf_message() {
    let leaf = Some(Msg::Shell(ShellRequest::Quit));
    let out = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Char('q')),
        Some(ComponentId::Library),
        RouterOutcome::Command(Command::Stop),
    );
    assert!(
        out.is_empty(),
        "Command must discard the leaf's message; the command is dispatched by the caller"
    );
}
#[test]
fn fallthrough_leaves_exactly_one_leaf_message_standing() {
    let leaf = Some(Msg::Shell(ShellRequest::Quit));
    let out = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Down),
        Some(ComponentId::Library),
        RouterOutcome::FallThrough,
    );
    assert_eq!(out.len(), 1, "exactly one leaf message must stand");
    assert!(matches!(&out[0], Msg::Shell(ShellRequest::Quit)));
}
#[test]
fn fallthrough_with_no_leaf_message_fires_no_global_effect() {
    let out = fold_tick_with_outcome(
        None,
        key(KeyCode::Down),
        Some(ComponentId::Library),
        RouterOutcome::FallThrough,
    );
    assert!(
        out.is_empty(),
        "no leaf message + FallThrough must run nothing (no global effect)"
    );
}

#[test]
fn rebound_global_respects_blocking_overlays() {
    // `help_open` rebound from F1 to Ctrl+h (task 3.2): under a blocking
    // overlay the rebound chord must not fire — the overlay routing rules
    // hold for configured chords exactly as for declared defaults.
    let keybinds = keybinds_with_override("help_open", "Ctrl+h");
    let rebound_key = crossterm::event::KeyEvent::new(
        KeyCode::Char('h'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let snapshot = RouterSnapshot {
        blocking_overlay_open: true,
        ..RouterSnapshot::default()
    };

    assert_eq!(
        resolve_router_outcome_with_focused(rebound_key, &snapshot, None, &keybinds),
        RouterOutcome::Swallow,
        "a rebound chord under a blocking overlay is swallowed"
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            rebound_key,
            &snapshot,
            Some(&ComponentId::Modal(ModalId::Confirm)),
            &keybinds
        ),
        RouterOutcome::FallThrough,
        "the focused blocking overlay keeps its own request over a rebound chord"
    );
}
