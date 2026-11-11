//! The router snapshot's plain-data types (ADR 0023): the playback, overlay,
//! and Client facts the policy's gates read, split out of `key_policy.rs`
//! along the type seam (the policy itself stays beside its ordered table).

use mbv_ui_model::settings::{PanelFocus, PanelMode};

/// Which remote-control target currently owns transport shortcuts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RemoteControlTarget {
    #[default]
    None,
    DirectRemote,
}

/// Playback facts read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RouterPlaybackState {
    pub player_active: bool,
    pub remote_target: RemoteControlTarget,
    pub queue_only_idle: bool,
    pub idle_feed_link_available: bool,
}

/// The overlay-focus situation read by the central keyboard policy.
///
/// The policy only ever distinguishes the situations below. `blocking`
/// implies `holds_focus` (the blocking set is a subset of the focus-holding
/// set) and the context menu is itself a blocking overlay, so the facts are
/// not independent and are carried as one ordered enum instead of three
/// settable bools. `help`/`sessions_sidebar`/`text_entry_focused` stay
/// independent and live on `RouterOverlayState`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverlayFocus {
    /// No overlay holds `TuiRealm` focus; the panels own the keyboard.
    #[default]
    Free,
    /// A non-blocking overlay (Help, a sidebar) holds focus.
    NonBlocking,
    /// A blocking overlay other than the context menu holds focus.
    Blocking,
    /// The context menu is open (it is a blocking overlay the policy names
    /// explicitly, so the `ClearQueuePrompt` gate can read it directly).
    ContextMenu,
    /// The GUI Settings screen is open (`add-gui-settings-screen` design
    /// D2): a non-blocking overlay of the GUI Client's own that holds the
    /// keyboard over the queue and the Library panel, so the queue-scoped
    /// gates read it directly while the global and transport actions fire.
    /// The TUI Client never produces it — its Settings sidebar keeps the
    /// `NonBlocking` situation.
    SettingsScreen,
}

impl OverlayFocus {
    /// Whether a blocking overlay is mounted.
    pub(in crate::app) fn blocking(self) -> bool {
        matches!(self, Self::Blocking | Self::ContextMenu)
    }

    /// Whether any overlay/sidebar/modal holds `TuiRealm` focus.
    pub(in crate::app) fn holds_focus(self) -> bool {
        !matches!(self, Self::Free)
    }

    /// Whether the context menu is open.
    pub(in crate::app) fn context_menu(self) -> bool {
        matches!(self, Self::ContextMenu)
    }

    /// Whether the GUI Settings screen is open.
    pub(in crate::app) fn settings_screen(self) -> bool {
        matches!(self, Self::SettingsScreen)
    }
}

/// Overlay facts read by the central keyboard policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RouterOverlayState {
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
pub struct RouterSnapshot {
    pub playback: RouterPlaybackState,
    pub overlays: RouterOverlayState,
    pub panel_mode: PanelMode,
    pub panel_focus: PanelFocus,
    /// Whether the focused list holds an active Visual selection. Its row-local
    /// Space (`media-list-multi-select` spec) takes Space ahead of the global
    /// play/pause toggle.
    pub visual_selection_active: bool,
    /// Whether prefix mode is armed: the mirror of the App-owned bit. While
    /// true, the
    /// next chord resolves against the prefix namespace only; arming is
    /// suppressed while a text entry owns focus or a blocking overlay is
    /// open (the `prefix_arm` layer's gate).
    pub prefix_armed: bool,
    /// Whether the resolving Client is the GUI Client (add-tauri-gui-client
    /// row 6.2): the GUI-only key context's gate reads it, so the canvas's
    /// `z` (collapse), `q` (queue fold), and `c` (cycle subtitle) chords
    /// route in the GUI Client and never in the TUI Client — a configured
    /// TUI binding on those chords does not follow into the GUI, and the
    /// TUI's default bindings stay unchanged.
    pub gui_client: bool,
}
