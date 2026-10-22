//! Run-lifecycle facts for one playback run, held in place of loose flags on
//! `PlaybackRun` (design D1 of the `playback-run-lifecycle` change).
//!
//! The module owns the "has playback begun" boundary: a generic mpv failure
//! before the first `PlaybackRestart` is a start failure and stops the run
//! with an error (#941); after it, mpv owns skipping a failed entry and the
//! playlist-position drift machinery follows it — unless no entry follows
//! the failed one, where the run still ends with the error (#942, #943).
//! It also owns which projection feeds mpv: the active-file projection keeps
//! exactly one entry in mpv's playlist and never returns to the playlist
//! projection. The facts take plain values (`PlaybackRestart` observed, a
//! queue of entries), never a raw `MpvEvent`.

use super::ExecutionSequence;

#[derive(Copy, Clone, Debug, Default)]
enum Projection {
    /// mpv's playlist carries the run's entries, and `playlist-pos` names
    /// one of them.
    #[default]
    Playlist,
    /// mpv holds only the entry playing (each item is reloaded directly),
    /// so `playlist-pos` never names the run's ordinal.
    ActiveFile,
}

/// The lifecycle facts of one submission: the "has playback begun" boundary
/// and the projection that decides whether anything follows a failed entry.
/// `PlaybackRun` holds this in place of the `playback_began` and
/// `active_file` flags it produced (design D1).
#[derive(Debug, Default)]
pub(crate) struct RunLifecycle {
    /// Whether any `PlaybackRestart` has arrived in this run — the "has
    /// playback begun" boundary. A generic mpv failure before it is a start
    /// failure and stops the run with an error (#941); after it, mpv owns
    /// skipping a failed entry and the playlist-position drift machinery
    /// follows it — unless no entry follows the failed one, where the run
    /// still ends with the error (#942, #943).
    began: bool,
    projection: Projection,
}

impl RunLifecycle {
    /// Records a `PlaybackRestart`: playback began in this run.
    pub(crate) fn playback_started(&mut self) {
        self.began = true;
    }

    /// Builds the initial state for a submitted queue: the active-file
    /// projection when the queue holds any Audiobookshelf entries (the
    /// predicate that selects that branch at `PlaybackRun::init_from_queue`),
    /// otherwise the playlist projection. `began` stays false: no
    /// `PlaybackRestart` arrived in the new run.
    #[must_use]
    pub(crate) fn for_queue(queue: &ExecutionSequence) -> Self {
        Self {
            began: false,
            projection: if queue.has_audiobookshelf_entries() {
                Projection::ActiveFile
            } else {
                Projection::Playlist
            },
        }
    }

    /// Switches the run to the active-file projection. Idempotent; the
    /// projection never returns to `Playlist`.
    pub(crate) fn enter_active_file(&mut self) {
        self.projection = Projection::ActiveFile;
    }

    /// Whether the run is in the active-file projection.
    #[must_use]
    pub(crate) fn is_active_file(&self) -> bool {
        matches!(self.projection, Projection::ActiveFile)
    }

    /// Whether playback began in this run (the #941 boundary).
    #[must_use]
    pub(crate) fn began(&self) -> bool {
        self.began
    }
}
