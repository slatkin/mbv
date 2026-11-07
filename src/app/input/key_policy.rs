//! Live keyboard policy for the central router (ADR 0023).
//!
//! The policy is an ordered, pure function over a normalized chord and a
//! plain-data snapshot. It deliberately does not read `TuiRealm` attributes:
//! precedence belongs to the router, not to distributed component mirrors.

use super::resolver::KeyChord;
use crate::app::dispatch::action::{IdleFeedLinkContext, idle_feed_command_for_key};
use mbv_keybinds::{Keybinds, action_by_id};
use mbv_ui_model::settings::{PanelFocus, PanelMode};
use mbv_ui_msg::{Key, KeyModifiers};

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
/// The policy only ever distinguishes four reachable situations. `blocking`
/// implies `holds_focus` (the blocking set is a subset of the focus-holding
/// set) and the context menu is itself a blocking overlay, so the three facts
/// are not independent and are carried as one ordered enum instead of three
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

/// One ordered layer of the keyboard policy.
#[derive(Debug, Clone)]
pub(in crate::app) struct KeyPolicyEntry {
    /// Human-readable row label for the policy table; only the tests below
    /// read it, so silence dead-code just where the tests are compiled out.
    pub name: &'static str,
    /// Whether the central router (`UiRoot`) owns the binding.
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
    /// The Sessions sidebar's fixed queue-transfer keys `h`/`t` (design
    /// D6): a layer that keeps the leaf's request while the sidebar is
    /// mounted, so the global `hide_visual_slot` binding cannot swallow
    /// `h` before the component sees it. Not a registry action; matches
    /// unmodified letters only, and maps to no command (the router folds a
    /// matched commandless layer to `FallThrough`).
    SessionsSidebarTransfer,
    PlaylistsOpen,
    SearchOpen,
    /// The Twitch URL prompt opener (twitch-url-playback D7): a
    /// shell-owned modal mount, dispatched as a `ShellCommand` like Help.
    TwitchUrlOpen,
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
    CycleAudio,
    CycleSubtitle,
    OpenIdleFeedLink,
    CtrlL,
    F5,
    /// The prefix-arming layer's identity: matches the
    /// configured prefix chord, not a literal — the chord is resolved
    /// against `&Keybinds` in `entry_matches` because the layer is not a
    /// registry action. Always blocking: arming swallows.
    PrefixArm,
    /// The GUI-only key context's collapse toggle (add-tauri-gui-client row
    /// 6.2, design D4a's ruled collision): the canvas's `z`, which the TUI
    /// binds to `cycle_subtitle`. Not a registry action — the chord is
    /// canvas-fixed and the TUI never routes the layer, so the registry's
    /// configurable vocabulary stays TUI-facing.
    GuiCollapsePanel,
    /// The GUI-only key context's queue-fold toggle: the canvas's `q`, which
    /// the TUI binds to `quit`.
    GuiFoldQueue,
    /// The GUI-only key context's cycle-subtitle chord: the canvas's `c`,
    /// which the TUI binds to `clear_queue_prompt_c`.
    GuiCycleSubtitle,
    /// The GUI-only key context's queue-header Length/Ends-at switch.
    GuiRightColumnToggle,
    /// The GUI-only key context's queue-filter opener: the canvas's `/`,
    /// which the TUI binds to nothing (the library's inline search is a
    /// leaf-local `/`).
    GuiQueueFilterOpen,
    /// The GUI-only key context's playback-target chooser (add-tauri-gui-
    /// client row 6.4): `r` opens and closes the canvas's chooser, the GUI
    /// counterpart of the TUI's F3 Sessions sidebar. No TUI default binds
    /// `r`, so the chord collides with nothing there.
    GuiOpenTargetChooser,
    /// The canvas queue's row-move actions (add-tauri-gui-client row 6.5,
    /// the "a row is moved by key" spec): declared actions — rebindable —
    /// whose `GuiClient` gate keeps even a rebound chord out of the TUI,
    /// which moves a row with the Queue surface's `Shift+Up`/`Shift+Down`.
    QueueMoveDown,
    QueueMoveUp,
    /// The canvas transport's seek-back-10 action (add-tauri-gui-client
    /// row 6.4, the "back 10 s" spec): a registry action — rebindable —
    /// whose `GuiClient` gate keeps its chords out of the TUI.
    SeekBack10,
    /// The canvas transport's seek-forward-30 action (the "forward 30 s"
    /// spec), gated as `SeekBack10` is.
    SeekForward30,
    /// The audio chip's popover action (add-tauri-gui-client row 6.4, the
    /// "every action is reachable from the keyboard" spec): a registry
    /// action — rebindable — whose `GuiClient` gate keeps its chords out
    /// of the TUI, which paints no track chip.
    OpenAudioPopover,
    /// The subtitle chip's popover action, gated as `OpenAudioPopover` is.
    OpenSubtitlePopover,
}

