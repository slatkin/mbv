//! Table-driven production-style routing matrix (task 2.2).
//!
//! This is the `Application::tick()`-level integration harness. TuiRealm's
//! `with_test_barrier` is `#[cfg(test)]` inside the tuirealm crate, so mbv's
//! tests cannot inject terminal events into a live `Application::tick`; the
//! matrix therefore drives the exact seam where routing happens — the ADR 0023
//! fold (`apply_router_outcome`) — with the exact message ordering
//! `Application::tick` produces: the focused component's message first, then
//! the UiRoot observer's `TerminalEvent`.
//!
//! Each row pins one load-bearing precedence quirk from the handoff (task 1.3)
//! plus the required U2 coverage: blocking-overlay swallow, router
//! `Command`/`Swallow` discarding the leaf's message, `FallThrough` leaving
//! exactly one leaf message standing, Queue-vs-Library focus routing, playback
//! gating, and the double-tap first-press fall-through / second-press claim.
//!
//! The matrix began against the empty policy; global rows now assert the live
//! `Command`/`Swallow` outcomes while the remaining migration rows continue to
//! pin their deliberate `FallThrough` behavior until their owning task moves
//! the effect into the router. Playback rows additionally pin the live
//! first-press FallThrough / second-press Command policy.

use super::*;
use crate::app::action::Command;
use crate::app::components::{
    BrowserKey, BrowserKind, ComponentId, ModalId, Msg, OverlayId, QueueRequest, ShellRequest,
    TerminalObserverEvent,
};
use crate::app::input_resolver::KeyChord;
use crate::app::router::{resolve_router_outcome, RouterOutcome, RouterSnapshot};
use crate::app::shell::apply_router_outcome;
use crate::app::types_playback::QueueScope;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::ServiceKind;

/// Simulate one `Application::tick`'s message list for a key chord: the
/// focused leaf's request (or `None`) plus the UiRoot observer's key signal.
/// Returns the messages that survive the fold.
fn fold_tick(
    leaf: Option<Msg>,
    key: KeyEvent,
    focused: Option<ComponentId>,
    snapshot: RouterSnapshot,
) -> Vec<Msg> {
    let mut messages = Vec::new();
    if let Some(leaf) = leaf {
        messages.push(leaf);
    }
    messages.push(Msg::TerminalEvent(TerminalObserverEvent::Key(key)));
    let outcome = resolve_router_outcome(key, &snapshot);
    apply_router_outcome(messages, focused.as_ref(), &outcome)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A router policy stub for testing the fold's `Command`/`Swallow` arms. The
/// production `resolve_router_outcome` is empty (all `FallThrough`) until
/// section 4; these tests inject the outcome directly to prove the fold
/// semantics that the live policy will rely on.
fn fold_tick_with_outcome(
    leaf: Option<Msg>,
    key: KeyEvent,
    focused: Option<ComponentId>,
    outcome: RouterOutcome,
) -> Vec<Msg> {
    let mut messages = Vec::new();
    if let Some(leaf) = leaf {
        messages.push(leaf);
    }
    messages.push(Msg::TerminalEvent(TerminalObserverEvent::Key(key)));
    apply_router_outcome(messages, focused.as_ref(), &outcome)
}

fn idle_snapshot() -> RouterSnapshot {
    RouterSnapshot {
        ..RouterSnapshot::default()
    }
}

fn active_snapshot() -> RouterSnapshot {
    RouterSnapshot {
        player_active: true,
        ..RouterSnapshot::default()
    }
}


// ── U2 coverage rows ────────────────────────────────────────────────────────

/// Blocking overlay `Swallow`s an unbound chord: the leaf (the overlay) is the
/// active component; the router's `Swallow` discards even its request and runs
/// nothing.
#[test]
fn blocking_overlay_swallows_unbound_chord() {
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmKey(key(KeyCode::Char('x')))));
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

