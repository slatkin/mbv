//! Live keyboard policy for the central router (ADR 0023).
//!
//! The policy is an ordered, pure function over a normalized chord and a
//! plain-data snapshot. It deliberately does not read TuiRealm attributes:
//! precedence belongs to the router, not to distributed component mirrors.

use super::action::idle_feed_command_for_key;
use super::components::component_id::OverlayId;
use super::components::ComponentId;
use super::input_resolver::{resolve_key, InputContext, InputSnapshot, KeyChord, KeyResolution};
use super::types_settings::PanelMode;
use crossterm::event::{KeyCode, KeyModifiers};

/// Plain-data state read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RouterSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
    pub panel_mode: PanelMode,
    pub blocking_overlay_open: bool,
    pub selection_modal_open: bool,
    pub context_menu_open: bool,
    pub idle_feed_link_available: bool,
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

/// Key shape associated with a policy layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyPolicyBinding {
    Any,
    GlobalOverlayOpen,
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
            Self::GlobalOverlayOpen => {
                matches!(chord.code, KeyCode::F(2) | KeyCode::F(3) | KeyCode::F(4))
                    || (chord.mods.contains(KeyModifiers::CONTROL)
                        && matches!(chord.code, KeyCode::Char('/') | KeyCode::Char('_')))
            }
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
            Self::QueueColumnWidth => snapshot.panel_mode == PanelMode::Both,
            Self::NoContextMenu => !snapshot.context_menu_open,
            Self::Playback => {
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
                    snapshot.has_remote_session,
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
        name: "global_overlay_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::GlobalOverlayOpen,
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
        gate: KeyPolicyGate::Always,
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
        gate: KeyPolicyGate::Always,
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
        gate: KeyPolicyGate::Always,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "f5_refresh",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        binding: KeyPolicyBinding::F5,
        gate: KeyPolicyGate::Always,
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
