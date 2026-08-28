//! Live keyboard policy for the central router (ADR 0023).
//!
//! The policy is an ordered, pure function over a normalized chord and a
//! plain-data snapshot. It deliberately does not read TuiRealm attributes:
//! precedence belongs to the router, not to distributed component mirrors.

use super::action::{idle_feed_command_for_key, Command};
use super::components::component_id::OverlayId;
use super::components::ComponentId;
use super::input_resolver::{resolve_key, InputContext, InputSnapshot, KeyChord, KeyResolution};
use super::types_settings::{PanelFocus, PanelMode};
use crossterm::event::{KeyCode, KeyModifiers};

/// Plain-data state read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RouterSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
    pub connected_session_id_present: bool,
    pub panel_mode: PanelMode,
    pub panel_focus: PanelFocus,
    pub blocking_overlay_open: bool,
    pub help_overlay_open: bool,
    pub selection_modal_open: bool,
    pub context_menu_open: bool,
    pub idle_feed_link_available: bool,
    /// Whether the focused leaf is a text-entry component (the search sidebar,
    /// inline library search, or the settings form's text inputs). Global
    /// bindings do not fire while a text entry owns focus.
    pub text_entry_focused: bool,
    /// Whether the previous eligible Space press is within the double-tap
    /// window. The timer remains App-owned; this is the router's plain-data
    /// view of it.
    pub space_double_tap: bool,
    /// Whether the previous eligible Esc press is within the double-tap
    /// window. The timer remains App-owned; this is the router's plain-data
    /// view of it.
    pub esc_double_tap: bool,
}

/// One ordered layer of the keyboard policy.
#[derive(Debug, Clone)]
pub(super) struct KeyPolicyEntry {
    pub name: &'static str,
    pub owner: KeyPolicyOwner,
    pub binding: KeyPolicyBinding,
    pub gate: KeyPolicyGate,
    pub blocking: bool,
}