/// Blocking overlay `Swallow`s a global chord (`q` would otherwise quit).
#[test]
fn blocking_overlay_swallows_global_chord() {
    let leaf = Some(Msg::Shell(ShellRequest::ConfirmKey(key(KeyCode::Char('q')))));
    let out = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Char('q')),
        Some(ComponentId::Modal(ModalId::Confirm)),
        RouterOutcome::Swallow,
    );
    assert!(
        out.is_empty(),
        "a blocking overlay's Swallow must swallow even a global quit chord"
    );
}

/// Router `Command` discards the focused leaf's message for that tick and
/// dispatches the command instead.
#[test]
fn router_command_discards_focused_leaf_message() {
    let leaf = Some(Msg::Shell(ShellRequest::GlobalViewKey(key(KeyCode::Char('q')))));
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

/// Router `FallThrough` leaves exactly one leaf message standing, with no
/// global effect: the observer key signal is dropped (the leaf got the event)
/// and the leaf's request survives.
#[test]
fn fallthrough_leaves_exactly_one_leaf_message_standing() {
    let leaf = Some(Msg::Shell(ShellRequest::GlobalViewKey(key(KeyCode::Down))));
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
        Msg::Shell(ShellRequest::GlobalViewKey(_))
    ));
}

/// `FallThrough` with no leaf message (the leaf returned `None`) leaves
/// nothing to run — no global effect fires.
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

/// Queue focus routes Queue chords to the Queue owner: the Queue component's
/// request stands under the empty policy (it handles `[`/`]` locally).
#[test]
fn queue_focus_routes_queue_chord_to_queue_owner() {
    let leaf = Some(Msg::Queue(QueueRequest::Scope(
        QueueScope::Local,
    )));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Queue),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1, "Queue's own scope request must stand");
    assert!(matches!(
        &out[0],
        Msg::Queue(QueueRequest::Scope(_))
    ));
}

/// Library focus routes the same `[` chord to the Library leaf, not Queue:
/// the Library destination interprets `[` as letter-pill cycling / season
/// switching, meaning something different under Library focus (handoff 6c).
#[test]
fn library_focus_routes_bracket_to_library_leaf() {
    let leaf = Some(Msg::Shell(ShellRequest::BrowserCycleLetterPill { delta: -1 }));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::BrowserCycleLetterPill { delta: -1 })
    ));
}

// ── Playback gating and the double-tap (handoff 6e) ─────────────────────────

/// Space with playback active: first press falls through (leaf's request
/// stands — no toggle). The router claims the second press within 300 ms;
/// the live timer is exercised by the shell-level test in `shell_tests`.
#[test]
fn playback_gating_space_first_press_falls_through() {
    let leaf = Some(Msg::Shell(ShellRequest::GlobalViewKey(key(KeyCode::Char(' ')))));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char(' ')),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        active_snapshot(),
    );
    assert_eq!(
        out.len(),
        1,
        "first Space press must fall through (browse leaf keeps its request)"
    );
}

/// Esc with playback active: first press falls through (the leaf's own
/// meaning, e.g. browse `go_back`), second within 300 ms claims Stop.
#[test]
fn playback_gating_esc_first_press_falls_through() {
    let leaf = Some(Msg::Shell(ShellRequest::BrowserBack));
    let out = fold_tick(
        leaf,
        key(KeyCode::Esc),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        active_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], Msg::Shell(ShellRequest::BrowserBack)));
}

#[test]
fn playback_gating_space_second_press_claims_toggle() {
    let mut snapshot = active_snapshot();
    snapshot.space_double_tap = true;
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char(' ')), &snapshot),
        RouterOutcome::Command(Command::TogglePlayPause)
    );
}

#[test]
fn playback_gating_esc_second_press_claims_stop() {
    let mut snapshot = active_snapshot();
    snapshot.esc_double_tap = true;
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Esc), &snapshot),
        RouterOutcome::Command(Command::Stop)
    );
}

