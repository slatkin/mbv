//! Central input resolution: the single, testable seam that turns a key press
//! (in a given UI context) into a semantic `Command`, a `Swallow`, or a
//! `FallThrough`. See `docs/adr/0002-centralized-input-handling.md`.
//!
//! Phase 1 (#130) covers only the Playback and Help contexts. The full
//! context-priority stack that *selects* the context arrives in phase 2 (#131).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A normalized key press: physical key code plus active modifiers, with the
/// terminal-specific `kind`/`state` fields of `KeyEvent` dropped. This is the
/// unit the resolver matches bindings against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    // Test-only constructor: production code always builds a `KeyChord` from
    // a real `KeyEvent` via `from_key`. `#[cfg(test)]` keeps it out of the
    // non-test build, where it would otherwise be unreachable dead code (see
    // `cargo clippy --all-targets -D warnings` in `docs/CHECKIN.md`).
    #[cfg(test)]
    pub(super) fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }

    pub(super) fn from_key(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            mods: key.modifiers,
        }
    }
}

use super::action::Command;
use super::App;

/// A UI context that can bind keys. Phase 1 has only the two contexts that
/// already had a pure translation seam; phase 2 (#131) adds the rest and the
/// priority stack that selects among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputContext {
    Playback,
}

/// The outcome of resolving a chord in a context.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum KeyResolution {
    /// Dispatch this semantic command.
    Command(Command),
    /// Consume the key with no action (e.g. an overlay eating unknown keys).
    #[allow(dead_code)] // constructed by future surface conversions (help was the first)
    Swallow,
    /// Decline the key; a lower-priority context (or the view handler) handles it.
    FallThrough,
}

/// The plain-data view of app state the resolver reads, so resolution stays a
/// pure function testable without constructing an `App`. Phase 1 carries only
/// the fields the Playback gate needs; phase 2 grows this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InputSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
}

/// Resolve a chord within a single context. Pure: no `App`/`Player` access.
pub(super) fn resolve_key(
    context: InputContext,
    snapshot: &InputSnapshot,
    chord: KeyChord,
) -> KeyResolution {
    match context {
        // Playback keys are gated; an unbound or gate-closed key falls through
        // to the handlers below it in `handle_key`.
        InputContext::Playback => super::action::playback_command_for_key(
            chord,
            snapshot.player_active,
            snapshot.has_remote_session,
        )
        .map_or(KeyResolution::FallThrough, KeyResolution::Command),
    }
}

impl App {
    /// Build the input snapshot from current app state. Single build-site so
    /// "what does input depend on?" has one auditable answer.
    pub(super) fn input_snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            player_active: self.player.status.lock().unwrap().active,
            // A direct daemon connection is also a valid playback route even
            // before it reports active playback; this keeps Stop available
            // while a guarded Play is resolving.
            has_remote_session: self.connected_session_id.is_some()
                || self.player.is_remote()
                || self.is_cast_attached(),
        }
    }
}

/// One layer of the keyboard precedence stack: a name for assertions/debugging
/// and a handler that returns `Some(quit)` if this context claimed the key, or
/// `None` to fall through to the next-lower-priority context. Phase 2 (#131)
/// makes `handle_key`'s branch order into this explicit, ordered, testable
/// list instead of implicit control flow.
///
/// A stack-entry handler is only meant to be invoked through `CONTEXT_STACK`
/// via `handle_key`'s loop, never called directly — direct calls would bypass
/// the explicit precedence order this stack exists to make assertable. The
/// `pub(super)` visibility on these handlers is required for the fn-pointer
/// table below, not an invitation to call them from elsewhere in `app`.
#[derive(Clone, Copy)]
pub(super) struct ContextEntry {
    // Only read by the `context_stack_order_is_pinned` characterization test
    // today; kept outside `#[cfg(test)]` since it's part of the type's
    // intended (debugging/assertion) purpose, not test-only scaffolding.
    #[allow(dead_code)]
    pub name: &'static str,
    pub handler: fn(&mut App, KeyEvent) -> Option<bool>,
}

/// The full keyboard context-priority stack, first-match-wins, in the exact
/// order `handle_key` checked them before phase 2. See
/// `docs/adr/0002-centralized-input-handling.md`.
pub(super) const CONTEXT_STACK: &[ContextEntry] = &[
    ContextEntry {
        name: "global_overlay_open",
        handler: App::handle_key_global_overlay_open,
    },
    ContextEntry {
        name: "queue_column_width",
        handler: App::handle_key_queue_column_width,
    },
    ContextEntry {
        name: "panel_mode_cycle_x",
        handler: App::handle_key_panel_mode_cycle,
    },
    ContextEntry {
        name: "confirm_skip_intro",
        handler: App::handle_key_confirm_skip_intro,
    },
    ContextEntry {
        name: "confirm_next_up",
        handler: App::handle_key_confirm_next_up,
    },
    ContextEntry {
        name: "clear_queue_prompt_c",
        handler: App::handle_key_clear_queue_prompt,
    },
    ContextEntry {
        name: "visualizer",
        handler: App::handle_key_visualizer,
    },
    ContextEntry {
        name: "playback",
        handler: App::handle_playback_key,
    },
    ContextEntry {
        name: "ctrl_l_force_clear",
        handler: App::handle_key_ctrl_l,
    },
    ContextEntry {
        name: "f5_refresh",
        handler: App::handle_key_f5_refresh,
    },
    ContextEntry {
        name: "view_dispatch",
        handler: App::handle_key_view_dispatch,
    },
];

#[cfg(test)]
#[path = "input_resolver_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "input_resolver_handle_key_tests.rs"]
mod app_level_tests;
