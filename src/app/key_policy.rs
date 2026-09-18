//! Live keyboard policy for the central router (ADR 0023).
//!
//! The policy is an ordered, pure function over a normalized chord and a
//! plain-data snapshot. It deliberately does not read TuiRealm attributes:
//! precedence belongs to the router, not to distributed component mirrors.

use super::action::{idle_feed_command_for_key, Command};
use super::input_resolver::{resolve_key, InputContext, InputSnapshot, KeyChord, KeyResolution};
use super::types_settings::{PanelFocus, PanelMode};
use crossterm::event::{KeyCode, KeyModifiers};
use mbv_core::keybinds::{action_by_id, Keybinds};

/// Plain-data state read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RouterSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
    pub connected_session_id_present: bool,
    pub queue_only_idle: bool,
    pub panel_mode: PanelMode,
    pub panel_focus: PanelFocus,
    pub blocking_overlay_open: bool,
    pub help_overlay_open: bool,
    /// Whether the (non-blocking) Sessions sidebar is mounted. When open, Esc
    /// closes it and takes precedence over the double-Escape playback stop,
    /// matching the legacy context stack (Sessions before Playback).
    pub sessions_sidebar_open: bool,
    pub context_menu_open: bool,
    pub idle_feed_link_available: bool,
    /// Whether the focused leaf is a text-entry component (the search sidebar,
    /// inline library search, or the settings form's text inputs). Global
    /// bindings do not fire while a text entry owns focus.
    pub text_entry_focused: bool,
    /// Whether any overlay/sidebar/modal holds TuiRealm focus (the shell's
    /// `overlay_holds_focus()` set). While one does, panel-focus switching is
    /// gated off: the sidebar owns the keyboard, so plain arrows must reach
    /// it (e.g. the Playlists sidebar's collapse/open) instead of moving
    /// panel focus behind it.
    pub overlay_holds_focus: bool,
}

/// One ordered layer of the keyboard policy.
#[derive(Debug, Clone)]
pub(super) struct KeyPolicyEntry {
    /// Human-readable row label for the policy table; only the tests below
    /// read it, so silence dead-code just where the tests are compiled out.
    #[cfg_attr(not(test), allow(dead_code))]
    pub name: &'static str,
    /// Whether the central router (UiRoot) owns the binding.
    pub global: bool,
    pub binding: KeyPolicyBinding,
    pub gate: KeyPolicyGate,
    pub blocking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyPolicyBinding {
    SettingsOpen,
    SessionsOpen,
    SessionsDismiss,
    PlaylistsOpen,
    SearchOpen,
    HelpOpen,
    Quit,
    NextLibraryTab,
    PreviousLibraryTab,
    LibraryTabJump,
    PanelRight,
    PanelLeft,
    AltNextLibraryTab,
    AltPreviousLibraryTab,
    AltSwallow,
    QueueColumnWidth,
    PanelModeCycle,
    ClearQueue,
    Visualizer,
    Playback,
    CtrlL,
    F5,
}

impl KeyPolicyBinding {
    /// Literal chord match for the policy entries that declare no registry
    /// action (`sessions_sidebar_escape`, `queue_column_width`, `alt_swallow`,
    /// and the `playback` transport bucket). Every declared action instead
    /// matches its configured chords from `&Keybinds` in `resolve_policy`
    /// (design D1); these keep their literals, including the swallow guard
    /// that blocks rather than commands (D9).
    fn matches(self, chord: KeyChord) -> bool {
        match self {
            Self::SessionsDismiss => chord.code == KeyCode::Esc,
            Self::AltSwallow => chord.mods.contains(KeyModifiers::ALT),
            Self::QueueColumnWidth => {
                matches!(chord.code, KeyCode::Left | KeyCode::Right)
                    && chord.mods == KeyModifiers::SHIFT
            }
            Self::Playback => true,
            _ => false,
        }
    }
}

/// Runtime condition for a policy layer. Every condition is evaluated from
/// `RouterSnapshot`; no component attribute or subscription state participates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyPolicyGate {
    NoBlockingOverlay,
    NoBlockingOverlayAndHelpClosed,
    PanelFocusQueue,
    PanelFocusLibraryBoth,
    QueueColumnWidth,
    ClearQueuePrompt,
    SessionsSidebarOpen,
    Playback,
}

