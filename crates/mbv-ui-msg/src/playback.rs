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
    ToggleMute,
    VolumeDelta(i64),
    CycleAudio,
    CycleSubtitle,
    /// Open the track chip's popover (add-tauri-gui-client row 6.2): the
    /// audio or subtitle chip's choice list over the local player's tracks.
    /// The GUI Client's chips emit it; the TUI Client paints no track chip,
    /// so its dispatch arm is the documented absence.
    OpenTrackPopover(TrackChipKind),
}

/// Which track chip a popover belongs to (add-tauri-gui-client row 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackChipKind {
    Audio,
    Subtitles,
}
