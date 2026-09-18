//! One Central Keyboard Router (ADR 0023).
//!
//! `UiRoot` is the single keyboard routing authority. This module resolves a
//! chord against the ordered policy and returns ADR 0002's three outcomes —
//! `Command` (run this semantic command, discard the focused leaf's message),
//! `Swallow` (run nothing, discard the leaf's message), or `FallThrough` (the
//! leaf's own typed request stands).

use crossterm::event::KeyEvent;
use mbv_core::keybinds::Keybinds;

use super::action::Command;
use super::components::ComponentId;
use super::input_resolver::KeyChord;
use super::key_policy::{command_for_policy, resolve_policy, KeyPolicyBinding};

pub(super) use super::key_policy::RouterSnapshot;

/// ADR 0002's three routing outcomes, exactly. `Application::tick` returns the
/// focused component's message before subscribers'; the router's outcome
/// selects between running the leaf's request and discarding it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RouterOutcome {
    /// Run this semantic command and discard the leaf's message for this tick.
    Command(Command),
    /// Run nothing and discard the leaf's message for this tick.
    Swallow,
    /// The leaf's message stands (if it produced one).
    FallThrough,
    /// A context-sensitive candidate resolved after leaf arbitration.
    Deferred(Command),
    /// Prefix mode (design D6, task 6.2): arm prefix mode and consume the
    /// chord. If already armed this re-arms (the double-prefix record).
    PrefixArm,
    /// Armed dispatch: a mapped prefix-namespace chord whose action's
    /// declared gate currently allows it — run the command and disarm.
    PrefixDispatch(Command),
    /// Armed chord resolved to nothing (unmapped, Escape, or a mapped chord
    /// whose gate is closed): swallow and disarm. No FallThrough exists
    /// while armed.
    PrefixSwallow,
}

/// Resolve a chord against the live ordered policy. A matched command is
/// dispatched by the shell; blocking and catch-all layers swallow the leaf.
///
/// The two-argument form treats every chord as occurring with no text entry
/// focused. Production routing goes through
/// `resolve_router_outcome_with_focused`, which carries the focused leaf so
/// the policy can tell "the leaf is the blocking overlay" from "an overlay is
/// mounted elsewhere". The rules, in the order they apply:
///
/// 0. **Armed prefix capture** (design D6, task 6.2): while
///    `snapshot.prefix_armed` is true the chord resolves against the prefix
///    namespace only (`resolve_armed_outcome`); no chord reaches any surface.
/// 1. **Never swallow the focused leaf's own typed request.** When the
///    policy would return `Swallow` and the focused leaf is the blocking
///    overlay (`snapshot.blocking_overlay_open` is true and the focused id
///    is one of the blocking-overlay `ComponentId`s), return `FallThrough`
///    so the leaf's request stands.
/// 2. **Text entry keeps ordinary characters.** When the policy matches a
///    global binding and `snapshot.text_entry_focused` is true, return
///    `FallThrough` instead of `Command` so the leaf's character input stands.
///    The F1-F4 sidebar bindings remain router-owned so sidebars switch
///    directly even while Settings or Search text entry is focused.
///
/// The `blocking_overlay_open` catch-all rules stay: they still discard the
/// focused leaf's message when no overlay is mounted.
pub(super) fn resolve_router_outcome_with_focused(
    key: KeyEvent,
    snapshot: &RouterSnapshot,
    focused: Option<&ComponentId>,
    keybinds: &Keybinds,
) -> RouterOutcome {
    let chord = KeyChord::from_key(key);
    // Armed prefix capture (design D6): while armed, every chord resolves
    // against the prefix namespace only — before text-entry, overlay, and
    // leaf arbitration, none of which can see an armed chord.
    if snapshot.prefix_armed {
        return resolve_armed_outcome(chord, snapshot, keybinds);
    }
    let focused_is_blocking_overlay =
        snapshot.blocking_overlay_open && focused.is_some_and(is_blocking_overlay);
    match resolve_policy(chord, snapshot, keybinds) {
        Some(entry) if entry.binding == KeyPolicyBinding::PrefixArm => RouterOutcome::PrefixArm,
        Some(entry) if entry.blocking => RouterOutcome::Swallow,
        Some(entry) => {
            if snapshot.text_entry_focused
                && entry.global
                && !matches!(
                    entry.binding,
                    KeyPolicyBinding::SettingsOpen
                        | KeyPolicyBinding::SessionsOpen
                        | KeyPolicyBinding::PlaylistsOpen
                        | KeyPolicyBinding::HelpOpen
                )
            {
                return RouterOutcome::FallThrough;
            }
            match command_for_policy(entry.binding, chord) {
                Some(cmd @ (Command::TogglePlayPause | Command::Stop)) => {
                    RouterOutcome::Deferred(cmd)
                }
                Some(cmd) => RouterOutcome::Command(cmd),
                None => {
                    if snapshot.blocking_overlay_open {
                        if focused_is_blocking_overlay {
                            RouterOutcome::FallThrough
                        } else {
                            RouterOutcome::Swallow
                        }
                    } else {
                        RouterOutcome::FallThrough
                    }
                }
            }
        }
        None if snapshot.blocking_overlay_open => {
            if focused_is_blocking_overlay {
                RouterOutcome::FallThrough
            } else {
                RouterOutcome::Swallow
            }
        }
        None => RouterOutcome::FallThrough,
    }
}