#[test]
fn playback_policy_preserves_per_key_eligibility() {
    let active = active_snapshot();
    assert_eq!(
        resolve_router_outcome(
            key(KeyCode::Char('<')),
            &active,
        ),
        RouterOutcome::Command(Command::SeekRelative(-5.0))
    );
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('a')), &active),
        RouterOutcome::Command(Command::ToggleMuteOrCycleAudio)
    );
    assert_eq!(
        resolve_router_outcome(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &active,
        ),
        RouterOutcome::FallThrough
    );
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('m')), &idle_snapshot()),
        RouterOutcome::Command(Command::ToggleMute)
    );
}

#[test]
fn visualizer_resolves_to_router_command() {
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('v')), &idle_snapshot()),
        RouterOutcome::Command(Command::ToggleVisualizer)
    );
}

#[test]
fn playback_and_visualizer_commands_fall_through_under_blocking_overlay() {
    let snapshot = RouterSnapshot {
        player_active: true,
        blocking_overlay_open: true,
        space_double_tap: true,
        ..RouterSnapshot::default()
    };

    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('v')), &snapshot),
        RouterOutcome::FallThrough
    );
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('m')), &snapshot),
        RouterOutcome::FallThrough
    );
}

#[test]
fn idle_feed_path_uses_connected_session_not_broad_playback_route() {
    let snapshot = RouterSnapshot {
        has_remote_session: true,
        idle_feed_link_available: true,
        ..RouterSnapshot::default()
    };
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('o')), &snapshot),
        RouterOutcome::Command(Command::OpenIdleFeedLink)
    );

    let connected = RouterSnapshot {
        connected_session_id_present: true,
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome(key(KeyCode::Char('o')), &connected),
        RouterOutcome::FallThrough
    );
}

// ── Task 1.3 load-bearing quirks ────────────────────────────────────────────

/// (a) `clear_queue_prompt_c` vs context-menu mutual exclusion (#135): with a
/// context menu open (a blocking overlay), `'c'` must not open the clear-queue
/// confirmation. The blocking overlay's `Swallow` handles this once section
/// 4.4 activates; with the empty policy the menu's own key handling stands.
#[test]
fn clear_queue_c_does_not_fire_under_open_context_menu() {
    let leaf = Some(Msg::Shell(ShellRequest::ContextMenuKey(key(KeyCode::Char('c')))));
    let out = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Char('c')),
        Some(ComponentId::Overlay(OverlayId::ContextMenu)),
        RouterOutcome::Swallow,
    );
    assert!(
        out.is_empty(),
        "an open context menu must swallow 'c' (no clear-queue confirmation)"
    );
}

/// (b) `Ctrl+a` enqueue-before-playback claim (#209): under Library focus,
/// `Ctrl+a` means enqueue-selected and must be claimed BEFORE the playback
/// context's `'a'` (which is `!ctrl`-guarded) ever sees it. The leaf's typed
/// enqueue request stands under the empty policy.
#[test]
fn ctrl_a_under_library_focus_is_enqueue_not_audio_toggle() {
    let leaf = Some(Msg::Shell(ShellRequest::BrowserEnqueue {
        item: crate::app::tests::make_item("item", "Movie"),
    }));
    let out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        active_snapshot(),
    );
    assert_eq!(out.len(), 1, "Ctrl+a must reach the leaf as enqueue-selected");
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::BrowserEnqueue { .. })
    ));
    // The playback command table's `'a'` arm is `!ctrl`-guarded; assert the
    // audio toggle does NOT resolve for Ctrl+a even with playback active.
    let playback_cmd = crate::app::action::playback_command_for_key(
        KeyChord {
            code: KeyCode::Char('a'),
            mods: KeyModifiers::CONTROL,
        },
        true,
        false,
    );
    assert!(
        playback_cmd.is_none(),
        "audio toggle must not claim Ctrl+a (enqueue-before-playback, #209)"
    );
}

