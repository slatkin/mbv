//! Table-driven production-style routing matrix (task 2.2).
//!
//! `tests_tick_harness.rs` now injects events into a live `Application::tick()`
//! via `EventListenerCfg::add_port`; this matrix remains because the cheap
//! table rows cover precedence combinations that would be wasteful to exercise
//! through the live harness. It drives the exact seam where routing happens —
//! the ADR 0023 fold (`apply_router_outcome`) — with the exact message ordering
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

use crate::app::components::{BrowserKey, BrowserKind, ComponentId, Msg, TerminalObserverEvent};
use crate::app::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use crate::app::shell::apply_router_outcome;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::ServiceKind;

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
    let outcome = resolve_router_outcome_with_focused(key, &snapshot, None);
    apply_router_outcome(messages, focused.as_ref(), &outcome)
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
    apply_router_outcome(messages, focused.as_ref(), &outcome)
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
    let outcome = resolve_router_outcome_with_focused(key, &snapshot, focused.as_ref());
    apply_router_outcome(messages, focused.as_ref(), &outcome)
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

pub(crate) fn browser_key() -> BrowserKey {
    BrowserKey {
        service: ServiceKind::Emby,
        library_id: "lib".into(),
        kind: BrowserKind::Generic,
    }
}
