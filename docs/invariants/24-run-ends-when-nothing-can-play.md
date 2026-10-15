# Invariant 24 — A run never stays active with nothing left to play

## The invariant

1. A run that cannot play anything ends, and reports one `Stopped` naming the
   error. It does not sit marked active while mpv is idle.
2. A run ends this way only when mpv has no entry after the failed one to skip
   to. With an entry after it, the failure is not a run-ending error: mpv
   advances, and the playlist-position drift machinery follows.

## Why it matters

Nothing enforces this in a type. `PlayerEvent::Stopped` is the only signal that
ends a run in the Owner, and every stop path must send it for its own case. mpv
has no obligation to say "I am idle and I have nothing to do": it emits `idle`,
mbv decodes that event to `Ignored`, and `playlist-pos` goes to -1, which the
drift machinery drops. So the run's own state is the only witness that playback
stopped. A run that misses that witness keeps `status.active = true` forever:
the now-playing panel freezes, the queue stays bound, and the user sees no
error, because the failure that caused it reached no error path.

## How the code maintains it today

- An mpv load error arrives as `end-file` with `reason=error`, which decodes to
  `MpvEvent::Failed(MpvFailure::Other(..))` — **not** `MpvEvent::EndFile`. So
  the end-of-file paths (`on_end_file`, `on_end_file_queue`, `stop_at_queue_end`,
  `on_end_file_standalone`) never see an error, and `stop_at_queue_end` is
  unreachable for a failed entry. Every error exit lives in `on_mpv_error`
  (`crates/mbv-player/src/run/events/termination.rs`).
- `stop_for_unrecoverable_error` (same file) is that path for a generic error.
  It stops the run when `playback_began` is false (#941, a file that never
  started), and when mpv has no entry after the failed one (#942 a failed last
  queue entry, #943 a failed single-file run). mpv holds a playlist of its own
  only outside the `active_file` projection, which clears it to the single
  entry playing (`install_active_projection`, `crates/mbv-player/src/run/queue.rs`),
  so an `active_file` run always stops. `current_idx` is the failed entry: the
  advance to it settled before mpv tried to load it.
- `playback_began` (`crates/mbv-player/src/run/types.rs`) is set on the first
  `PlaybackRestart` (`run/events/restart.rs`) and is the boundary between the
  two cases.
- A run that ends this way has its `QueueSlotId` resolved before the event
  leaves the run, so the Owner can retire the occurrence (design D2).

## Where it currently fails

Known limits, none observed:

- A jump whose target fails to load leaves `forced_jump` armed, so
  `current_idx` is still the outgoing entry when the error arrives. The
  no-next-entry test then reads the outgoing entry's successor, not the
  failed target's. A run can also stay in that armed state for other reasons,
  so the target-failure case needs its own observation.
