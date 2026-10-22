use super::super::{EndFileReason, JumpArm, MpvFailure, QueueItem};

pub(crate) fn is_clocked_audio_error(failure: &MpvFailure, audio_device_configured: bool) -> bool {
    audio_device_configured && matches!(failure, MpvFailure::AudioOutputInit)
}

/// Whether an `EndFile` is debris from a superseded `JumpTo`.
///
/// Pressing Enter on two queue rows in quick succession sends two
/// `PlayerCommand::JumpTo`s; both are drained before any mpv event, so the
/// pending transition marks the *second* target. mpv may briefly start the
/// first target before the second `playlist-pos` write lands, and emit a
/// `Stop` `EndFile` for it. The real target's own `EndFile` already consumed
/// the pending transition marker, so this stray one has no forced marker.
/// Falling through to the
/// `current_idx + 1` advance would step the active index one past the row
/// the user actually selected, desyncing the UI from what mpv is playing.
/// mpv's upcoming `start-file` / `playlist-pos` change is authoritative
/// instead.
pub(crate) fn is_superseded_jump_end_file(
    reason: EndFileReason,
    jump_arm: JumpArm,
    track_finished: bool,
) -> bool {
    let unsettled_marker = match jump_arm {
        // A pending arm still holding its transition marker IS the settle the
        // caller waits for; debris arrives after that marker was consumed by
        // the real target's end-file.
        JumpArm::Pending(jump) => jump.transition.is_none(),
        JumpArm::Idle | JumpArm::ResumeOnly { .. } => true,
    };
    unsettled_marker
        && !track_finished
        && matches!(reason, EndFileReason::Stop | EndFileReason::Redirect)
}

pub(crate) fn provider_lifecycle_close_pos(
    item: &QueueItem,
    natural_end: bool,
    runtime: i64,
    last_valid_pos: i64,
) -> i64 {
    if item.is_audiobookshelf() && natural_end {
        runtime.max(0)
    } else {
        last_valid_pos
    }
}
