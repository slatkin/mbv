//! The Cast output's provider progress reporting (cast-owner-output design
//! D9, row 6.4): the Emby half. The reporter for an Emby target is the
//! player's `SessionReporter`, built from the `CastProgressTarget` ids the
//! resolve returned and fed from the presented status the Owner publishes
//! (design D8) — no second reporter is written. Every report is queued on
//! the reporter's own worker, so the event loop never waits on the network.
//!
//! The reporting drives only while the receiver's reported media is the item
//! the Owner loaded (spec "Progress is reported while mbv is attached"): the
//! same content-id gate the presentation uses gates the feed. The session
//! start goes out once, on the first status that presents the loaded item —
//! the Cast counterpart of the run's `PlaybackRestart` start — and the stop
//! goes out for an ended, stopped, or replaced item, always before any later
//! start report (design D9's output-change ordering).

use std::sync::Arc;

use mbv_ids::ItemId;
use mbv_queue::QueueSlotId;

use super::super::DaemonLoop;
use super::PlaybackOutput;

/// The provider reporting one loaded Cast item drives (design D9). The seam
/// keeps the receiver-driven reporting testable: the real reporter is the
/// player's `SessionReporter`, and a test injects a recording double.
pub(crate) trait CastProgressReporting: Send {
    /// Reports the session start for the loaded item (design D9).
    fn report_start(&self);
    /// Reports the loaded item's presented position (design D9).
    fn report_progress(&self);
    /// Reports the stop of the loaded item at `position_ticks` (design D9).
    fn report_stop(&self, position_ticks: i64);
}

/// The real reporter: the player's `SessionReporter` the local path already
/// runs, fed from the presented status instead of a run of its own. The
/// event name is the one the previous Cast reporting sent (`timeupdate`).
struct EmbySessionReporter {
    reporter: mbv_player::SessionReporter,
}

impl CastProgressReporting for EmbySessionReporter {
    fn report_start(&self) {
        self.reporter.report_output_start();
    }

    fn report_progress(&self) {
        self.reporter.report_output_progress("timeupdate");
    }

    fn report_stop(&self, position_ticks: i64) {
        self.reporter.report_output_stop(position_ticks);
    }
}

/// Builds the loaded item's provider reporting (design D9). An Emby target
/// gets a `SessionReporter` built from the target's ids, reading the
/// presented status the Owner publishes; the Audiobookshelf lifecycle and
/// the Feed `PlayerEvent`s join in their own pieces of row 6.4, so neither
/// builds reporting here yet.
pub(crate) fn build_progress_reporting(
    daemon: &DaemonLoop,
    slot: QueueSlotId,
    progress: &crate::cast_resolve::CastProgressTarget,
) -> Option<Box<dyn CastProgressReporting>> {
    let crate::cast_resolve::CastProgressTarget::Emby {
        item_id,
        media_source_id,
        session_id,
        ..
    } = progress
    else {
        return None;
    };
    // The stop report zeroes an audio item's position the way the run's stop
    // reports do, so the item's is-audio comes from the queue slot itself.
    let is_audio = daemon
        .owner
        .core
        .queue
        .slot(slot)
        .is_some_and(|slot| slot.item.is_audio());
    Some(Box::new(EmbySessionReporter {
        reporter: mbv_player::SessionReporter::new(
            Arc::new(daemon.client.lock().unwrap().clone()),
            daemon.ws_send_tx.clone(),
            ItemId::new(item_id.clone()),
            media_source_id.clone(),
            session_id.clone(),
            is_audio,
            Arc::clone(&daemon.player.status),
        ),
    }))
}

/// Feeds the loaded item's provider reporting from the receiver's status
/// (design D9, spec "Progress is reported while mbv is attached"). The first
/// status that presents the loaded item reports the session start, and every
/// such status reports the presented position — the reporter snapshots them
/// from the status the preceding presentation refresh installed. A status
/// that presents media the Owner did not load reports nothing. Called after
/// the presentation refresh of one applied status report.
pub(super) fn report_presenting_status(daemon: &mut DaemonLoop) {
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    let super::CastOutput {
        loaded,
        last_status,
        ..
    } = &mut **cast;
    let (Some(loaded), Some(status)) = (loaded.as_mut(), last_status.as_ref()) else {
        return;
    };
    if !super::status::presents_loaded_item(loaded, status) {
        return;
    }
    let Some(reporting) = loaded.reporting.as_ref() else {
        return;
    };
    if !loaded.start_reported {
        reporting.report_start();
        loaded.start_reported = true;
    }
    reporting.report_progress();
}

/// Reports the stop of the loaded item and drops its reporting (design D9):
/// the position is the presented position — read before the presentation is
/// replaced — and the report goes out only for a started session, so a
/// receiver that never confirmed the load leaves no session trail. Called
/// ahead of every step that ends the loaded item's session: the advance's
/// next load, a user stop, and (from its own piece) an output change.
pub(crate) fn stop_loaded_reporting(daemon: &mut DaemonLoop) {
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    let Some(loaded) = cast.loaded.as_mut() else {
        return;
    };
    let position_ticks = daemon.player.status.lock().unwrap().position_ticks;
    if loaded.start_reported
        && let Some(reporting) = loaded.reporting.take()
    {
        reporting.report_stop(position_ticks);
    }
}
