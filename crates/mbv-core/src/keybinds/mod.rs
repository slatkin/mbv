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

/// Keyboard modifiers of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyMods(u8);

impl KeyMods {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// The key half of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Backspace,
    Enter,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
}

/// One normalized keyboard chord: modifiers plus key. The textual grammar is
/// `"Ctrl+b"`, `"F8"`, `"Shift+Left"`, `"Space"`; modifier order is not
/// significant and the canonical rendering (Display) is Ctrl+Shift+Alt+key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: KeyMods,
    pub key: Key,
}

/// Why a chord string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChordParseError {
    Empty,
    EmptyToken,
    UnknownModifier(String),
    UnknownKey(String),
}

impl std::fmt::Display for ChordParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty chord"),
            Self::EmptyToken => write!(f, "empty chord token"),
            Self::UnknownModifier(name) => write!(f, "unknown modifier `{name}`"),
            Self::UnknownKey(name) => write!(f, "unknown key `{name}`"),
        }
    }
}

impl Chord {
    /// Parse a chord string: optional modifiers (`Ctrl`, `Shift`, `Alt`,
    /// order-insensitive, case-insensitive) plus one key (a single character,
    /// `F1`–`F12`, or a named key such as `Esc`, `Tab`, `BackTab`, `Enter`,
    /// `Space`, arrows, `Home`, `End`, `PageUp`, `PageDown`, `Delete`,
    /// `Insert`, `Backspace`). A lone `+` or `-` is a character chord.
    pub fn parse(s: &str) -> Result<Self, ChordParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ChordParseError::Empty);
        }
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Ok(Self {
                mods: KeyMods::NONE,
                key: Key::Char(c),
            });
        }
        let tokens: Vec<&str> = s.split('+').collect();
        let (key_token, modifier_tokens) = tokens.split_last().expect("non-empty split");
        let mut mods = KeyMods::NONE;
        for token in modifier_tokens {
            let token = token.trim();
            if token.is_empty() {
                return Err(ChordParseError::EmptyToken);
            }
            let modifier = match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyMods::CTRL,
                "shift" => KeyMods::SHIFT,
                "alt" => KeyMods::ALT,
                _ => {
                    return Err(ChordParseError::UnknownModifier(token.to_string()));
                }
            };
            mods = mods.union(modifier);
        }
        Ok(Self {
            mods,
            key: parse_key(key_token)?,
        })
    }
}

