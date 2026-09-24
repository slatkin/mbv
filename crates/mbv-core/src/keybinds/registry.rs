//! Declared keybind action registry — the single source of truth for every
//! configurable keyboard action (change `add-configurable-keybinds`, D1).
//!
//! One table (`KEYBIND_ACTIONS`) declares each configurable action with its
//! stable id (reusing `KeyPolicyEntry.name` so routing, config keys, and
//! presentation share one vocabulary), its owning `KeySection`, its default
//! chords, the gate bucket that limits eligibility, and whether it may be
//! rebound in router scope or addressed in the prefix namespace.
//!
//! Consumers that must derive from this declaration and never spell a chord
//! or action name themselves: `Keybinds::defaults()`, config parsing and
//! load-time validation (`load`), policy chord matching, and the help /
//! settings-Keys presentation. Entries that are not configurable actions
//! (`alt_swallow`, `queue_column_width`, `sessions_sidebar_escape`) are
//! deliberately absent and keep their literal matches.
//!
//! `KeyGate` is a coarser presentation-only bucket than the keyboard
//! policy's `KeyPolicyGate`: every transport action collapses to one
//! `Playback` variant here, while `KeyPolicyGate` splits that bucket into
//! `Playback`/`PlaybackUngated`/`IdleFeedLink` per-key eligibility
//! conditions (task 5.1). `KeyGate` is used only to select which declared
//! actions render in the help/settings-Keys Playback group, never for
//! routing eligibility — that stays owned by the policy split.

use super::chord::Chord;

/// One presentation-only gate bucket used to group declared actions for
/// help/settings-Keys rendering; coarser than `key_policy.rs`'s
/// `KeyPolicyGate`, which owns actual routing eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyGate {
    NoBlockingOverlay,
    NoBlockingOverlayAndHelpClosed,
    PanelFocusQueue,
    PanelFocusLibraryBoth,
    QueueColumnWidth,
    ClearQueuePrompt,
    SessionsSidebarOpen,
    Playback,
}

/// The settings UI's section vocabulary plus `Global`, in `SETTING_SECTIONS`
/// order with `Global` appended (design D4). The settings main list's
/// navigation-only `Keys` entry is not a config section and has no variant
/// here. Config section names match the vocabulary case-insensitively
/// (design D3's file shape spells them lowercase); the canonical spelling is
/// `name()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeySection {
    Services,
    Playback,
    Display,
    Session,
    Library,
    Queue,
    Mpv,
    Feeds,
    Actions,
    Global,
}

pub const KEY_SECTIONS: &[KeySection] = &[
    KeySection::Services,
    KeySection::Playback,
    KeySection::Display,
    KeySection::Session,
    KeySection::Library,
    KeySection::Queue,
    KeySection::Mpv,
    KeySection::Feeds,
    KeySection::Actions,
    KeySection::Global,
];

impl KeySection {
    pub fn name(self) -> &'static str {
        match self {
            Self::Services => "Services",
            Self::Playback => "Playback",
            Self::Display => "Display",
            Self::Session => "Session",
            Self::Library => "Library",
            Self::Queue => "Queue",
            Self::Mpv => "Mpv",
            Self::Feeds => "Feeds",
            Self::Actions => "Actions",
            Self::Global => "Global",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        KEY_SECTIONS
            .iter()
            .copied()
            .find(|s| s.name().eq_ignore_ascii_case(name))
    }
}

/// One declared configurable keyboard action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeybindAction {
    /// Stable identifier: the config key and the policy entry name it
    /// replaces (unique across the table).
    pub id: &'static str,
    /// The owning settings section; a config assignment elsewhere is
    /// rejected at load.
    pub section: KeySection,
    /// Declared default chord(s). More than one chord records an alias
    /// (`+`/`=`); a configured action collapses to its single chord.
    pub default_chords: &'static [&'static str],
    /// Routing-eligibility bucket shared with the keyboard policy's gates.
    pub gate: KeyGate,
    /// The `KEY_POLICY` entry this action's matching replaces. For the split
    /// transport set this records the single `playback` bucket entry the
    /// actions were split out of (task 5.1); each action's own id is its
    /// `KEY_POLICY` entry name now.
    pub policy: &'static str,
    /// Router-scope override allowed. Every declared action is rebindable:
    /// the registry is exactly the rebindable set.
    pub rebindable: bool,
    /// May be assigned in the prefix namespace. Actions whose default chord
    /// is `Esc` are not: Escape while armed always disarms, never dispatches.
    pub prefix_addressable: bool,
}

impl KeybindAction {
    /// The declared default chords, parsed. Declaration strings must parse;
    /// the unit tests pin this.
    pub fn parsed_default_chords(&self) -> Vec<Chord> {
        self.default_chords
            .iter()
            .map(|s| Chord::parse(s).expect("declared default chord must parse"))
            .collect()
    }
}

/// Chords no configurable binding may take. Extensible; held for a future
/// hard binding (`Ctrl+q` already triggers quit in the daemon-lost overlay).
pub const RESERVED_CHORDS: &[&str] = &["Ctrl+q"];

