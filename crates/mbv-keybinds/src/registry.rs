//! Declared keybind action registry — the single source of truth for every
//! configurable keyboard action.
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
//! conditions. `KeyGate` is used only to select which declared
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
    /// Routing only ever opens while the resolving Client is the GUI
    /// Client (add-tauri-gui-client row 6.4): a GUI-only action's default
    /// chord is canvas-scoped, so the TUI-facing help and routing treat the
    /// action as theirs by section alone but its chord never resolves in a
    /// TUI Client.
    GuiClient,
}

/// The settings UI's section vocabulary plus `Global`, in `SETTING_SECTIONS`
/// order with `Global` appended. The settings main list's navigation-only
/// `Keys` entry is not a config section and has no variant here. Config
/// section names match the vocabulary case-insensitively; the canonical
/// spelling is `name()`.
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
    #[must_use]
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

    #[must_use]
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
    /// actions were split out of; each action's own id is its
    /// `KEY_POLICY` entry name now.
    pub policy: &'static str,
    /// Router-scope override allowed. Every declared action is rebindable:
    /// the registry is exactly the rebindable set.
    pub rebindable: bool,
    /// May be assigned in the prefix namespace. `stop` is not: an `Esc` stop
    /// chord and the armed disarm chord would collide, because Escape while
    /// armed always disarms, never dispatches.
    pub prefix_addressable: bool,
}

impl KeybindAction {
    /// The declared default chords, parsed. Declaration strings must parse;
    /// the unit tests pin this.
    ///
    /// # Panics
    ///
    /// Panics if a `default_chords` declaration fails to parse, i.e. if the
    /// "declared default chord must parse" invariant is violated.
    #[must_use]
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

/// The declared action table: the router-owned global actions plus the 12
/// split transport actions, grouped by section.
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
    KeybindAction {
        id: "twitch_url_open",
        section: KeySection::Global,
        // Crossterm-case treatment (as `next_track`): Shift+t arrives as
        // Char('T') + SHIFT, so the declared chord carries SHIFT.
        default_chords: &["Shift+T"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "twitch_url_open",
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
        // Positional: the digit pressed resolves the tab.
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
    // The canvas queue's row move (add-tauri-gui-client row 6.5, the
    // "a row is moved by key" spec): `Alt+J`/`Alt+K` move the selected row
    // one place, the same Owner slot move a row drag sends. GUI-only by
    // gate — the TUI moves a row with the Queue surface's
    // `Shift+Up`/`Shift+Down`, and its defaults stay unchanged — while
    // staying in the rebindable registry so a user can assign their own
    // chord for the GUI Client.
    KeybindAction {
        id: "queue_move_down",
        section: KeySection::Queue,
        default_chords: &["Alt+j"],
        gate: KeyGate::GuiClient,
        policy: "queue_move_down",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "queue_move_up",
        section: KeySection::Queue,
        default_chords: &["Alt+k"],
        gate: KeyGate::GuiClient,
        policy: "queue_move_up",
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
        id: "hide_visual_slot",
        section: KeySection::Playback,
        default_chords: &["h"],
        gate: KeyGate::NoBlockingOverlay,
        policy: "hide_visual_slot",
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
        default_chords: &["Shift+S"],
        gate: KeyGate::Playback,
        policy: "playback",
        rebindable: true,
        // An `Esc` stop chord and the armed disarm chord would collide: Escape
        // while armed must disarm, never dispatch (spec: prefix namespace
        // capture), so `stop` cannot take a prefix assignment.
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
    // The canvas transport's deeper steps (add-tauri-gui-client row 6.4,
    // the "back 10 s, forward 30 s" spec): the GUI Client's `seekBack`/
    // `seekForward` regions press the same commands their chords carry.
    // GUI-only by gate — the TUI keeps `<`/`>` at ±5 s and its defaults
    // unchanged — while staying in the rebindable registry so a user can
    // assign a chord of their own for the GUI Client.
    KeybindAction {
        id: "seek_back_10",
        section: KeySection::Playback,
        default_chords: &["["],
        gate: KeyGate::GuiClient,
        policy: "seek_back_10",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "seek_forward_30",
        section: KeySection::Playback,
        default_chords: &["]"],
        gate: KeyGate::GuiClient,
        policy: "seek_forward_30",
        rebindable: true,
        prefix_addressable: true,
    },
    // The track chips' popover openers (add-tauri-gui-client row 6.4, the
    // "every action is reachable from the keyboard" spec): the GUI
    // Client's audio and subtitle chips press the same popover toggle
    // their chords dispatch. GUI-only by gate — the TUI paints no track
    // chip — while staying in the rebindable registry so a user can
    // assign a chord of their own for the GUI Client.
    KeybindAction {
        id: "open_audio_popover",
        section: KeySection::Playback,
        default_chords: &["a"],
        gate: KeyGate::GuiClient,
        policy: "open_audio_popover",
        rebindable: true,
        prefix_addressable: true,
    },
    KeybindAction {
        id: "open_subtitle_popover",
        section: KeySection::Playback,
        default_chords: &["s"],
        gate: KeyGate::GuiClient,
        policy: "open_subtitle_popover",
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
        id: "cycle_audio",
        section: KeySection::Playback,
        default_chords: &["#"],
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
#[must_use]
pub fn action_by_id(id: &str) -> Option<&'static KeybindAction> {
    KEYBIND_ACTIONS.iter().find(|action| action.id == id)
}