fn parse_key(token: &str) -> Result<Key, ChordParseError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(ChordParseError::EmptyToken);
    }
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Ok(Key::Char(c));
    }
    let lower = token.to_ascii_lowercase();
    let key = match lower.as_str() {
        "esc" | "escape" => Key::Esc,
        "tab" => Key::Tab,
        "backtab" => Key::BackTab,
        "enter" | "return" => Key::Enter,
        "space" => Key::Char(' '),
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "delete" | "del" => Key::Delete,
        "insert" => Key::Insert,
        "backspace" => Key::Backspace,
        f if f.len() > 1
            && f.starts_with('f')
            && f[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) =>
        {
            Key::F(f[1..].parse::<u8>().expect("validated above"))
        }
        _ => return Err(ChordParseError::UnknownKey(token.to_string())),
    };
    Ok(key)
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backspace => write!(f, "Backspace"),
            Self::Enter => write!(f, "Enter"),
            Self::Esc => write!(f, "Esc"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Up => write!(f, "Up"),
            Self::Down => write!(f, "Down"),
            Self::Home => write!(f, "Home"),
            Self::End => write!(f, "End"),
            Self::PageUp => write!(f, "PageUp"),
            Self::PageDown => write!(f, "PageDown"),
            Self::Tab => write!(f, "Tab"),
            Self::BackTab => write!(f, "BackTab"),
            Self::Delete => write!(f, "Delete"),
            Self::Insert => write!(f, "Insert"),
            Self::F(n) => write!(f, "F{n}"),
            Self::Char(' ') => write!(f, "Space"),
            Self::Char(c) => write!(f, "{c}"),
        }
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mods.contains(KeyMods::CTRL) {
            write!(f, "Ctrl+")?;
        }
        if self.mods.contains(KeyMods::SHIFT) {
            write!(f, "Shift+")?;
        }
        if self.mods.contains(KeyMods::ALT) {
            write!(f, "Alt+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// Per-section configured bindings: router-scope overrides and
/// prefix-namespace assignments, each mapping a declared action id to its
/// configured chord.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SectionBindings {
    pub router: Vec<(&'static str, Chord)>,
    pub prefix: Vec<(&'static str, Chord)>,
}

/// The compiled keybind configuration: the optional prefix chord plus
/// per-section router overrides and prefix-namespace assignments. Actions
/// without an override resolve to their declared defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keybinds {
    pub prefix: Option<Chord>,
    pub sections: Vec<(KeySection, SectionBindings)>,
}

impl Keybinds {
    /// The all-defaults configuration: no prefix, every action on its
    /// declared chords. Compiled from `KEYBIND_ACTIONS` through the
    /// resolution helpers below.
    pub fn defaults() -> Self {
        Self::default()
    }

    /// The configured router-scope chord for an action id, if any.
    pub fn router_override(&self, id: &str) -> Option<Chord> {
        self.sections
            .iter()
            .flat_map(|(_, bindings)| bindings.router.iter())
            .find(|(action_id, _)| *action_id == id)
            .map(|(_, chord)| *chord)
    }

    /// The configured prefix-namespace chord for an action id, if any.
    pub fn prefix_assignment(&self, id: &str) -> Option<Chord> {
        self.sections
            .iter()
            .flat_map(|(_, bindings)| bindings.prefix.iter())
            .find(|(action_id, _)| *action_id == id)
            .map(|(_, chord)| *chord)
    }

    /// The chords that fire an action under this configuration: its single
    /// configured chord, or all of its declared default chords.
    pub fn router_chords(&self, action: &KeybindAction) -> Vec<Chord> {
        match self.router_override(action.id) {
            Some(chord) => vec![chord],
            None => action.parsed_default_chords(),
        }
    }

    /// The first chord that fires an action under this configuration.
    pub fn router_chord(&self, action: &KeybindAction) -> Chord {
        self.router_chords(action)
            .into_iter()
            .next()
            .expect("every action resolves to at least one chord")
    }

    /// Every configured prefix-namespace assignment as (action, configured
    /// chord), for the armed-dispatch lookup (change
    /// `add-configurable-keybinds`, design D6). Ids are validated at load.
    pub fn prefix_assignments(&self) -> impl Iterator<Item = (&'static KeybindAction, Chord)> + '_ {
        self.sections
            .iter()
            .flat_map(|(_, bindings)| bindings.prefix.iter())
            .filter_map(|(id, chord)| Some((action_by_id(id)?, *chord)))
    }

    /// The number of actions whose router-scope binding deviates from the
    /// declared default: the settings main list's Keys-row summary count
    /// (design D7). Counted as distinct action ids — an action bound by
    /// more than one router entry still counts once — and a configured
    /// chord equal to a declared default is not a deviation.
    pub fn override_count(&self) -> usize {
        let mut overridden: Vec<&'static str> = self
            .sections
            .iter()
            .flat_map(|(_, bindings)| bindings.router.iter())
            .filter(|(id, chord)| {
                action_by_id(id)
                    .is_some_and(|action| !action.parsed_default_chords().contains(chord))
            })
            .map(|(id, _)| *id)
            .collect();
        overridden.sort_unstable();
        overridden.dedup();
        overridden.len()
    }
}

/// The `[keys]` configuration as read from the file, before compilation:
/// section-outer shape with the prefix chord and per-section router /
/// prefix-namespace chord assignments (design D3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawKeybinds {
    pub prefix: Option<String>,
    pub sections: Vec<(String, RawSection)>,
}

/// One section's entries as read from the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawSection {
    pub router: Vec<(String, String)>,
    pub prefix: Vec<(String, String)>,
}

/// A load-time rejection, naming the offending entry (both entries for the
/// collision classes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindsError {
    ReservedChord {
        chord: String,
        entry: String,
    },
    UnparseableChord {
        chord: String,
        entry: String,
        reason: String,
    },
    UnknownAction {
        action: String,
        section: String,
    },
    UnknownSection {
        section: String,
    },
    DuplicateSection {
        first: String,
        second: String,
    },
    SectionMismatch {
        action: String,
        section: String,
        declared: KeySection,
    },
    NotPrefixAddressable {
        action: String,
        section: String,
    },
    PrefixCollision {
        chord: String,
        entry: String,
    },
    RouterCollision {
        chord: String,
        first: String,
        second: String,
    },
    PrefixNamespaceCollision {
        chord: String,
        first: String,
        second: String,
    },
}

