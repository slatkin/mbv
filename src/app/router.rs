//! One Central Keyboard Router (ADR 0023).
//!
//! `UiRoot` is the single keyboard routing authority. This module resolves a
//! chord against the ordered policy and returns ADR 0002's three outcomes —
//! `Command` (run this semantic command, discard the focused leaf's message),
//! `Swallow` (run nothing, discard the leaf's message), or `FallThrough` (the
//! leaf's own typed request stands).

use crossterm::event::KeyEvent;

use super::action::Command;
use super::input_resolver::KeyChord;
use super::key_policy::resolve_policy;

pub(super) use super::key_policy::RouterSnapshot;

/// ADR 0002's three routing outcomes, exactly. `Application::tick` returns the
/// focused component's message before subscribers'; the router's outcome
/// selects between running the leaf's request and discarding it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RouterOutcome {
    /// Run this semantic command and discard the leaf's message for this tick.
    #[allow(dead_code)]
    Command(Command),
    /// Run nothing and discard the leaf's message for this tick.
    Swallow,
    /// The leaf's message stands (if it produced one).
    FallThrough,
}

/// Resolve a chord against the live ordered policy. Policy layers that have
/// not yet moved their effects into the router deliberately fall through; the
/// policy still identifies their precedence and eligibility for the next
/// migration units. Blocking layers already have their ADR 0002 semantics.
pub(super) fn resolve_router_outcome(key: KeyEvent, snapshot: &RouterSnapshot) -> RouterOutcome {
    match resolve_policy(KeyChord::from_key(key), snapshot) {
        Some(entry) if entry.blocking => RouterOutcome::Swallow,
        Some(_) | None => RouterOutcome::FallThrough,
    }
}
