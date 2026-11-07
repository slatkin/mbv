//! Playback request type. Split from `msg.rs` (task 8.3) to keep the central
//! `Msg` file below the 800-line cap.

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
}

/// Which track chip a popover belongs to (add-tauri-gui-client row 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackChipKind {
    Audio,
    Subtitles,
}
