//! Playback request type. Split from `msg.rs` (task 8.3) to keep the central
//! `Msg` file below the 800-line cap.

use mbv_queue::NamedQueueId;

/// Playback effect requests emitted by Interactive Components. The shell
/// resolves them through the existing `App::dispatch` path; the component
/// owns the cursor and the user intent.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackRequest {
    TogglePlayPause,
    Stop,
    Previous,
    Next,
    /// Seek to a resolved 0.0..=1.0 fraction of the runtime. `LibraryPlaybackPanel`
    /// resolves the click column against its own `seekbar_area` so no shell code
    /// reads painted seek-bar geometry (ADR 0022 Residual A).
    SeekTo(f64),
    /// Seek by a signed delta in seconds, exactly as the `seek_back` and
    /// `seek_forward` chords do (add-tauri-gui-client row 6.4): the canvas's
    /// back and forward controls emit the deltas their chords carry, so a
    /// button press and the chord dispatch the same command.
    SeekRelative(f64),
    ToggleMute,
    VolumeDelta(i64),
    CycleAudio,
    CycleSubtitle,
    /// Open the track chip's popover (add-tauri-gui-client row 6.2): the
    /// audio or subtitle chip's choice list over the local player's tracks.
    /// The GUI Client's chips emit it; the TUI Client paints no track chip,
    /// so its dispatch arm is the documented absence.
    OpenTrackPopover(TrackChipKind),
    /// Activate the open track popover's painted choice by its track id
    /// (add-tauri-gui-client rows 6.4 and 6.5): a popover row's press runs
    /// the same execution the popover's Enter applies, and it names the
    /// choice's mpv id rather than its list index — the list can change
    /// between the frame's paint and the press, and an index would then
    /// switch to the neighbour. The GUI Client's core resolves the painted
    /// row index to the id it painted; the TUI Client paints no track chip,
    /// so its dispatch arm is the documented absence.
    TrackPopoverSelect(i64),
    /// The canvas header's collapse control (add-tauri-gui-client row 6.4):
    /// the `z` toggle's pointer dispatch. The GUI Client's header emits it;
    /// the TUI's panel has no collapse control.
    TogglePanelCollapse,
    /// The canvas header's queue-fold control: the `q` toggle's pointer
    /// dispatch. GUI-painted only, as `TogglePanelCollapse` is.
    ToggleQueueFold,
    /// The player panel's right-click (gui-player-overlay-menu): opens the
    /// Client-core player menu at the pointer, whose single entry toggles
    /// the persisted auto-hide preference. GUI-painted only, as
    /// `TogglePanelCollapse` is. The anchor is the pointer's CSS pixels.
    OpenPlayerMenu {
        /// The right-click's window anchor, in CSS pixels.
        x: f32,
        y: f32,
    },
    /// The player menu's auto-hide entry press (gui-player-overlay-menu):
    /// the entry the window painted, run through the same execution the
    /// menu's Enter applies, and the menu closes with it. GUI-painted only.
    PlayerMenuSelect,
    /// The canvas queue header's filter control (row 6.5): the `/` chord's
    /// text-entry focus, pressed on the header control. GUI-painted only.
    ActivateQueueFilter,
    /// The canvas queue header's chooser control (design D8 of
    /// `gui-named-queues`): the `t` chord's open and close, pressed on the
    /// header control. GUI-painted only.
    QueueChooserToggle,
    /// The canvas queue header's new-queue control (design D8 of
    /// `gui-named-queues`): the `Ctrl+T` chord's request, pressed on the header
    /// control. GUI-painted only.
    NewQueue,
    /// The canvas queue header's close control (design D8 of `gui-named-queues`):
    /// the `Ctrl+W` chord's request, pressed on the header control.
    /// GUI-painted only.
    CloseQueue,
    /// One entry of the open Queue chooser (task 8.3 of `gui-named-queues`): the
    /// entry's painted identity — a local Named queue, or `None` for the
    /// connected peer Player owner — so the press shows what the window painted
    /// and a queue that closed in between is not replaced by its neighbour. It
    /// runs the same show the chooser's Enter applies: the entry is shown, and
    /// no playback starts, stops, or moves.
    QueueChooserEntry(Option<NamedQueueId>),
}

/// Which track chip a popover belongs to (add-tauri-gui-client row 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackChipKind {
    Audio,
    Subtitles,
}
