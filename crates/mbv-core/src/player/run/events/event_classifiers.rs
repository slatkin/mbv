use super::super::{EndFileReason, QueueItem};
use libmpv2::mpv_end_file_reason;

pub(in crate::player) fn is_clocked_audio_error(
    error: &libmpv2::Error,
    audio_device_configured: bool,
) -> bool {
    audio_device_configured
        && matches!(
            error,
            libmpv2::Error::Raw(code) if *code == libmpv2::mpv_error::AoInitFailed
        )
}

/// Whether an `EndFile` is debris from a superseded `JumpTo`.
///
/// Pressing Enter on two queue rows in quick succession sends two
/// `PlayerCommand::JumpTo`s; both are drained before any mpv event, so
/// `forced_slot_id` holds the *second* target. mpv, meanwhile, may have
/// briefly started the first target before the second `playlist-pos` write
/// landed, and emits a `Stop` `EndFile` for it. The real target's own
/// `EndFile` already consumed `forced_slot_id` and advanced the queue, so
/// this stray one has no forced marker. Falling through to the
/// `current_idx + 1` advance would step the active index one past the row
/// the user actually selected, desyncing the UI from what mpv is playing.
/// mpv's upcoming `start-file` / `playlist-pos` change is authoritative
/// instead.
pub(in crate::player) fn is_superseded_jump_end_file(
    reason: EndFileReason,
    has_forced_slot: bool,
    track_finished: bool,
) -> bool {
    !has_forced_slot
        && !track_finished
        && (reason == mpv_end_file_reason::Stop || reason == mpv_end_file_reason::Redirect)
}

pub(in crate::player) fn provider_lifecycle_close_pos(
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
