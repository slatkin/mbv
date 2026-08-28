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
    SeekRelative(i64),
    SeekTo(u16),
    ToggleMute,
    VolumeDelta(i64),
    CycleAudio,
    CycleSubtitle,
    ToggleVisualizer,
}
