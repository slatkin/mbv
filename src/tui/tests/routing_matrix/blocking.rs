//! Routing matrix: blocking precedence and policy rows.

use super::support::*;
use mbv_client_core::dispatch::action::{PlaybackIntent, ShellCommand};
use mbv_client_core::input::resolver::KeyChord;
use mbv_client_core::input::router::{RouterOutcome, RouterSnapshot, resolve_router_outcome};
use mbv_ui_msg::ConfirmIntent;
use mbv_ui_msg::{ComponentId, ModalId, Msg, ShellRequest};
use mbv_ui_msg::{Key, KeyModifiers};

#[test]
fn focused_blocking_overlay_keeps_its_own_unbound_chord() {
    let snapshot = RouterSnapshot {
        overlays: mbv_client_core::input::RouterOverlayState {
            focus: mbv_client_core::input::OverlayFocus::Blocking,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::ConfirmIntent(
        ConfirmIntent::Dismiss,
    ))));
    let out = fold_tick_focused(
        leaf,
        key(Key::Char('x')),
        Some(&ComponentId::Modal(ModalId::Confirm)),
        snapshot,
    );
    assert_eq!(out.len(), 1, "the overlay's own request must stand");
    assert!(matches!(
        &out[0],
        Msg::Shell(shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(ConfirmIntent::Dismiss))));
}
#[test]
fn focused_blocking_overlay_keeps_its_own_global_chord() {
    let snapshot = RouterSnapshot {
        overlays: mbv_client_core::input::RouterOverlayState {
            focus: mbv_client_core::input::OverlayFocus::Blocking,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::ConfirmIntent(
        ConfirmIntent::Accept,
    ))));
    let out = fold_tick_focused(
        leaf,
        key(Key::Char('q')),
        Some(&ComponentId::Modal(ModalId::Confirm)),
        snapshot,
    );
    assert_eq!(
        out.len(),
        1,
        "a global quit chord must not be swallowed away from the focused overlay"
    );
    assert!(matches!(
        &out[0],
        Msg::Shell(shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(ConfirmIntent::Accept))));
}
#[test]
fn focused_blocking_overlay_falls_through_unmatched_and_global_chords() {
    let snapshot = RouterSnapshot {
        overlays: mbv_client_core::input::RouterOverlayState {
            focus: mbv_client_core::input::OverlayFocus::Blocking,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };
    let focused = ComponentId::Modal(ModalId::Confirm);

    for code in [Key::Char('z'), Key::Char('q')] {
        assert_eq!(
            resolve_router_outcome(
                KeyChord::from_key(key(code)),
                &snapshot,
                &disarmed(),
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
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::ConfirmIntent(
        ConfirmIntent::Dismiss,
    ))));
    let out = fold_tick_with_outcome(
        leaf,
        key(Key::Char('x')),
        Some(&ComponentId::Modal(ModalId::Confirm)),
        &RouterOutcome::Swallow,
    );
    assert!(
        out.is_empty(),
        "Swallow must discard the leaf's message and run nothing"
    );
}
#[test]
fn router_command_discards_focused_leaf_message() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::Quit)));
    let out = fold_tick_with_outcome(
        leaf,
        key(Key::Char('q')),
        Some(&ComponentId::Library),
        &RouterOutcome::Command(PlaybackIntent::Stop.into()),
    );
    assert!(
        out.is_empty(),
        "Command must discard the leaf's message; the command is dispatched by the caller"
    );
}
#[test]
fn fallthrough_leaves_exactly_one_leaf_message_standing() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::Quit)));
    let out = fold_tick_with_outcome(
        leaf,
        key(Key::Down),
        Some(&ComponentId::Library),
        &RouterOutcome::FallThrough,
    );
    assert_eq!(out.len(), 1, "exactly one leaf message must stand");
    assert!(
        matches!(&out[0], Msg::Shell(shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::Quit))
    );
}
#[test]
fn fallthrough_with_no_leaf_message_fires_no_global_effect() {
    let out = fold_tick_with_outcome(
        None,
        key(Key::Down),
        Some(&ComponentId::Library),
        &RouterOutcome::FallThrough,
    );
    assert!(
        out.is_empty(),
        "no leaf message + FallThrough must run nothing (no global effect)"
    );
}

/// The Twitch URL opener (twitch-url-playback task 5.4): Shift+T resolves
/// the shell-owned mount command when no overlay blocks it.
#[test]
fn twitch_url_open_resolves_to_its_shell_command_on_the_default_chord() {
    let key = key_mod(Key::Char('T'), KeyModifiers::SHIFT);
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key),
            &idle_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(ShellCommand::OpenTwitchUrl.into())
    );
}

/// The opener's gate is `NoBlockingOverlay`: while a blocking overlay is
/// mounted the chord is swallowed and the prompt does not open.
#[test]
fn twitch_url_open_is_swallowed_under_a_blocking_overlay() {
    let key = key_mod(Key::Char('T'), KeyModifiers::SHIFT);
    let snapshot = RouterSnapshot {
        overlays: mbv_client_core::input::RouterOverlayState {
            focus: mbv_client_core::input::OverlayFocus::Blocking,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key),
            &snapshot,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Swallow
    );
}

#[test]
fn rebound_global_respects_blocking_overlays() {
    // `help_open` rebound from F1 to Ctrl+h (task 3.2): under a blocking
    // overlay the rebound chord must not fire — the overlay routing rules
    // hold for configured chords exactly as for declared defaults.
    let keybinds = keybinds_with_override("help_open", "Ctrl+h");
    let rebound_key = key_mod(Key::Char('h'), KeyModifiers::CONTROL);
    let snapshot = RouterSnapshot {
        overlays: mbv_client_core::input::RouterOverlayState {
            focus: mbv_client_core::input::OverlayFocus::Blocking,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };

    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(rebound_key),
            &snapshot,
            &disarmed(),
            None,
            &keybinds
        ),
        RouterOutcome::Swallow,
        "a rebound chord under a blocking overlay is swallowed"
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(rebound_key),
            &snapshot,
            &disarmed(),
            Some(&ComponentId::Modal(ModalId::Confirm)),
            &keybinds
        ),
        RouterOutcome::FallThrough,
        "the focused blocking overlay keeps its own request over a rebound chord"
    );
}