/// Armed-dispatch resolution (design D6, task 6.2): the next chord after the
/// prefix resolves against the prefix-namespace assignments only. The prefix
/// chord re-arms; a mapped chord fires its action only under the action's
/// normal eligibility gate (arming reuses each action's declared gate, never
/// bypasses it) and disarms; Escape or an unmapped chord — including a mapped
/// chord whose gate is closed — swallows and disarms. There is no FallThrough
/// path: while armed, no chord reaches the focused component or any surface.
pub(super) fn resolve_armed_outcome(
    chord: KeyChord,
    snapshot: &RouterSnapshot,
    keybinds: &Keybinds,
) -> RouterOutcome {
    // The prefix chord itself re-arms and stays consumed (double prefix).
    if keybinds.prefix.map(KeyChord::from_keybinds_chord) == Some(chord) {
        return RouterOutcome::PrefixArm;
    }
    let mapped = keybinds
        .prefix_assignments()
        .find(|(_, configured)| KeyChord::from_keybinds_chord(*configured) == chord)
        .map(|(action, _)| action);
    if let Some(action) = mapped {
        // Armed dispatch resolves the action through its own policy layer so
        // the declared gate and the command binding stay the registry's.
        if let Some(entry) = super::key_policy::KEY_POLICY
            .iter()
            .find(|entry| entry.name == action.id)
        {
            if entry.gate.allows(chord, snapshot) {
                if let Some(command) = command_for_policy(entry.binding, chord) {
                    return RouterOutcome::PrefixDispatch(command);
                }
            }
        }
    }
    // Unmapped, Escape, or mapped-with-closed-gate: consumed, disarms,
    // nothing executes. `stop` is not prefix-addressable (design D1), so
    // Escape can never dispatch through this path.
    RouterOutcome::PrefixSwallow
}

/// Whether a component id is one of the blocking overlays the policy mounts.
/// Mirrors the shell's `blocking_overlay_active` set so the router can tell
/// "the focused leaf is the overlay itself" from "an overlay is mounted
/// elsewhere".
pub(super) fn is_blocking_overlay(id: &ComponentId) -> bool {
    use super::components::{ModalId, OverlayId, PopupId};
    matches!(
        id,
        ComponentId::Overlay(OverlayId::ContextMenu)
            | ComponentId::Modal(ModalId::Confirm | ModalId::DaemonLost | ModalId::SavePlaylist,)
            | ComponentId::Popup(
                PopupId::Multiselect | PopupId::LibraryRoutes | PopupId::FeedManage,
            )
    )
}
