//! Routing matrix: blocking precedence and policy rows.

use super::tests_routing_matrix_support::*;
use crate::app::action::Command;
use crate::app::components::{BrowserKey, BrowserKind, ComponentId, ModalId, Msg, ShellRequest};
use crate::app::components::msg::ConfirmIntent;
use crate::app::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use crossterm::event::KeyCode;
use mbv_core::config::ServiceKind;

#[test]
fn focused_blocking_overlay_keeps_its_own_unbound_chord() {
    let snapshot = RouterSnapshot {
        blocking_overlay_open: true,
        ..RouterSnapshot::default()
    };
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Dismiss)));
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
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept)));
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
            resolve_router_outcome_with_focused(key(code), &snapshot, Some(&focused)),
            RouterOutcome::FallThrough,
            "the focused blocking overlay must keep {code:?}"
        );
    }
}
#[test]
fn injected_swallow_discards_leaf_message() {
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Dismiss)));
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
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
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
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        RouterOutcome::FallThrough,
    );
    assert_eq!(out.len(), 1, "exactly one leaf message must stand");
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::Quit)
    ));
}
#[test]
fn fallthrough_with_no_leaf_message_fires_no_global_effect() {
    let out = fold_tick_with_outcome(
        None,
        key(KeyCode::Down),
        Some(ComponentId::Home),
        RouterOutcome::FallThrough,
    );
    assert!(
        out.is_empty(),
        "no leaf message + FallThrough must run nothing (no global effect)"
    );
}