impl std::fmt::Display for KeybindsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReservedChord { chord, entry } => write!(
                f,
                "keys: chord `{chord}` in `{entry}` is reserved and cannot be configured"
            ),
            Self::UnparseableChord {
                chord,
                entry,
                reason,
            } => write!(f, "keys: `{entry}` has unparseable chord `{chord}`: {reason}"),
            Self::UnknownAction { action, section } => write!(
                f,
                "keys: unknown action `{action}` in section `keys.{section}`"
            ),
            Self::UnknownSection { section } => {
                write!(f, "keys: unknown section `keys.{section}`")
            }
            Self::DuplicateSection { first, second } => write!(
                f,
                "keys: sections `keys.{first}` and `keys.{second}` name the same section under different spellings"
            ),
            Self::SectionMismatch {
                action,
                section,
                declared,
            } => write!(
                f,
                "keys: action `{action}` is declared in section `{}` but configured under `keys.{section}`",
                declared.name()
            ),
            Self::NotPrefixAddressable { action, section } => write!(
                f,
                "keys: action `{action}` is not prefix-addressable (`keys.{section}.prefix.{action}`)"
            ),
            Self::PrefixCollision { chord, entry } => write!(
                f,
                "keys: prefix chord `{chord}` collides with binding `{entry}`"
            ),
            Self::RouterCollision {
                chord,
                first,
                second,
            } => write!(
                f,
                "keys: actions `{first}` and `{second}` share router-scope chord `{chord}`"
            ),
            Self::PrefixNamespaceCollision {
                chord,
                first,
                second,
            } => write!(
                f,
                "keys: actions `{first}` and `{second}` share prefix-namespace chord `{chord}`"
            ),
        }
    }
}

impl std::error::Error for KeybindsError {}

