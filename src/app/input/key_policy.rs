//! Live keyboard policy for the central router (ADR 0023).
//!
//! The policy is an ordered, pure function over a normalized chord and a
//! plain-data snapshot. It deliberately does not read TuiRealm attributes:
//! precedence belongs to the router, not to distributed component mirrors.

use super::resolver::KeyChord;
use crate::app::dispatch::action::{
    idle_feed_command_for_key, Command, IdleFeedLinkContext, VOLUME_STEP,
};
use crate::app::state::types::settings::{PanelFocus, PanelMode};
use crossterm::event::{KeyCode, KeyModifiers};
use mbv_keybinds::{action_by_id, Keybinds};

/// Which attached playback target currently owns remote-control shortcuts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) enum RemotePlaybackTarget {
    #[default]
    None,
    Session,
    DirectRemote,
    Cast,
}

/// Playback facts read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) struct RouterPlaybackState {
    pub player_active: bool,
    pub remote_target: RemotePlaybackTarget,
    pub queue_only_idle: bool,
    pub idle_feed_link_available: bool,
}

/// The overlay-focus situation read by the central keyboard policy.
///
/// The policy only ever distinguishes four reachable situations. `blocking`
/// implies `holds_focus` (the blocking set is a subset of the focus-holding
/// set) and the context menu is itself a blocking overlay, so the three facts
/// are not independent and are carried as one ordered enum instead of three
/// settable bools. `help`/`sessions_sidebar`/`text_entry_focused` stay
/// independent and live on `RouterOverlayState`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) enum OverlayFocus {
    /// No overlay holds TuiRealm focus; the panels own the keyboard.
    #[default]
    Free,
    /// A non-blocking overlay (Help, a sidebar) holds focus.
    NonBlocking,
    /// A blocking overlay other than the context menu holds focus.
    Blocking,
    /// The context menu is open (it is a blocking overlay the policy names
    /// explicitly, so the `ClearQueuePrompt` gate can read it directly).
    ContextMenu,
}

impl OverlayFocus {
    /// Whether a blocking overlay is mounted.
    pub(in crate::app) fn blocking(self) -> bool {
        matches!(self, Self::Blocking | Self::ContextMenu)
    }

    /// Whether any overlay/sidebar/modal holds TuiRealm focus.
    pub(in crate::app) fn holds_focus(self) -> bool {
        !matches!(self, Self::Free)
    }

    /// Whether the context menu is open.
    pub(in crate::app) fn context_menu(self) -> bool {
        matches!(self, Self::ContextMenu)
    }
}

/// Overlay facts read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) struct RouterOverlayState {
    /// Which overlay-focus situation the keyboard sees.
    pub focus: OverlayFocus,
    /// Whether the (non-blocking) Help overlay is mounted.
    pub help: bool,
    /// Whether the (non-blocking) Sessions sidebar is mounted. When open, Esc
    /// closes it and takes precedence over the Escape playback stop.
    pub sessions_sidebar: bool,
    /// Whether the focused leaf is a text-entry component (the search sidebar,
    /// inline library search, or the settings form's text inputs). Global
    /// bindings do not fire while a text entry owns focus.
    pub text_entry_focused: bool,
}

/// Plain-data state read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) struct RouterSnapshot {
    pub playback: RouterPlaybackState,
    pub overlays: RouterOverlayState,
    pub panel_mode: PanelMode,
    pub panel_focus: PanelFocus,
    /// Whether prefix mode is armed (change `add-configurable-keybinds`,
    /// design D6, task 6.1): the mirror of the App-owned bit. While true, the
    /// next chord resolves against the prefix namespace only; arming is
    /// suppressed while a text entry owns focus or a blocking overlay is
    /// open (the `prefix_arm` layer's gate).
    pub prefix_armed: bool,
}