/// The declared action table: 18 router-owned global chords plus the 12
/// split transport actions, sectioned per design D4.
pub const KEYBIND_ACTIONS: &[KeybindAction] = &[
    // ── Global (chrome not tied to one settings domain) ─────────────────
    KeybindAction {
        id: "help_open",
        section: KeySection::Global,
        default_chords: &["F1"],
        gate: KeyGate::NoBlockingOverlayAndHelpClosed,
        policy: "help_open",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "settings_open",
        section: KeySection::Global,
        default_chords: &["F2"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "settings_open",
        rebindable: true,
        prefix_addressable: true,
    },
    // ── Session ─────────────────────────────────────────────────────────
    KeybindAction {
        id: "quit",
        section: KeySection::Session,
        default_chords: &["q"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "quit",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "sessions_open",
        section: KeySection::Session,
        default_chords: &["F3"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "sessions_open",
        rebindable: true,
        prefix_addressable: true,
    },
    // ── Library ─────────────────────────────────────────────────────────
    KeybindAction {
        id: "search_open",
        section: KeySection::Library,
        default_chords: &["Ctrl+/", "Ctrl+_"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "search_open",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "next_library_tab",
        section: KeySection::Library,
        default_chords: &["Tab"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "next_library_tab",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "previous_library_tab",
        section: KeySection::Library,
        default_chords: &["BackTab"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "previous_library_tab",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "library_tab_jump",
        section: KeySection::Library,
        // Positional: the digit pressed resolves the tab (kept per design D2).
        default_chords: &["1", "2", "3", "4", "5", "6", "7", "8", "9"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "library_tab_jump",
        rebindable: true,
        // The payload is the pressed digit, which an action-level prefix
        // mapping cannot carry: a prefix assignment would pin the tab to the
        // configured router chord, not the digit typed. Not
        // prefix-addressable (rejected at load).
        prefix_addressable: false,
    },
    KeybindAction {
        id: "f5_refresh",
        section: KeySection::Library,
        default_chords: &["F5"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "f5_refresh",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "alt_next_library_tab",
        section: KeySection::Library,
        default_chords: &["Alt+Down"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "alt_next_library_tab",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "alt_previous_library_tab",
        section: KeySection::Library,
        default_chords: &["Alt+Up"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "alt_previous_library_tab",
        rebindable: true,
        prefix_addressable: true,
    },
    // ── Queue ───────────────────────────────────────────────────────────
    KeybindAction {
        id: "playlists_open",
        section: KeySection::Queue,
        default_chords: &["F4"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "playlists_open",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "clear_queue_prompt_c",
        section: KeySection::Queue,
        default_chords: &["c"],
        gate: KeyGate::ClearQueuePrompt,
        policy: "clear_queue_prompt_c",
        rebindable: true,
        prefix_addressable: true,
    },
    // ── Playback ────────────────────────────────────────────────────────
    KeybindAction {
        id: "visualizer",
        section: KeySection::Playback,
        default_chords: &["v"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "visualizer",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "toggle_play_pause",
        section: KeySection::Playback,
        default_chords: &["Space"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "stop",
        section: KeySection::Playback,
        default_chords: &["Esc"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        // Escape while armed must disarm, never dispatch (spec: prefix
        // namespace capture), so `stop` cannot take a prefix assignment.
        prefix_addressable: false,
    },
    KeybindAction {
        id: "seek_back",
        section: KeySection::Playback,
        default_chords: &["<"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "seek_forward",
        section: KeySection::Playback,
        default_chords: &[">"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "next_track",
        section: KeySection::Playback,
        // Crossterm-case treatment (U3 lesson, as `Ctrl+l`): Shift+n is
        // delivered as `Char('N')` + SHIFT, so the declared chord must carry
        // SHIFT to be seen by routing at all.
        default_chords: &["Shift+N"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "previous_track",
        section: KeySection::Playback,
        // Crossterm-case treatment: Shift+p arrives as Char('P') + SHIFT.
        default_chords: &["Shift+P"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "volume_down",
        section: KeySection::Playback,
        default_chords: &["-"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "volume_up",
        section: KeySection::Playback,
        // Alias: both chords fire today; a configured chord replaces both.
        default_chords: &["+", "="],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "toggle_mute",
        section: KeySection::Playback,
        default_chords: &["m"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "toggle_mute_or_cycle_audio",
        section: KeySection::Playback,
        default_chords: &["a"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "cycle_subtitle",
        section: KeySection::Playback,
        default_chords: &["z"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "open_idle_feed_link",
        section: KeySection::Playback,
        default_chords: &["o"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        prefix_addressable: true,
    },
    // ── Display ─────────────────────────────────────────────────────────
    KeybindAction {
        id: "panel_mode_cycle_x",
        section: KeySection::Display,
        default_chords: &["x"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "panel_mode_cycle_x",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "panel_right",
        section: KeySection::Display,
        default_chords: &["Ctrl+Right"],
        gate: KeyGate::PanelFocusQueue,
        policy: "panel_right",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "panel_left",
        section: KeySection::Display,
        default_chords: &["Ctrl+Left"],
        gate: KeyGate::PanelFocusLibraryBoth,
        policy: "panel_left",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "ctrl_l_force_clear",
        section: KeySection::Display,
        // Lowercase: crossterm delivers Ctrl+L as `Char('l')` + CONTROL, and
        // the chord grammar maps a single letter verbatim.
        default_chords: &["Ctrl+l"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "ctrl_l_force_clear",
        rebindable: true,
        prefix_addressable: true,
    },
];

/// Look up a declared action by its stable id.
pub fn action_by_id(id: &str) -> Option<&'static KeybindAction> {
    KEYBIND_ACTIONS.iter().find(|action| action.id == id)
}