impl KeyPolicyGate {
    fn allows(self, chord: KeyChord, snapshot: &RouterSnapshot) -> bool {
        match self {
            Self::NoBlockingOverlay => !snapshot.blocking_overlay_open,
            Self::NoBlockingOverlayAndHelpClosed => {
                !snapshot.blocking_overlay_open && !snapshot.help_overlay_open
            }
            Self::PanelFocusQueue => {
                !snapshot.blocking_overlay_open
                    && !snapshot.overlay_holds_focus
                    && snapshot.panel_focus == PanelFocus::Queue
            }
            Self::PanelFocusLibraryBoth => {
                !snapshot.blocking_overlay_open
                    && !snapshot.overlay_holds_focus
                    && snapshot.panel_focus == PanelFocus::Library
                    && snapshot.panel_mode == PanelMode::Both
            }
            Self::QueueColumnWidth => snapshot.panel_mode == PanelMode::Both,
            Self::ClearQueuePrompt => {
                !snapshot.blocking_overlay_open && !snapshot.context_menu_open
            }
            Self::SessionsSidebarOpen => snapshot.sessions_sidebar_open,
            Self::Playback => {
                // Playback shortcuts are single letters (space, o, m, z, a, …);
                // a focused text entry must keep them as typed characters.
                if snapshot.blocking_overlay_open || snapshot.text_entry_focused {
                    return false;
                }
                let input = InputSnapshot {
                    player_active: snapshot.player_active,
                    has_remote_session: snapshot.has_remote_session,
                };
                matches!(
                    resolve_key(InputContext::Playback, &input, chord),
                    KeyResolution::Command(_)
                ) || idle_feed_command_for_key(
                    chord,
                    snapshot.player_active,
                    snapshot.connected_session_id_present,
                    snapshot.queue_only_idle,
                    snapshot.idle_feed_link_available,
                )
                .is_some()
            }
        }
    }
}

/// The ordered keyboard policy. Entries are first-match-wins.
pub(super) const KEY_POLICY: &[KeyPolicyEntry] = &[
    KeyPolicyEntry {
        name: "settings_open",
        global: true,
        binding: KeyPolicyBinding::SettingsOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "sessions_open",
        global: true,
        binding: KeyPolicyBinding::SessionsOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "playlists_open",
        global: true,
        binding: KeyPolicyBinding::PlaylistsOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "search_open",
        global: true,
        binding: KeyPolicyBinding::SearchOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "help_open",
        global: true,
        binding: KeyPolicyBinding::HelpOpen,
        gate: KeyPolicyGate::NoBlockingOverlayAndHelpClosed,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "quit",
        global: true,
        binding: KeyPolicyBinding::Quit,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "next_library_tab",
        global: true,
        binding: KeyPolicyBinding::NextLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "previous_library_tab",
        global: true,
        binding: KeyPolicyBinding::PreviousLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "queue_column_width",
        global: false,
        binding: KeyPolicyBinding::QueueColumnWidth,
        gate: KeyPolicyGate::QueueColumnWidth,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "panel_mode_cycle_x",
        global: true,
        binding: KeyPolicyBinding::PanelModeCycle,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "clear_queue_prompt_c",
        global: true,
        binding: KeyPolicyBinding::ClearQueue,
        gate: KeyPolicyGate::ClearQueuePrompt,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "visualizer",
        global: true,
        binding: KeyPolicyBinding::Visualizer,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "sessions_sidebar_escape",
        global: false,
        binding: KeyPolicyBinding::SessionsDismiss,
        gate: KeyPolicyGate::SessionsSidebarOpen,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "playback",
        global: false,
        binding: KeyPolicyBinding::Playback,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "ctrl_l_force_clear",
        global: true,
        binding: KeyPolicyBinding::CtrlL,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "f5_refresh",
        global: true,
        binding: KeyPolicyBinding::F5,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "panel_right",
        global: true,
        binding: KeyPolicyBinding::PanelRight,
        gate: KeyPolicyGate::PanelFocusQueue,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "panel_left",
        global: true,
        binding: KeyPolicyBinding::PanelLeft,
        gate: KeyPolicyGate::PanelFocusLibraryBoth,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_previous_library_tab",
        global: true,
        binding: KeyPolicyBinding::AltPreviousLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_next_library_tab",
        global: true,
        binding: KeyPolicyBinding::AltNextLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_swallow",
        global: true,
        binding: KeyPolicyBinding::AltSwallow,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: true,
    },
    KeyPolicyEntry {
        name: "library_tab_jump",
        global: true,
        binding: KeyPolicyBinding::LibraryTabJump,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
];

/// Resolve the first policy layer that matches this chord and snapshot. A
/// declared action matches the chords configured for it in `keybinds`
/// (falling back to its declared defaults when unconfigured, design D1/D3);
/// entries without a declared action keep their literal match.
pub(super) fn resolve_policy(
    key: KeyChord,
    snapshot: &RouterSnapshot,
    keybinds: &Keybinds,
) -> Option<&'static KeyPolicyEntry> {
    KEY_POLICY
        .iter()
        .find(|entry| entry_matches(entry, key, keybinds) && entry.gate.allows(key, snapshot))
}