/// One ordered layer of the keyboard policy.
#[derive(Debug, Clone)]
pub(in crate::app) struct KeyPolicyEntry {
    /// Human-readable row label for the policy table; only the tests below
    /// read it, so silence dead-code just where the tests are compiled out.
    pub name: &'static str,
    /// Whether the central router (UiRoot) owns the binding.
    pub global: bool,
    pub binding: KeyPolicyBinding,
    pub gate: KeyPolicyGate,
    pub blocking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum KeyPolicyBinding {
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
    HideVisualSlot,
    TogglePlayPause,
    Stop,
    SeekBack,
    SeekForward,
    NextTrack,
    PreviousTrack,
    VolumeDown,
    VolumeUp,
    ToggleMute,
    ToggleMuteOrCycleAudio,
    CycleSubtitle,
    OpenIdleFeedLink,
    CtrlL,
    F5,
    /// The prefix-arming layer's identity (design D6): matches the
    /// configured prefix chord, not a literal — the chord is resolved
    /// against `&Keybinds` in `entry_matches` because the layer is not a
    /// registry action (D9). Always blocking: arming swallows.
    PrefixArm,
}

impl KeyPolicyBinding {
    /// Literal chord match for the policy entries that declare no registry
    /// action (`sessions_sidebar_escape`, `queue_column_width`, and the
    /// blocking `alt_swallow`). Every declared action — the split transport
    /// set included — instead matches its configured chords from `&Keybinds`
    /// in `resolve_policy` (design D1/D2); these keep their literals,
    /// including the swallow guard that blocks rather than commands (D9).
    fn matches(self, chord: KeyChord) -> bool {
        match self {
            Self::SessionsDismiss => chord.code == KeyCode::Esc,
            Self::AltSwallow => chord.mods.contains(KeyModifiers::ALT),
            Self::QueueColumnWidth => {
                matches!(chord.code, KeyCode::Left | KeyCode::Right)
                    && chord.mods == KeyModifiers::SHIFT
            }
            _ => false,
        }
    }
}

/// Runtime condition for a policy layer. Every condition is evaluated from
/// `RouterSnapshot`; no component attribute or subscription state participates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum KeyPolicyGate {
    NoBlockingOverlay,
    NoBlockingOverlayAndHelpClosed,
    PanelFocusQueue,
    PanelFocusLibraryBoth,
    QueueColumnWidth,
    ClearQueuePrompt,
    SessionsSidebarOpen,
    /// Transport keys whose eligibility requires an active player or a
    /// remote session (Space, Esc, `<`/`>`, N, P, `a` today). The shared
    /// `Playback` label is the routing bucket, not one uniform condition —
    /// each split action carries the condition its key had before the
    /// bucket split (task 5.1, design D2).
    Playback,
    /// Transport keys with no session/activity condition (`z`, `m`, `-`,
    /// `+`/`=` today). Modifier exclusions are structural: exact-chord
    /// matching makes Ctrl+chords inert (design D3).
    PlaybackUngated,
    /// The idle-feed link shortcut (`o`): its own availability condition,
    /// owned by `idle_feed_command_for_key`.
    IdleFeedLink,
    /// Prefix arming (design D6, task 6.2): the prefix chord must not arm
    /// while a text-entry surface owns focus or a blocking overlay is open;
    /// in those states it routes as it does without the change.
    PrefixArming,
}

impl KeyPolicyGate {
    pub(in crate::app) fn allows(self, chord: KeyChord, snapshot: &RouterSnapshot) -> bool {
        match self {
            Self::NoBlockingOverlay => !snapshot.overlays.focus.blocking(),
            Self::NoBlockingOverlayAndHelpClosed => {
                !snapshot.overlays.focus.blocking() && !snapshot.overlays.help
            }
            Self::PanelFocusQueue => panel_focus_queue_allowed(snapshot),
            Self::PanelFocusLibraryBoth => panel_focus_library_both_allowed(snapshot),
            Self::QueueColumnWidth => snapshot.panel_mode == PanelMode::Both,
            Self::ClearQueuePrompt => {
                !snapshot.overlays.focus.blocking() && !snapshot.overlays.focus.context_menu()
            }
            Self::SessionsSidebarOpen => snapshot.overlays.sessions_sidebar,
            Self::Playback => playback_allowed(snapshot),
            Self::PlaybackUngated | Self::PrefixArming => {
                !snapshot.overlays.focus.blocking() && !snapshot.overlays.text_entry_focused
            }
            Self::IdleFeedLink => idle_feed_link_allowed(chord, snapshot),
        }
    }
}