/// Compile a raw `[keys]` configuration into validated `Keybinds`, rejecting
/// every malformed or colliding entry at load (design D5).
pub fn load(raw: &RawKeybinds) -> Result<Keybinds, KeybindsError> {
    let reserved: Vec<Chord> = RESERVED_CHORDS
        .iter()
        .map(|s| Chord::parse(s).expect("reserved chord must parse"))
        .collect();

    let prefix = raw
        .prefix
        .as_ref()
        .map(|s| parse_configured(s, "keys.prefix", &reserved))
        .transpose()?;

    let mut sections = Vec::new();
    // `KeySection::from_name` matches case-insensitively, but the save loop
    // keys the compiled tables by lowercase section name — two raw spellings
    // of one section would silently collapse to the later spelling on save
    // while the reader took the first. Reject them at load.
    let mut seen_sections: Vec<&str> = Vec::new();
    let mut configured_router: Vec<(&'static KeybindAction, Chord)> = Vec::new();
    let mut configured_prefix_ns: Vec<(&'static KeybindAction, Chord)> = Vec::new();

    for (section_name, raw_section) in &raw.sections {
        if let Some(first) = seen_sections
            .iter()
            .find(|seen| seen.eq_ignore_ascii_case(section_name))
        {
            return Err(KeybindsError::DuplicateSection {
                first: (*first).to_string(),
                second: section_name.clone(),
            });
        }
        seen_sections.push(section_name);
        let section =
            KeySection::from_name(section_name).ok_or_else(|| KeybindsError::UnknownSection {
                section: section_name.clone(),
            })?;
        let mut bindings = SectionBindings::default();
        for (id, chord) in &raw_section.router {
            let action = lookup_in_section(id, section_name)?;
            let entry = format!("keys.{section_name}.{id}");
            let parsed = parse_configured(chord, &entry, &reserved)?;
            configured_router.push((action, parsed));
            bindings.router.push((action.id, parsed));
        }
        for (id, chord) in &raw_section.prefix {
            let action = lookup_in_section(id, section_name)?;
            if !action.prefix_addressable {
                return Err(KeybindsError::NotPrefixAddressable {
                    action: id.clone(),
                    section: section_name.clone(),
                });
            }
            let entry = format!("keys.{section_name}.prefix.{id}");
            let parsed = parse_configured(chord, &entry, &reserved)?;
            configured_prefix_ns.push((action, parsed));
            bindings.prefix.push((action.id, parsed));
        }
        sections.push((section, bindings));
    }

    let keybinds = Keybinds { prefix, sections };

    // The prefix chord must not collide with any other configured or declared
    // binding (router scope or prefix namespace).
    if let Some(prefix) = keybinds.prefix {
        for (action, chord) in &configured_router {
            if *chord == prefix {
                return Err(KeybindsError::PrefixCollision {
                    chord: prefix.to_string(),
                    entry: format!("keys.{}.{}", action.section.name(), action.id),
                });
            }
        }
        for (action, chord) in &configured_prefix_ns {
            if *chord == prefix {
                return Err(KeybindsError::PrefixCollision {
                    chord: prefix.to_string(),
                    entry: format!("keys.{}.prefix.{}", action.section.name(), action.id),
                });
            }
        }
        for action in KEYBIND_ACTIONS {
            if configured_router
                .iter()
                .any(|(configured, _)| configured.id == action.id)
            {
                continue;
            }
            if action.parsed_default_chords().contains(&prefix) {
                return Err(KeybindsError::PrefixCollision {
                    chord: prefix.to_string(),
                    entry: format!("keys.{}.{}", action.section.name(), action.id),
                });
            }
        }
    }

    // Router-scope: a configured chord must not equal any other action's
    // effective chord (configured or declared default). Pure default-vs-
    // default pairs are exempt — the declaration test pins that no two
    // actions share a default chord.
    for (action, chord) in &configured_router {
        for other in KEYBIND_ACTIONS {
            if other.id == action.id {
                continue;
            }
            let other_effective = match configured_router
                .iter()
                .find(|(configured, _)| configured.id == other.id)
            {
                Some((_, configured)) => vec![*configured],
                None => other.parsed_default_chords(),
            };
            if other_effective.contains(chord) {
                return Err(KeybindsError::RouterCollision {
                    chord: chord.to_string(),
                    first: action.id.to_string(),
                    second: other.id.to_string(),
                });
            }
        }
    }

    // Prefix namespace: two configured assignments must not share a chord.
    for (index, (first, chord)) in configured_prefix_ns.iter().enumerate() {
        for (second, other_chord) in configured_prefix_ns.iter().skip(index + 1) {
            if first.id != second.id && chord == other_chord {
                return Err(KeybindsError::PrefixNamespaceCollision {
                    chord: chord.to_string(),
                    first: first.id.to_string(),
                    second: second.id.to_string(),
                });
            }
        }
    }

    Ok(keybinds)
}

fn lookup_in_section(
    id: &str,
    section_name: &str,
) -> Result<&'static KeybindAction, KeybindsError> {
    let action = action_by_id(id).ok_or_else(|| KeybindsError::UnknownAction {
        action: id.to_string(),
        section: section_name.to_string(),
    })?;
    if !action.section.name().eq_ignore_ascii_case(section_name) {
        return Err(KeybindsError::SectionMismatch {
            action: id.to_string(),
            section: section_name.to_string(),
            declared: action.section,
        });
    }
    Ok(action)
}

fn parse_configured(chord: &str, entry: &str, reserved: &[Chord]) -> Result<Chord, KeybindsError> {
    let parsed = Chord::parse(chord).map_err(|reason| KeybindsError::UnparseableChord {
        chord: chord.to_string(),
        entry: entry.to_string(),
        reason: reason.to_string(),
    })?;
    if reserved.contains(&parsed) {
        return Err(KeybindsError::ReservedChord {
            chord: chord.to_string(),
            entry: entry.to_string(),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests;