/// The component or router surface associated with a policy layer.
#[derive(Debug, Clone)]
pub(super) enum KeyPolicyOwner {
    /// The active/focused component receives the key first.
    Active(Option<ComponentId>),
    /// The central router owns the binding.
    Sub(ComponentId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyPolicyBinding {
    Any,
    SettingsOpen,
    SessionsOpen,
    PlaylistsOpen,
    SearchOpen,
    HelpOpen,
    Quit,
    NextLibraryTab,
    PreviousLibraryTab,
    LibraryTabJump,
    AltPanelRight,
    AltPanelLeft,
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
    ViewDispatch,
}

impl KeyPolicyBinding {
    fn matches(self, chord: KeyChord) -> bool {
        match self {
            Self::Any => true,
            Self::SettingsOpen => chord.code == KeyCode::F(2),
            Self::SessionsOpen => chord.code == KeyCode::F(3),
            Self::PlaylistsOpen => chord.code == KeyCode::F(4),
            Self::SearchOpen => {
                chord.mods.contains(KeyModifiers::CONTROL)
                    && matches!(chord.code, KeyCode::Char('/') | KeyCode::Char('_'))
            }
            Self::HelpOpen => chord.code == KeyCode::F(1),
            Self::Quit => chord.code == KeyCode::Char('q') && chord.mods.is_empty(),
            Self::NextLibraryTab => chord.code == KeyCode::Tab,
            Self::PreviousLibraryTab => chord.code == KeyCode::BackTab,
            Self::LibraryTabJump => {
                matches!(chord.code, KeyCode::Char('1'..='9')) && chord.mods.is_empty()
            }
            Self::AltPanelRight => {
                chord.mods.contains(KeyModifiers::ALT) && chord.code == KeyCode::Right
            }
            Self::AltPanelLeft => {
                chord.mods.contains(KeyModifiers::ALT) && chord.code == KeyCode::Left
            }
            Self::AltNextLibraryTab => {
                chord.mods.contains(KeyModifiers::ALT) && chord.code == KeyCode::Down
            }
            Self::AltPreviousLibraryTab => {
                chord.mods.contains(KeyModifiers::ALT) && chord.code == KeyCode::Up
            }
            Self::AltSwallow => chord.mods.contains(KeyModifiers::ALT),
            Self::QueueColumnWidth => {
                matches!(chord.code, KeyCode::Left | KeyCode::Right)
                    && chord.mods == KeyModifiers::SHIFT
            }
            Self::PanelModeCycle => chord.code == KeyCode::Char('x') && chord.mods.is_empty(),
            Self::ClearQueue => {
                chord.code == KeyCode::Char('c') && !chord.mods.contains(KeyModifiers::ALT)
            }
            Self::Visualizer => chord.code == KeyCode::Char('v') && chord.mods.is_empty(),
            Self::Playback => true,
            Self::CtrlL => {
                chord.code == KeyCode::Char('l') && chord.mods.contains(KeyModifiers::CONTROL)
            }
            Self::F5 => chord.code == KeyCode::F(5),
            Self::ViewDispatch => false,
        }
    }
}

/// Runtime condition for a policy layer. Every condition is evaluated from
/// `RouterSnapshot`; no component attribute or subscription state participates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyPolicyGate {
    Always,
    SelectionModal,
    NoBlockingOverlay,
    NoBlockingOverlayAndHelpClosed,
    PanelFocusQueue,
    PanelFocusLibraryBoth,
    QueueColumnWidth,
    NoContextMenu,
    Playback,
}

impl KeyPolicyGate {
    fn allows(self, chord: KeyChord, snapshot: &RouterSnapshot) -> bool {
        match self {
            Self::Always => true,
            Self::SelectionModal => snapshot.selection_modal_open,
            Self::NoBlockingOverlay => !snapshot.blocking_overlay_open,
            Self::NoBlockingOverlayAndHelpClosed => {
                !snapshot.blocking_overlay_open && !snapshot.help_overlay_open
            }
            Self::PanelFocusQueue => {
                !snapshot.blocking_overlay_open && snapshot.panel_focus == PanelFocus::Queue
            }
            Self::PanelFocusLibraryBoth => {
                !snapshot.blocking_overlay_open
                    && snapshot.panel_focus == PanelFocus::Library
                    && snapshot.panel_mode == PanelMode::Both
            }
            Self::QueueColumnWidth => snapshot.panel_mode == PanelMode::Both,
            Self::NoContextMenu => !snapshot.context_menu_open,
            Self::Playback => {
                if snapshot.blocking_overlay_open {
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
        name: "selection_modal",
        owner: KeyPolicyOwner::Active(Some(ComponentId::Overlay(OverlayId::SelectionModal))),
        binding: KeyPolicyBinding::Any,
        gate: KeyPolicyGate::SelectionModal,
        blocking: true,
    },
    KeyPolicyEntry {
        name: "settings_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::SettingsOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "sessions_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::SessionsOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "playlists_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::PlaylistsOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "search_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::SearchOpen,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "help_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::HelpOpen,
        gate: KeyPolicyGate::NoBlockingOverlayAndHelpClosed,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "quit",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::Quit,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "next_library_tab",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::NextLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "previous_library_tab",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::PreviousLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "queue_column_width",
        owner: KeyPolicyOwner::Sub(ComponentId::Queue),
        binding: KeyPolicyBinding::QueueColumnWidth,
        gate: KeyPolicyGate::QueueColumnWidth,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "panel_mode_cycle_x",
        owner: KeyPolicyOwner::Sub(ComponentId::Library),
        binding: KeyPolicyBinding::PanelModeCycle,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "clear_queue_prompt_c",
        owner: KeyPolicyOwner::Sub(ComponentId::Queue),
        binding: KeyPolicyBinding::ClearQueue,
        gate: KeyPolicyGate::NoContextMenu,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "visualizer",
        owner: KeyPolicyOwner::Sub(ComponentId::Playback),
        binding: KeyPolicyBinding::Visualizer,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "playback",
        owner: KeyPolicyOwner::Sub(ComponentId::Playback),
        binding: KeyPolicyBinding::Playback,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "ctrl_l_force_clear",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::CtrlL,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "f5_refresh",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::F5,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_panel_right",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::AltPanelRight,
        gate: KeyPolicyGate::PanelFocusQueue,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_panel_left",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::AltPanelLeft,
        gate: KeyPolicyGate::PanelFocusLibraryBoth,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_previous_library_tab",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::AltPreviousLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_next_library_tab",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::AltNextLibraryTab,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "alt_swallow",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::AltSwallow,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: true,
    },
    KeyPolicyEntry {
        name: "library_tab_jump",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::LibraryTabJump,
        gate: KeyPolicyGate::NoBlockingOverlay,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "view_dispatch",
        owner: KeyPolicyOwner::Active(None),
        binding: KeyPolicyBinding::ViewDispatch,
        gate: KeyPolicyGate::Always,
        blocking: false,
    },
];

/// Resolve the first policy layer that matches this chord and snapshot.
pub(super) fn resolve_policy(
    key: KeyChord,
    snapshot: &RouterSnapshot,
) -> Option<&'static KeyPolicyEntry> {
    KEY_POLICY
        .iter()
        .find(|entry| entry.binding.matches(key) && entry.gate.allows(key, snapshot))
}

/// Translate a matched router binding into the semantic command it owns.
pub(super) fn command_for_policy(
    binding: KeyPolicyBinding,
    key: KeyChord,
    snapshot: &RouterSnapshot,
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
        KeyPolicyBinding::AltPanelRight => Some(Command::FocusPanel(PanelFocus::Library)),
        KeyPolicyBinding::AltPanelLeft => Some(Command::FocusPanel(PanelFocus::Queue)),
        KeyPolicyBinding::PanelModeCycle => Some(Command::CyclePanelMode),
        KeyPolicyBinding::CtrlL => Some(Command::ForceClear),
        KeyPolicyBinding::F5 => Some(Command::RefreshCurrentView),
        KeyPolicyBinding::Visualizer => Some(Command::ToggleVisualizer),
        KeyPolicyBinding::Playback => {
            let command = idle_feed_command_for_key(
                key,
                snapshot.player_active,
                snapshot.connected_session_id_present,
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
            match command {
                Command::TogglePlayPause if !snapshot.space_double_tap => None,
                Command::Stop if !snapshot.esc_double_tap => None,
                command => Some(command),
            }
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
        assert_eq!(names.remove(0), "selection_modal");
        assert_eq!(names.last(), Some(&"view_dispatch"));
    }

    #[test]
    fn queue_column_width_requires_both_panels_and_shift_horizontal() {
        let key = chord(KeyCode::Left, KeyModifiers::SHIFT);
        assert_eq!(
            resolve_policy(key, &snapshot()).unwrap().name,
            "queue_column_width"
        );

        let mut queue_only = snapshot();
        queue_only.panel_mode = PanelMode::QueueOnly;
        assert_ne!(
            resolve_policy(key, &queue_only).map(|entry| entry.name),
            Some("queue_column_width")
        );
        assert_ne!(
            resolve_policy(chord(KeyCode::Left, KeyModifiers::NONE), &snapshot())
                .map(|entry| entry.name),
            Some("queue_column_width")
        );
    }

    #[test]
    fn playback_gate_uses_per_key_resolution_and_idle_feed_path() {
        let mut active = snapshot();
        active.player_active = true;
        assert_eq!(
            resolve_policy(chord(KeyCode::Char(' '), KeyModifiers::NONE), &active)
                .unwrap()
                .name,
            "playback"
        );
        assert_eq!(
            resolve_policy(chord(KeyCode::Char('a'), KeyModifiers::CONTROL), &active)
                .map(|entry| entry.name),
            None
        );

        let mut idle_feed = snapshot();
        idle_feed.idle_feed_link_available = true;
        assert_eq!(
            resolve_policy(chord(KeyCode::Char('o'), KeyModifiers::NONE), &idle_feed)
                .unwrap()
                .name,
            "playback"
        );
    }

    #[test]
    fn clear_queue_is_gated_when_context_menu_is_open() {
        let key = chord(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(
            resolve_policy(key, &snapshot()).unwrap().name,
            "clear_queue_prompt_c"
        );

        let mut menu = snapshot();
        menu.context_menu_open = true;
        assert_ne!(
            resolve_policy(key, &menu).map(|entry| entry.name),
            Some("clear_queue_prompt_c")
        );
    }
}