/// Whether a policy layer's binding matches this chord: from the declared
/// action's configured chords when the entry names one, otherwise from the
/// entry's literal match.
fn entry_matches(entry: &KeyPolicyEntry, key: KeyChord, keybinds: &Keybinds) -> bool {
    match action_by_id(entry.name) {
        Some(action) => keybinds
            .router_chords(action)
            .iter()
            .any(|configured| KeyChord::from_keybinds_chord(*configured) == key),
        None => entry.binding.matches(key),
    }
}

/// Translate a matched router binding into the semantic command it owns.
/// The `keybinds` parameter carries the loaded configuration for the
/// transport split (task 5.1), whose per-action resolution will read the
/// configured chords here the way `resolve_policy` already does.
pub(super) fn command_for_policy(
    binding: KeyPolicyBinding,
    key: KeyChord,
    snapshot: &RouterSnapshot,
    _keybinds: &Keybinds,
) -> Option<Command> {
    match binding {
        KeyPolicyBinding::SettingsOpen => Some(Command::ToggleSettings),
        KeyPolicyBinding::SessionsOpen => Some(Command::OpenSessions),
        KeyPolicyBinding::PlaylistsOpen => Some(Command::OpenPlaylists),
        KeyPolicyBinding::SearchOpen => Some(Command::OpenSearch),
        KeyPolicyBinding::HelpOpen => Some(Command::OpenHelp),
        KeyPolicyBinding::Quit => Some(Command::Quit),
        KeyPolicyBinding::NextLibraryTab | KeyPolicyBinding::AltNextLibraryTab => {
            Some(Command::NextLibraryTab)
        }
        KeyPolicyBinding::PreviousLibraryTab | KeyPolicyBinding::AltPreviousLibraryTab => {
            Some(Command::PreviousLibraryTab)
        }
        KeyPolicyBinding::LibraryTabJump => match key.code {
            KeyCode::Char(c @ '1'..='9') => {
                Some(Command::SetLibraryTab((c as usize) - '1' as usize))
            }
            _ => None,
        },
        KeyPolicyBinding::PanelRight => Some(Command::FocusPanel(PanelFocus::Library)),
        KeyPolicyBinding::PanelLeft => Some(Command::FocusPanel(PanelFocus::Queue)),
        KeyPolicyBinding::PanelModeCycle => Some(Command::CyclePanelMode),
        KeyPolicyBinding::CtrlL => Some(Command::ForceClear),
        KeyPolicyBinding::ClearQueue => Some(Command::RequestClearQueue),
        KeyPolicyBinding::F5 => Some(Command::RefreshCurrentView),
        KeyPolicyBinding::Visualizer => Some(Command::ToggleVisualizer),
        KeyPolicyBinding::Playback => {
            let command = idle_feed_command_for_key(
                key,
                snapshot.player_active,
                snapshot.connected_session_id_present,
                snapshot.queue_only_idle,
                snapshot.idle_feed_link_available,
            )
            .or_else(|| {
                let input = InputSnapshot {
                    player_active: snapshot.player_active,
                    has_remote_session: snapshot.has_remote_session,
                };
                match resolve_key(InputContext::Playback, &input, key) {
                    KeyResolution::Command(command) => Some(command),
                    KeyResolution::FallThrough | KeyResolution::Swallow => None,
                }
            })?;
            Some(command)
        }
        _ => None,
    }
}
// ---------------------------------------------------------------------------
// Mouse subscription pattern (design D8)
// ---------------------------------------------------------------------------
//
// Per-surface conversion tasks follow this pattern for mouse routing:

//
// * Each currently visible top-level region (Queue, the active Library
//   destination, an overlay) subscribes to mouse events with its own guard.
// * Every subscriber may inspect the event, but returns a message only when
//   the coordinates fall within geometry that it painted during `view()`.
// * Geometry is component-owned, so painting and hit-testing cannot drift.
// * While a blocking overlay is mounted, underlying regions receive no mouse
//   event and cannot mutate.
// * During migration, converted surfaces own mouse hit-testing and the shell
//   runs any remaining App effects. This pattern is wired per surface.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::router::RouterSnapshot;
    use mbv_core::keybinds::{Chord, KeySection, SectionBindings, KEYBIND_ACTIONS};

    fn keybinds() -> Keybinds {
        Keybinds::default()
    }

    /// A configuration with one router-scope override: `id` fires on `chord`
    /// instead of its declared default.
    fn rebound(id: &'static str, chord: &str) -> Keybinds {
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

    fn snapshot() -> RouterSnapshot {
        RouterSnapshot {
            panel_mode: PanelMode::Both,
            ..RouterSnapshot::default()
        }
    }

    fn chord(code: KeyCode, mods: KeyModifiers) -> KeyChord {
        KeyChord { code, mods }
    }

    #[test]
    fn policy_entries_have_unique_ordered_names() {
        let mut names = KEY_POLICY
            .iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
        assert_eq!(names.remove(0), "settings_open");
    }

    #[test]
    fn queue_column_width_requires_both_panels_and_shift_horizontal() {
        let key = chord(KeyCode::Left, KeyModifiers::SHIFT);
        assert_eq!(
            resolve_policy(key, &snapshot(), &keybinds()).unwrap().name,
            "queue_column_width"
        );

        let mut queue_only = snapshot();
        queue_only.panel_mode = PanelMode::QueueOnly;
        assert_ne!(
            resolve_policy(key, &queue_only, &keybinds()).map(|entry| entry.name),
            Some("queue_column_width")
        );
        assert_ne!(
            resolve_policy(
                chord(KeyCode::Left, KeyModifiers::NONE),
                &snapshot(),
                &keybinds()
            )
            .map(|entry| entry.name),
            Some("queue_column_width")
        );
    }

    #[test]
    fn panel_mode_cycle_falls_through_during_text_entry() {
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let mut text_entry = snapshot();
        text_entry.text_entry_focused = true;
        assert_eq!(
            crate::app::router::resolve_router_outcome_with_focused(
                key,
                &text_entry,
                None,
                &keybinds()
            ),
            crate::app::router::RouterOutcome::FallThrough
        );

        let normal = snapshot();
        assert_eq!(
            crate::app::router::resolve_router_outcome_with_focused(
                key,
                &normal,
                None,
                &keybinds()
            ),
            crate::app::router::RouterOutcome::Command(Command::CyclePanelMode)
        );
    }

    #[test]
    fn playback_gate_uses_per_key_resolution_and_idle_feed_path() {
        let mut active = snapshot();
        active.player_active = true;
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char(' '), KeyModifiers::NONE),
                &active,
                &keybinds()
            )
            .unwrap()
            .name,
            "playback"
        );
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('a'), KeyModifiers::CONTROL),
                &active,
                &keybinds()
            )
            .map(|entry| entry.name),
            None
        );

        let mut idle_feed = snapshot();
        idle_feed.idle_feed_link_available = true;
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('o'), KeyModifiers::NONE),
                &idle_feed,
                &keybinds()
            )
            .unwrap()
            .name,
            "playback"
        );

        // A focused text entry (e.g. Inline Search) keeps every playback letter
        // as a typed character rather than routing it to a playback command.
        idle_feed.text_entry_focused = true;
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('o'), KeyModifiers::NONE),
                &idle_feed,
                &keybinds()
            )
            .map(|entry| entry.name),
            None
        );
        let mut typing = snapshot();
        typing.player_active = true;
        typing.text_entry_focused = true;
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char(' '), KeyModifiers::NONE),
                &typing,
                &keybinds()
            )
            .map(|entry| entry.name),
            None
        );
    }

    #[test]
    fn sessions_sidebar_escape_precedes_double_escape_playback_stop() {
        let mut armed = snapshot();
        armed.player_active = true;
        assert_eq!(
            resolve_policy(chord(KeyCode::Esc, KeyModifiers::NONE), &armed, &keybinds())
                .unwrap()
                .name,
            "playback"
        );

        armed.sessions_sidebar_open = true;
        assert_eq!(
            resolve_policy(chord(KeyCode::Esc, KeyModifiers::NONE), &armed, &keybinds())
                .unwrap()
                .name,
            "sessions_sidebar_escape"
        );
        assert_eq!(
            crate::app::router::resolve_router_outcome_with_focused(
                crossterm::event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &armed,
                None,
                &keybinds()
            ),
            crate::app::router::RouterOutcome::FallThrough
        );
    }

    #[test]
    fn clear_queue_is_gated_when_context_menu_is_open() {
        let key = chord(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(
            resolve_policy(key, &snapshot(), &keybinds()).unwrap().name,
            "clear_queue_prompt_c"
        );

        let mut menu = snapshot();
        menu.context_menu_open = true;
        assert_ne!(
            resolve_policy(key, &menu, &keybinds()).map(|entry| entry.name),
            Some("clear_queue_prompt_c")
        );
    }

    #[test]
    fn rebound_action_fires_on_configured_chord_and_default_is_inert() {
        let keybinds = rebound("quit", "w");
        let new_chord = chord(KeyCode::Char('w'), KeyModifiers::NONE);

        assert_eq!(
            resolve_policy(new_chord, &snapshot(), &keybinds)
                .unwrap()
                .name,
            "quit"
        );
        assert_eq!(
            command_for_policy(KeyPolicyBinding::Quit, new_chord, &snapshot(), &keybinds),
            Some(Command::Quit)
        );

        // The declared default chord no longer reaches any policy layer.
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('q'), KeyModifiers::NONE),
                &snapshot(),
                &keybinds
            )
            .map(|entry| entry.name),
            None
        );
    }

    #[test]
    fn rebound_alias_default_collapses_to_the_configured_chord() {
        // search_open declares the Ctrl+/ and Ctrl+_ aliases; one configured
        // chord replaces both.
        let keybinds = rebound("search_open", "Ctrl+k");
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('k'), KeyModifiers::CONTROL),
                &snapshot(),
                &keybinds
            )
            .unwrap()
            .name,
            "search_open"
        );
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char('/'), KeyModifiers::CONTROL),
                &snapshot(),
                &keybinds
            )
            .map(|entry| entry.name),
            None
        );
    }

    #[test]
    fn defaults_reproduce_todays_resolution() {
        // With the default configuration every declared non-transport action
        // matches exactly its declared default chords (the transport bucket
        // still resolves per-key through the Playback gate until task 5.1).
        let keybinds = Keybinds::default();
        for action in KEYBIND_ACTIONS {
            if action.policy == "playback" {
                continue;
            }
            for chord_str in action.default_chords {
                let key = KeyChord::from_keybinds_chord(Chord::parse(chord_str).unwrap());
                let mut snap = snapshot();
                snap.panel_focus = match action.id {
                    "panel_right" => PanelFocus::Queue,
                    "panel_left" => PanelFocus::Library,
                    _ => snap.panel_focus,
                };
                assert_eq!(
                    resolve_policy(key, &snap, &keybinds).map(|entry| entry.name),
                    Some(action.id),
                    "default chord `{chord_str}` must resolve `{}` as today",
                    action.id
                );
            }
        }
    }
}