impl KeyPolicyBinding {
    /// Literal chord match for the policy entries that declare no registry
    /// action (`sessions_sidebar_escape`, `queue_column_width`, and the
    /// blocking `alt_swallow`). Every declared action — the split transport
    /// set included — instead matches its configured chords from `&Keybinds`
    /// in `resolve_policy`; these keep their literals,
    /// including the swallow guard that blocks rather than commands.
    fn matches(self, chord: KeyChord) -> bool {
        match self {
            Self::SessionsDismiss => chord.code == Key::Esc,
            Self::SessionsSidebarTransfer => {
                chord.mods.is_empty() && matches!(chord.code, Key::Char('h' | 't'))
            }
            Self::AltSwallow => chord.mods.contains(KeyModifiers::ALT),
            Self::QueueColumnWidth => {
                matches!(chord.code, Key::Left | Key::Right) && chord.mods == KeyModifiers::SHIFT
            }
            // The GUI-only key context's canvas chords: unmodified single
            // keys, so a modified chord (Ctrl+Z's undo, a text entry's
            // Shift+letter) keeps its own meaning.
            Self::GuiCollapsePanel => chord.mods.is_empty() && chord.code == Key::Char('z'),
            Self::GuiFoldQueue => chord.mods.is_empty() && chord.code == Key::Char('q'),
            Self::GuiCycleSubtitle => chord.mods.is_empty() && chord.code == Key::Char('c'),
            Self::GuiRightColumnToggle => chord.mods.is_empty() && chord.code == Key::Char('t'),
            Self::GuiQueueFilterOpen => chord.mods.is_empty() && chord.code == Key::Char('/'),
            Self::GuiOpenTargetChooser => chord.mods.is_empty() && chord.code == Key::Char('r'),
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
    /// bucket split.
    Playback,
    /// Transport keys with no session/activity condition (`z`, `m`, `-`,
    /// `+`/`=` today). Modifier exclusions are structural: exact-chord
    /// matching makes Ctrl+chords inert.
    PlaybackUngated,
    /// The idle-feed link shortcut (`o`): its own availability condition,
    /// owned by `idle_feed_command_for_key`.
    IdleFeedLink,
    /// Prefix arming: the prefix chord must not arm
    /// while a text-entry surface owns focus or a blocking overlay is open;
    /// in those states it routes as it does without the change.
    PrefixArming,
    /// The GUI-only key context (add-tauri-gui-client row 6.2): the canvas's
    /// chords route only in the GUI Client, and only with no blocking
    /// overlay open and no text entry focused, so the open row menu or the
    /// filter's text entry keeps the chords it owns.
    GuiClient,
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
            Self::GuiClient => {
                snapshot.gui_client
                    && !snapshot.overlays.focus.blocking()
                    && !snapshot.overlays.text_entry_focused
            }
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
            || snapshot.playback.remote_target != RemoteControlTarget::None)
}

fn idle_feed_link_allowed(chord: KeyChord, snapshot: &RouterSnapshot) -> bool {
    !snapshot.overlays.focus.blocking()
        && !snapshot.overlays.text_entry_focused
        && idle_feed_command_for_key(
            chord,
            &IdleFeedLinkContext {
                player_active: snapshot.playback.player_active,
                playback_panel_present_idle: snapshot.playback.queue_only_idle,
                link_available: snapshot.playback.idle_feed_link_available,
            },
        )
        .is_some()
}

/// The ordered keyboard policy. Entries are first-match-wins.
pub(in crate::app) const KEY_POLICY: &[KeyPolicyEntry] = &[
    // Prefix arming: the top layer. Its chord is
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
        name: "twitch_url_open",
        global: true,
        binding: KeyPolicyBinding::TwitchUrlOpen,
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
    // The GUI-only key context (add-tauri-gui-client row 6.2, design D4a's
    // ruled collision): the canvas's chords bind here, ahead of the TUI
    // actions whose defaults they collide with (`quit` on `q`,
    // `cycle_subtitle` on `z`, `clear_queue_prompt_c` on `c`), so a TUI
    // binding on those chords does not follow into the GUI. The TUI Client's
    // snapshot never opens the gate, so the layers are inert there and the
    // TUI's default bindings stay unchanged. Not registry actions: the
    // chords are canvas-fixed and the TUI-facing registry would otherwise
    // list chords the TUI never routes.
    KeyPolicyEntry {
        name: "gui_collapse_panel",
        global: true,
        binding: KeyPolicyBinding::GuiCollapsePanel,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "gui_fold_queue",
        global: true,
        binding: KeyPolicyBinding::GuiFoldQueue,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "gui_cycle_subtitle",
        global: true,
        binding: KeyPolicyBinding::GuiCycleSubtitle,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "gui_right_column_toggle",
        global: true,
        binding: KeyPolicyBinding::GuiRightColumnToggle,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "gui_queue_filter_open",
        global: true,
        binding: KeyPolicyBinding::GuiQueueFilterOpen,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "gui_open_target_chooser",
        global: true,
        binding: KeyPolicyBinding::GuiOpenTargetChooser,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    // The canvas queue's row moves (add-tauri-gui-client row 6.5): declared
    // actions like the deep seeks below — a user may rebind them — whose
    // `GuiClient` gate keeps even a rebound chord out of the TUI.
    KeyPolicyEntry {
        name: "queue_move_down",
        global: true,
        binding: KeyPolicyBinding::QueueMoveDown,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "queue_move_up",
        global: true,
        binding: KeyPolicyBinding::QueueMoveUp,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    // The canvas transport's registry actions (add-tauri-gui-client row
    // 6.4): unlike the canvas-fixed literals above, these are declared
    // actions — a user may rebind them — so `entry_matches` resolves their
    // chords from `&Keybinds`. The `GuiClient` gate keeps even a rebound
    // chord out of the TUI, where `<`/`>` at ±5 s stay the only seeks.
    KeyPolicyEntry {
        name: "seek_back_10",
        global: true,
        binding: KeyPolicyBinding::SeekBack10,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "seek_forward_30",
        global: true,
        binding: KeyPolicyBinding::SeekForward30,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    // The track chips' popover openers (the "every action is reachable
    // from the keyboard" spec): declared actions like the deep seeks
    // above — a user may rebind them — whose `GuiClient` gate keeps even
    // a rebound chord out of the TUI, which paints no track chip to open.
    KeyPolicyEntry {
        name: "open_audio_popover",
        global: true,
        binding: KeyPolicyBinding::OpenAudioPopover,
        gate: KeyPolicyGate::GuiClient,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "open_subtitle_popover",
        global: true,
        binding: KeyPolicyBinding::OpenSubtitlePopover,
        gate: KeyPolicyGate::GuiClient,
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
        // The Sessions sidebar's fixed transfer keys (design D6) must
        // outrank the global `hide_visual_slot` binding on `h`, so this
        // layer sits above it; first-match order decides. Like
        // `sessions_sidebar_escape`, it maps to no command: a match falls
        // through and the focused sidebar leaf keeps its request.
        name: "sessions_sidebar_transfer",
        global: false,
        binding: KeyPolicyBinding::SessionsSidebarTransfer,
        gate: KeyPolicyGate::SessionsSidebarOpen,
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
    // The split transport set: one entry per registry
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
        name: "cycle_audio",
        global: false,
        binding: KeyPolicyBinding::CycleAudio,
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
/// (falling back to its declared defaults when unconfigured);
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
    // The prefix-arming layer is not a registry action: it matches the
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

pub(in crate::app) use commands::command_for_policy;
mod commands;

// ---------------------------------------------------------------------------
// Mouse subscription pattern
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