fn panel_focus_queue_allowed(snapshot: &RouterSnapshot) -> bool {
    !snapshot.overlays.focus.blocking()
        && !snapshot.overlays.focus.holds_focus()
        && snapshot.panel_focus == PanelFocus::Queue
}

fn panel_focus_library_both_allowed(snapshot: &RouterSnapshot) -> bool {
    !snapshot.overlays.focus.blocking()
        && !snapshot.overlays.focus.holds_focus()
        && snapshot.panel_focus == PanelFocus::Library
        && snapshot.panel_mode == PanelMode::Both
}

fn playback_allowed(snapshot: &RouterSnapshot) -> bool {
    // Playback shortcuts are single letters (space, a, …); a focused text
    // entry must keep them as typed characters.
    !snapshot.overlays.focus.blocking()
        && !snapshot.overlays.text_entry_focused
        && (snapshot.playback.player_active
            || snapshot.playback.remote_target != RemotePlaybackTarget::None)
}

fn idle_feed_link_allowed(chord: KeyChord, snapshot: &RouterSnapshot) -> bool {
    !snapshot.overlays.focus.blocking()
        && !snapshot.overlays.text_entry_focused
        && idle_feed_command_for_key(
            chord,
            &IdleFeedLinkContext {
                player_active: snapshot.playback.player_active,
                has_connected_session: snapshot.playback.remote_target
                    == RemotePlaybackTarget::Session,
                playback_panel_present_idle: snapshot.playback.queue_only_idle,
                link_available: snapshot.playback.idle_feed_link_available,
            },
        )
        .is_some()
}

