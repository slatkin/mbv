//! The router snapshot's plain-data types (ADR 0023): the playback, overlay,
//! and Client facts the policy's gates read, split out of `key_policy.rs`
//! along the type seam (the policy itself stays beside its ordered table).

use crate::app::state::client_capabilities::ClientCapabilities;
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
    /// A Client surface of its own fills the frame and holds the keyboard over
    /// the panels (`add-gui-settings-screen` design D2): the window Client's
    /// Settings screen, whose queue keys stay behind it while the global and
    /// transport actions still fire. It is non-blocking, like `NonBlocking`,
    /// and the queue-scoped gates read it directly. The TUI Client never
    /// produces it — its Settings sidebar keeps the `NonBlocking` situation.
    FullScreenSurface,
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

    /// Whether a Client surface of its own fills the frame and holds the
    /// keyboard over the panels.
    pub(in crate::app) fn full_screen_surface(self) -> bool {
        matches!(self, Self::FullScreenSurface)
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
///
/// The fields are the policy's inputs only. Prefix mode is not one of them:
/// the armed bit belongs to the Client that owns it, and the router reads it
/// through the entry it is given ([`super::RouterNow`], design D6 of
/// `decouple-client-core`), so a resolver takes the facts and a neutral access
/// rather than a copy of a Client's field.
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
    /// The capabilities of the resolving Client (design D6a of
    /// `decouple-client-core`): the transport chips' key context
    /// (add-tauri-gui-client row 6.2) reads
    /// `ClientCapability::TransportChipKeys`, so the canvas's `z` (collapse),
    /// `q` (queue fold), and `c` (cycle subtitle) chords route in a Client
    /// that paints the chips and never in the TUI Client — a configured TUI
    /// binding on those chords does not follow into the GUI, and the TUI's
    /// default bindings stay unchanged.
    pub client_capabilities: ClientCapabilities,
}

/// What the router needs to know about the input it is resolving, beyond the
/// policy's [`RouterSnapshot`] facts: the neutral state that lives outside the
/// snapshot and that the router reads through a method instead of a field
/// (design D6 of `decouple-client-core`).
///
/// Two implementors carry the fact in production, so neither Client's routing
/// site reads the other's state: the core `App` (its own `prefix_armed`
/// field) and the GUI Client's own core (the same method, one accessor down),
/// and both Clients resolve through one router entry
/// ([`super::resolve_router_outcome`]). The TUI Client's shell forwards it as
/// well, because arming and disarming also hold `TuiRealm` focus off.
pub trait RouterNow {
    /// Whether prefix mode is armed: while true, the next chord resolves
    /// against the prefix namespace only; arming is suppressed while a text
    /// entry owns focus or a blocking overlay is open (the `prefix_arm`
    /// layer's gate).
    fn prefix_armed(&self) -> bool;
}
