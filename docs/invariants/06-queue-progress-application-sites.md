# Invariant 6 — mpv's one-shot resume start

**Scope:** mpv playlist entry resume options, native playlist navigation,
and owner-side relative steps and jumps (`ProgressObservation` in
`crates/mbv-queue` and `PlayerOwnerState::relative_step_target` in
`crates/mbv-player`).

## The invariant

mpv bakes a playlist entry's `start=<seconds>` resume option in once, at
`loadfile` time, and never re-reads or updates it afterward. Navigating back to
that entry with `playlist-pos`—including navigation initiated natively in
mpv—reopens it from that frozen value, not from its latest recorded position.
A re-visited entry therefore needs an explicit absolute seek after mpv has
loaded it; an immediate seek after `playlist-pos` is rejected because no file
is loaded yet. The Playback run's one-shot first-restart handling is the
loaded-entry point for this seek.

The seek target is not the canonical queue's stored position for every kind.
One per-kind decision chooses the source: `resume_source` in
`crates/mbv-player/src/resume.rs` returns `ResumeSource::Server` for an Emby
video, `Session` for an Audiobookshelf episode or book, `Local` for a Feed
entry, and `Beginning` for Emby audio. Every other resume decision matches on
that source and never on the item kind, and a new kind fails to compile until
it names its source. A `Server` slot is refreshed from Emby at jump time and
arms the jump with the refreshed `resume_ticks_for_item`, so the re-visit seeks
to the server's current position rather than the owner's loaded value. A
`Local` slot seeks to the owner's loaded ticks. A `Session` slot takes its
position from the item's open session, so `override_jump_resume` skips it. A
`Beginning` slot arms no resume seek.

A resume position read at a cold start must include the previous run's stop
report, so a run drains its report worker before its thread ends. After the
loop returns and the mpv child is dropped, `run()` calls
`SessionReporter::settle(REPORT_SETTLE_BOUND)`, which queues a
`ReportJob::Barrier` behind the run's reports and waits a bounded time for the
ack. The worker is FIFO, so the ack means every report queued before the
barrier was sent. On timeout the fetch runs anyway, as the fetch rule
requires. The tests `barrier_acks_only_after_the_jobs_ahead_of_it_ran` and
`zero_bound_settle_returns_without_the_ack` in
`crates/mbv-player/src/report_worker/tests.rs` pin those two rules.

Progress positions are interpreted by the shared `ProgressObservation`, and
relative navigation targets are resolved by `PlayerOwnerState::relative_step_target`.
There remains a narrow race: a jump dispatched before the owner applies that
item's preceding `TrackCompleted` observation can carry the prior position.
