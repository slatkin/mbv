//! One Central Keyboard Router (ADR 0023).
//!
//! `UiRoot` is the single keyboard routing authority. This module defines the
//! router's resolution function: given a key chord and a plain-data snapshot
//! of shell state, it returns ADR 0002's three outcomes — `Command` (run this
//! semantic command, discard the focused leaf's message), `Swallow` (run
//! nothing, discard the leaf's message), or `FallThrough` (the leaf's own
//! typed request stands).
//!
//! The policy is deliberately empty until section 4 activates `key_policy.rs`
//! as the live ordered policy. Every chord currently resolves `FallThrough`,
//! so `handle_legacy_key` still runs and behavior is unchanged.

use crossterm::event::KeyEvent;

use super::action::Command;

/// ADR 0002's three routing outcomes, exactly. `Application::tick` returns
/// the focused component's message before subscribers'; the router's outcome
/// selects between running the leaf's request and discarding it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RouterOutcome {
    /// Run this semantic command and discard the leaf's message for this tick.
    // Section 4 activates the live policy; until then the empty policy only
    // constructs `FallThrough`, so the other two variants are unconstructed
    // dead code by design (ADR 0023 seam, task 2.1).
    #[allow(dead_code)]
    Command(Command),
    /// Run nothing and discard the leaf's message for this tick.
    #[allow(dead_code)]
    Swallow,
    /// The leaf's message stands (if it produced one).
    FallThrough,
}

/// The plain-data snapshot the router's policy reads. Grown from ADR 0002's
/// `InputSnapshot` as the current `CONTEXT_STACK` gates activate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RouterSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
}

/// Resolve a chord against the ordered policy. Empty policy for now: every
/// chord falls through, so the focused leaf's interpretation stands and the
/// legacy `handle_legacy_key` path still runs unchanged.
pub(super) fn resolve_router_outcome(_key: KeyEvent, _snapshot: &RouterSnapshot) -> RouterOutcome {
    RouterOutcome::FallThrough
}
