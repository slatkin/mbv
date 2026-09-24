//! Table-driven production-style routing matrix (task 2.2).
//!
//! `tests_tick_harness.rs` now injects events into a live `Application::tick()`
//! via `EventListenerCfg::add_port`; this matrix remains because the cheap
//! table rows cover precedence combinations that would be wasteful to exercise
//! through the live harness. It drives the exact seam where routing happens —
//! the ADR 0023 fold (`fold_keyboard_messages`) — with the exact message ordering
//! `Application::tick` produces: the focused component's message first, then
//! the UiRoot observer's `TerminalEvent`.
//!
//! Each row pins one load-bearing precedence quirk from the handoff (task 1.3)
//! plus the required U2 coverage: blocking-overlay swallow, router
//! `Command`/`Swallow` discarding the leaf's message, `FallThrough` leaving
//! exactly one leaf message standing, Queue-vs-Library focus routing, playback
//! gating, and the deferred playback candidates (a consumed chord falls
//! through to the leaf; the shell's candidate fires only on an unhandled
//! press).
//!
//! The matrix began against the empty policy; global rows now assert the live
//! `Command`/`Swallow` outcomes while the remaining migration rows continue to
//! pin their deliberate `FallThrough` behavior until their owning task moves
//! the effect into the router. Playback rows additionally pin the deferred
//! candidate policy: a consumed chord falls through to the leaf, and the
//! candidate resolves as `Deferred`.

use crate::app::components::{ComponentId, Msg, TerminalObserverEvent};
use crate::app::input::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use crate::app::shell::fold_keyboard_messages;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::keybinds::{Chord, KeySection, Keybinds, SectionBindings};

/// The registry-defaults configuration the matrix resolves against (task
/// 3.2): no prefix, every declared action on its declared chords.
pub(crate) fn default_keybinds() -> Keybinds {
    Keybinds::default()
}

/// A configuration with one router-scope override: `id` fires on `chord`
/// instead of its declared default. The section is irrelevant to routing —
/// `Keybinds::router_override` scans every section.
pub(crate) fn keybinds_with_override(id: &'static str, chord: &str) -> Keybinds {
    Keybinds {
        prefix: None,
        sections: vec![(
            KeySection::Global,
            SectionBindings {
                router: vec![(id, Chord::parse(chord).expect("test chord must parse"))],
                prefix: vec![],
            },
        )],
    }
}

pub(crate) fn fold_tick(
    leaf: Option<Msg>,
    key: KeyEvent,
    focused: Option<ComponentId>,
    snapshot: RouterSnapshot,
) -> Vec<Msg> {
    let mut messages = Vec::new();
    if let Some(leaf) = leaf {
        messages.push(leaf);
    }
    messages.push(Msg::TerminalEvent(TerminalObserverEvent::Key(key.into())));
    let outcome = resolve_router_outcome_with_focused(key, &snapshot, None, &default_keybinds());
    fold_keyboard_messages(messages, focused.as_ref(), &outcome)
}

pub(crate) fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub(crate) fn fold_tick_with_outcome(
    leaf: Option<Msg>,
    key: KeyEvent,
    focused: Option<ComponentId>,
    outcome: RouterOutcome,
) -> Vec<Msg> {
    let mut messages = Vec::new();
    if let Some(leaf) = leaf {
        messages.push(leaf);
    }
    messages.push(Msg::TerminalEvent(TerminalObserverEvent::Key(key.into())));
    fold_keyboard_messages(messages, focused.as_ref(), &outcome)
}

pub(crate) fn fold_tick_focused(
    leaf: Option<Msg>,
    key: KeyEvent,
    focused: Option<ComponentId>,
    snapshot: RouterSnapshot,
) -> Vec<Msg> {
    let mut messages = Vec::new();
    if let Some(leaf) = leaf {
        messages.push(leaf);
    }
    messages.push(Msg::TerminalEvent(TerminalObserverEvent::Key(key.into())));
    let outcome = resolve_router_outcome_with_focused(key, &snapshot, focused.as_ref(), &default_keybinds());
    fold_keyboard_messages(messages, focused.as_ref(), &outcome)
}

pub(crate) fn idle_snapshot() -> RouterSnapshot {
    RouterSnapshot {
        ..RouterSnapshot::default()
    }
}

pub(crate) fn active_snapshot() -> RouterSnapshot {
    RouterSnapshot {
        player_active: true,
        ..RouterSnapshot::default()
    }
}

pub(crate) fn text_entry_snapshot() -> RouterSnapshot {
    RouterSnapshot {
        text_entry_focused: true,
        ..RouterSnapshot::default()
    }
}

#[test]
fn stale_summary_does_not_change_current_leaf_arbitration() {
    let stale_summary = Msg::Shell(crate::app::components::ShellRequest::SelectionProjection(
        crate::app::components::media_list::SelectionSummary {
            count: 99,
            origin: crate::app::components::media_list::SelectionOrigin::Queue,
        },
    ));
    let focused = Some(ComponentId::Library);
    let messages = fold_tick_with_outcome(
        Some(stale_summary.clone()),
        key(KeyCode::Char('z')),
        focused.clone(),
        RouterOutcome::FallThrough,
    );
    assert_eq!(messages, vec![stale_summary.clone()]);

    let swallowed = fold_tick_with_outcome(
        Some(stale_summary),
        key(KeyCode::Char('z')),
        focused,
        RouterOutcome::Swallow,
    );
    assert!(swallowed.is_empty());
}

#[test]
fn immediate_router_outcomes_have_distinct_fold_behavior() {
    let leaf = Some(Msg::Shell(crate::app::components::ShellRequest::Quit));
    let focused = Some(ComponentId::Library);

    let command = fold_tick_with_outcome(
        leaf.clone(),
        key(KeyCode::Char('q')),
        focused.clone(),
        RouterOutcome::Command(crate::app::dispatch::action::Command::Quit),
    );
    assert!(command.is_empty(), "Command replaces the focused leaf request");

    let swallow = fold_tick_with_outcome(
        leaf.clone(),
        key(KeyCode::Char('q')),
        focused.clone(),
        RouterOutcome::Swallow,
    );
    assert!(swallow.is_empty(), "Swallow discards the focused leaf request");

    let fall_through = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Char('z')),
        focused,
        RouterOutcome::FallThrough,
    );
    assert_eq!(fall_through.len(), 1, "FallThrough keeps the leaf request");
    assert!(matches!(fall_through[0], Msg::Shell(_)));
}