/// (d) `handle_lib_key` Ctrl/Alt catch-all swallow: under Library focus, an
/// unmapped Ctrl chord (e.g. `Ctrl+z`) must not leak to the Queue's undo
/// shortcut. The leaf's `None` (local swallow) plus `FallThrough` runs
/// nothing — no queue undo fires.
#[test]
fn lib_key_ctrl_catchall_swallows_unmapped_chord() {
    let out = fold_tick(
        None,
        KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        idle_snapshot(),
    );
    assert!(
        out.is_empty(),
        "an unmapped Ctrl chord under Library focus must be swallowed (no queue undo)"
    );
}

/// (f) Ctrl+/ terminal-encoding ambiguity: `Char('/')` and `Char('_')` with
/// CONTROL are the same chord across terminals. The policy must treat both
/// encodings identically once overlay-open activates; today both fall through
/// to the leaf.
#[test]
fn ctrl_slash_both_terminal_encodings_route_identically() {
    let leaf = Some(Msg::Shell(ShellRequest::GlobalViewKey(key(KeyCode::Char('/')))));
    let slash_out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL),
        Some(ComponentId::UiRoot),
        idle_snapshot(),
    );
    let leaf = Some(Msg::Shell(ShellRequest::GlobalViewKey(key(KeyCode::Char('_')))));
    let underscore_out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('_'), KeyModifiers::CONTROL),
        Some(ComponentId::UiRoot),
        idle_snapshot(),
    );
    assert_eq!(
        slash_out.len(),
        underscore_out.len(),
        "both Ctrl+/ encodings must route identically"
    );
}

#[test]
fn destination_independent_globals_resolve_to_router_commands() {
    let mut snapshot = idle_snapshot();
    let globals = [
        (key(KeyCode::Char('q')), Command::Quit),
        (key(KeyCode::Tab), Command::NextLibraryTab),
        (key(KeyCode::BackTab), Command::PreviousLibraryTab),
        (key(KeyCode::Char('1')), Command::SetLibraryTab(0)),
        (
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            Command::ForceClear,
        ),
        (key(KeyCode::F(5)), Command::RefreshCurrentView),
        (key(KeyCode::Char('x')), Command::CyclePanelMode),
        (key(KeyCode::F(2)), Command::ToggleSettings),
        (key(KeyCode::F(3)), Command::OpenSessions),
        (key(KeyCode::F(4)), Command::OpenPlaylists),
        (
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL),
            Command::OpenSearch,
        ),
    ];
    for (key, command) in globals {
        assert_eq!(
            resolve_router_outcome(key, &snapshot),
            RouterOutcome::Command(command),
            "global {key:?} must be claimed by the router"
        );
    }

    snapshot.help_overlay_open = false;
    assert_eq!(
        resolve_router_outcome(key(KeyCode::F(1)), &snapshot),
        RouterOutcome::Command(Command::OpenHelp)
    );
}

#[test]
fn help_and_alt_router_guards_preserve_overlay_precedence() {
    let mut snapshot = idle_snapshot();
    snapshot.blocking_overlay_open = true;
    assert_eq!(
        resolve_router_outcome(key(KeyCode::F(1)), &snapshot),
        RouterOutcome::FallThrough,
        "F1 must not open Help over a blocking overlay"
    );

    snapshot.blocking_overlay_open = false;
    snapshot.help_overlay_open = true;
    assert_eq!(
        resolve_router_outcome(key(KeyCode::F(1)), &snapshot),
        RouterOutcome::FallThrough,
        "Help keeps F1 for its dismiss request"
    );

    snapshot.help_overlay_open = false;
    snapshot.panel_focus = crate::app::PanelFocus::Queue;
    assert_eq!(
        resolve_router_outcome(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &snapshot
        ),
        RouterOutcome::Command(Command::FocusPanel(crate::app::PanelFocus::Library))
    );
    assert_eq!(
        resolve_router_outcome(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &snapshot
        ),
        RouterOutcome::Command(Command::NextLibraryTab)
    );
    assert_eq!(
        resolve_router_outcome(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT),
            &snapshot
        ),
        RouterOutcome::Swallow,
        "unhandled Alt chords must not leak into destination handling"
    );
}