/// The ordered keyboard policy. Entries are first-match-wins.
pub(in crate::app) const KEY_POLICY: &[KeyPolicyEntry] = &[
    // Prefix arming (design D6, task 6.2): the top layer. Its chord is
    // validated at load never to collide with any configured or declared
    // binding, so first-match order cannot shadow another entry; a closed
    // gate (text entry / blocking overlay) falls through to the ordinary
    // layers below, which is exactly the unconfigured behavior.
    KeyPolicyEntry {
        name: "prefix_arm",
        global: true,
        binding: KeyPolicyBinding::PrefixArm,
        gate: KeyPolicyGate::PrefixArming,
        blocking: true,
    },
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
        name: "hide_visual_slot",
        global: true,
        binding: KeyPolicyBinding::HideVisualSlot,
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
    // The split transport set (task 5.1, design D2): one entry per registry
    // action, each named by its action id so chord matching resolves through
    // the registry, each carrying its own eligibility gate. Order below the
    // sessions sidebar escape preserves today's precedence (Esc closes the
    // sidebar before it stops playback); among the transport entries the
    // order is immaterial — chords are distinct under exact matching and
    // collisions are rejected at load.
    KeyPolicyEntry {
        name: "toggle_play_pause",
        global: false,
        binding: KeyPolicyBinding::TogglePlayPause,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "stop",
        global: false,
        binding: KeyPolicyBinding::Stop,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "seek_back",
        global: false,
        binding: KeyPolicyBinding::SeekBack,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "seek_forward",
        global: false,
        binding: KeyPolicyBinding::SeekForward,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "next_track",
        global: false,
        binding: KeyPolicyBinding::NextTrack,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "previous_track",
        global: false,
        binding: KeyPolicyBinding::PreviousTrack,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "volume_down",
        global: false,
        binding: KeyPolicyBinding::VolumeDown,
        gate: KeyPolicyGate::PlaybackUngated,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "volume_up",
        global: false,
        binding: KeyPolicyBinding::VolumeUp,
        gate: KeyPolicyGate::PlaybackUngated,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "toggle_mute",
        global: false,
        binding: KeyPolicyBinding::ToggleMute,
        gate: KeyPolicyGate::PlaybackUngated,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "toggle_mute_or_cycle_audio",
        global: false,
        binding: KeyPolicyBinding::ToggleMuteOrCycleAudio,
        gate: KeyPolicyGate::Playback,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "cycle_subtitle",
        global: false,
        binding: KeyPolicyBinding::CycleSubtitle,
        gate: KeyPolicyGate::PlaybackUngated,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "open_idle_feed_link",
        global: false,
        binding: KeyPolicyBinding::OpenIdleFeedLink,
        gate: KeyPolicyGate::IdleFeedLink,
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
pub(in crate::app) fn resolve_policy(
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
    // The prefix-arming layer is not a registry action (D9): it matches the
    // configured prefix chord, which no literal or registry lookup carries.
    if entry.binding == KeyPolicyBinding::PrefixArm {
        return keybinds.prefix.map(KeyChord::from_keybinds_chord) == Some(key);
    }
    match action_by_id(entry.name) {
        Some(action) => keybinds
            .router_chords(action)
            .iter()
            .any(|configured| KeyChord::from_keybinds_chord(*configured) == key),
        None => entry.binding.matches(key),
    }
}

/// Translate a matched router binding into the semantic command it owns.
/// The transport split (task 5.1) binds each payload-carrying variant to its
/// fixed call-site value here — the single site the old
/// `playback_command_for_key` table bound them — so the eligibility gate and
/// registry chord matching in `resolve_policy` fully decide whether a
/// command dispatches.
pub(in crate::app) fn command_for_policy(
    binding: KeyPolicyBinding,
    key: KeyChord,
) -> Option<Command> {
    command_for_simple_binding(binding)
        .or_else(|| command_for_navigation_binding(binding, key))
        .or_else(|| command_for_transport_binding(binding))
}

fn command_for_simple_binding(binding: KeyPolicyBinding) -> Option<Command> {
    match binding {
        KeyPolicyBinding::SettingsOpen => Some(Command::ToggleSettings),
        KeyPolicyBinding::SessionsOpen => Some(Command::OpenSessions),
        KeyPolicyBinding::PlaylistsOpen => Some(Command::OpenPlaylists),
        KeyPolicyBinding::SearchOpen => Some(Command::OpenSearch),
        KeyPolicyBinding::HelpOpen => Some(Command::OpenHelp),
        KeyPolicyBinding::Quit => Some(Command::Quit),
        KeyPolicyBinding::PanelRight => Some(Command::FocusPanel(PanelFocus::Library)),
        KeyPolicyBinding::PanelLeft => Some(Command::FocusPanel(PanelFocus::Queue)),
        KeyPolicyBinding::PanelModeCycle => Some(Command::CyclePanelMode),
        KeyPolicyBinding::CtrlL => Some(Command::ForceClear),
        KeyPolicyBinding::ClearQueue => Some(Command::RequestClearQueue),
        KeyPolicyBinding::F5 => Some(Command::RefreshCurrentView),
        KeyPolicyBinding::Visualizer => Some(Command::ToggleVisualizer),
        KeyPolicyBinding::HideVisualSlot => Some(Command::ToggleVisualSlotHidden),
        _ => None,
    }
}

fn command_for_navigation_binding(binding: KeyPolicyBinding, key: KeyChord) -> Option<Command> {
    match binding {
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
        _ => None,
    }
}

fn command_for_transport_binding(binding: KeyPolicyBinding) -> Option<Command> {
    // The split transport set: each action binds the fixed payload its key's
    // call site carried before the bucket split (design D2).
    match binding {
        KeyPolicyBinding::TogglePlayPause => Some(Command::TogglePlayPause),
        KeyPolicyBinding::Stop => Some(Command::Stop),
        KeyPolicyBinding::SeekBack => Some(Command::SeekRelative(-5.0)),
        KeyPolicyBinding::SeekForward => Some(Command::SeekRelative(5.0)),
        KeyPolicyBinding::NextTrack => Some(Command::NextTrack),
        KeyPolicyBinding::PreviousTrack => Some(Command::PreviousTrack),
        KeyPolicyBinding::VolumeDown => Some(Command::AdjustVolume(-VOLUME_STEP)),
        KeyPolicyBinding::VolumeUp => Some(Command::AdjustVolume(VOLUME_STEP)),
        KeyPolicyBinding::ToggleMute => Some(Command::ToggleMute),
        KeyPolicyBinding::ToggleMuteOrCycleAudio => Some(Command::ToggleMuteOrCycleAudio),
        KeyPolicyBinding::CycleSubtitle => Some(Command::CycleOrToggleSubtitle),
        KeyPolicyBinding::OpenIdleFeedLink => Some(Command::OpenIdleFeedLink),
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
mod prefix_tests;
#[cfg(test)]
mod resolution_tests;
#[cfg(test)]
mod test_support;
