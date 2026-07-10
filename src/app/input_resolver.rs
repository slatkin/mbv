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
    Help,
    Playback,
}

/// The outcome of resolving a chord in a context.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum KeyResolution {
    /// Dispatch this semantic command.
    Command(Command),
    /// Consume the key with no action (e.g. an overlay eating unknown keys).
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
        // The help overlay consumes every key: bound keys become commands,
        // everything else is swallowed (never falls through).
        InputContext::Help => match super::action::help_command_for_key(chord) {
            Some(cmd) => KeyResolution::Command(cmd),
            None => KeyResolution::Swallow,
        },
        // Playback keys are gated; an unbound or gate-closed key falls through
        // to the handlers below it in `handle_key`.
        InputContext::Playback => {
            match super::action::playback_command_for_key(
                chord,
                snapshot.player_active,
                snapshot.has_remote_session,
            ) {
                Some(cmd) => KeyResolution::Command(cmd),
                None => KeyResolution::FallThrough,
            }
        }
    }
}

impl App {
    /// Build the input snapshot from current app state. Single build-site so
    /// "what does input depend on?" has one auditable answer.
    pub(super) fn input_snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            player_active: self.player.status.lock().unwrap().active,
            has_remote_session: self.connected_session_id.is_some(),
        }
    }
}

/// One layer of the keyboard precedence stack: a name for assertions/debugging
/// and a handler that returns `Some(quit)` if this context claimed the key, or
/// `None` to fall through to the next-lower-priority context. Phase 2 (#131)
/// makes `handle_key`'s branch order into this explicit, ordered, testable
/// list instead of implicit control flow.
#[derive(Clone, Copy)]
/// A stack-entry handler is only meant to be invoked through `CONTEXT_STACK`
/// via `handle_key`'s loop, never called directly — direct calls would bypass
/// the explicit precedence order this stack exists to make assertable. The
/// `pub(super)` visibility on these handlers is required for the fn-pointer
/// table below, not an invitation to call them from elsewhere in `app`.
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
        name: "save_modal",
        handler: App::handle_key_save_modal,
    },
    ContextEntry {
        name: "save_playlist",
        handler: App::handle_key_save_playlist_entry,
    },
    ContextEntry {
        name: "settings",
        handler: App::handle_key_settings,
    },
    ContextEntry {
        name: "help",
        handler: App::handle_key_help,
    },
    ContextEntry {
        name: "sessions",
        handler: App::handle_key_sessions,
    },
    ContextEntry {
        name: "playlists",
        handler: App::handle_key_playlists,
    },
    ContextEntry {
        name: "legacy_tail",
        handler: App::handle_key_legacy_tail,
    },
    ContextEntry {
        name: "context_menu",
        handler: App::handle_key_context_menu,
    },
    ContextEntry {
        name: "power_queue_alt_m",
        handler: App::handle_key_power_queue_alt_m,
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
mod tests {
    use super::*;
    use crate::app::action::Command;

    fn snap(active: bool, remote: bool) -> InputSnapshot {
        InputSnapshot {
            player_active: active,
            has_remote_session: remote,
        }
    }

    #[test]
    fn help_context_maps_bound_key_to_command() {
        let r = resolve_key(
            InputContext::Help,
            &snap(false, false),
            KeyChord::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Command(Command::CloseHelp));
    }

    #[test]
    fn help_context_swallows_unbound_key() {
        // The help overlay consumes every key while open.
        let r = resolve_key(
            InputContext::Help,
            &snap(false, false),
            KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Swallow);
    }

    #[test]
    fn playback_context_maps_gated_key_to_command_when_active() {
        let r = resolve_key(
            InputContext::Playback,
            &snap(true, false),
            KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Command(Command::TogglePlayPause));
    }

    #[test]
    fn playback_context_falls_through_when_gate_closed() {
        // Space is a no-op that must reach the view handler when nothing plays.
        let r = resolve_key(
            InputContext::Playback,
            &snap(false, false),
            KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::FallThrough);
    }

    #[test]
    fn playback_context_falls_through_on_unbound_key() {
        let r = resolve_key(
            InputContext::Playback,
            &snap(true, false),
            KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::FallThrough);
    }
}

#[cfg(test)]
mod app_level_tests {
    use crate::app::tests::make_app_stub;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use mbv_core::player::PlayerCommand;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn help_f1_closes_help_via_handle_key() {
        let mut app = make_app_stub();
        app.show_help = true;
        let quit = app.handle_key(ev(KeyCode::F(1), KeyModifiers::NONE));
        assert!(!quit);
        assert!(!app.show_help, "F1 closes the help overlay");
    }

    #[test]
    fn help_swallows_unbound_key_via_handle_key() {
        let mut app = make_app_stub();
        app.show_help = true;
        let quit = app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(!quit);
        assert!(
            app.show_help,
            "an unbound key is swallowed; help stays open"
        );
    }

    #[test]
    fn space_toggles_pause_when_active_via_handle_key() {
        let mut app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
        }
        let rx = app.player.spy_on_commands();
        app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    }

    #[test]
    fn space_does_not_toggle_pause_when_idle_via_handle_key() {
        let mut app = make_app_stub();
        let rx = app.player.spy_on_commands();
        // Idle home tab: Space must not emit a transport command (it falls
        // through to the view handler, which ignores it).
        app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(
            !matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)),
            "Space is inert while nothing plays"
        );
    }

    #[test]
    fn context_stack_order_is_pinned() {
        let names: Vec<&str> = super::CONTEXT_STACK.iter().map(|e| e.name).collect();
        assert_eq!(
            names,
            vec![
                "save_modal",
                "save_playlist",
                "settings",
                "help",
                "sessions",
                "playlists",
                "legacy_tail",
                "context_menu",
                "power_queue_alt_m",
                "playback",
                "ctrl_l_force_clear",
                "f5_refresh",
                "view_dispatch",
            ],
            "precedence order must match handle_key's pre-phase-2 branch order; \
             if this intentionally changes, update docs/adr/0002-centralized-input-handling.md too"
        );
    }
}
