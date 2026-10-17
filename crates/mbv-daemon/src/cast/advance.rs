use mbv_cast::client::{CastIdleReason, CastPlaybackState};
use mbv_ctrl::CtrlEvent;

use super::super::DaemonLoop;
use super::PlaybackOutput;

/// How close to the duration the last known position must be for an `Error`
/// end to count as the item's natural end. Over two 7 s poll intervals: the position is
/// the last polled one, and a receiver can fail its stream tail (HLS 310)
/// instead of reporting `Finished`.
const NEAR_END_SECONDS: f32 = 15.0;

/// What the freshly stored status's end report asks the Owner to do (the Cast
/// end table of design D7).
enum EndReport {
    Step,
    ReportError,
}

/// Applies the Cast end table to the output's freshly stored status. Only an
/// idle report that still names the loaded content id and an end this output
/// has not handled yet reaches the table: the end-handled flag makes the
/// second poll of the same `Finished` — and the same `Error` — a no-op, so a
/// receiver that lingers on one end report advances exactly once. The step
/// itself is the shared advance (`output/advance`, emby-session-owner-output
/// design D6).
pub(super) fn apply_end_report(daemon: &mut DaemonLoop) {
    let report = {
        let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        let (Some(status), Some(loaded)) = (cast.last_status.as_ref(), cast.loaded.as_mut()) else {
            return;
        };
        if status.state != CastPlaybackState::Idle
            || status.playing_content_id.as_deref() != Some(loaded.content_id.as_str())
            || loaded.end_handled
        {
            return;
        }
        match status.idle_reason {
            Some(CastIdleReason::Finished) => {
                loaded.end_handled = true;
                Some(EndReport::Step)
            }
            Some(CastIdleReason::Error) => {
                loaded.end_handled = true;
                let near_end = status
                    .position_seconds
                    .zip(status.duration_seconds)
                    .is_some_and(|(position, duration)| duration - position <= NEAR_END_SECONDS);
                Some(if near_end {
                    EndReport::Step
                } else {
                    EndReport::ReportError
                })
            }
            // A Cancelled end presents no active playback and advances
            // nothing; an Interrupted end is the Owner's own replacing load.
            // Both are presentation-only.
            _ => None,
        }
    };
    match report {
        Some(EndReport::Step) => {
            // The ended item's session reports its stop before the step's
            // load can report the next item's start.
            super::progress::stop_loaded_reporting(daemon);
            crate::output::advance::begin_advance(daemon);
        }
        Some(EndReport::ReportError) => {
            super::progress::stop_loaded_reporting(daemon);
            report_receiver_error(daemon);
        }
        None => {}
    }
}

/// Reports the receiver's failure to play the loaded item (spec "Receiver
/// fails to play the item"): the failure is named to every Client and nothing
/// is advanced or loaded.
fn report_receiver_error(daemon: &mut DaemonLoop) {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return;
    };
    let item = cast
        .loaded
        .as_ref()
        .and_then(|loaded| daemon.owner.core.queue.slot(loaded.slot))
        .map(|slot| slot.item.display_name());
    let report = match item {
        Some(name) => format!("\"{name}\" failed to play on the receiver"),
        None => "the receiver failed to play the loaded item".to_string(),
    };
    tracing::warn!(
        name: "cast.playback.failed",
        target: "cast",
        report = %report,
        "the receiver reported an error for the loaded item"
    );
    crate::core::broadcast(&daemon.ctrl_clients, &CtrlEvent::CommandRejected(report));
}
