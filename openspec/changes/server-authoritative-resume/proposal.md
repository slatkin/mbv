# Proposal

## Why

Quitting mbv part-way through an Emby video and relaunching restarts it at
0:00, even though Emby has the right position saved. Play takes the Emby start
position from mbv's local queue copy (`stay_alive_queue_state.json`). That copy
is written before shutdown stops the player, so it never sees the quit position,
and restoring it never asks Emby. mbv runs on several machines against central
servers. Each Service's server must be the only record of progress for its
media. Only feeds, which have no server, keep progress locally.

## What Changes

- **Emby: play reads the start position from Emby.** Whenever the Player
  decides where an Emby video entry starts (queue load, append, slot jump,
  Next/Previous, Next-Up), it fetches the item from Emby and applies the
  existing 1% resume rule to the server's position. A failed fetch is retried
  automatically up to 3 times (500ms, then 2s). After that, play starts from
  0:00 and never fails because of it. Audio items are not fetched, since they
  never resume.
- **Audiobookshelf: no play change.** Opening a playback session already
  returns the server's `currentTime` and uses it as the start position.
- **Feeds: no change.** Their progress stays local.
- **Persisted queue state stores no Emby or Audiobookshelf positions.** Saving
  and loading clear those items' playback positions. Feed positions are kept.
- **The Owner refreshes Service progress from the servers on startup
  restore**, not only on cold adoption. Refreshes take the server's position
  as-is: no max(fetched, stored) merge.
- **Audiobookshelf parity:** the queue progress refresh (cold adoption, manual
  queue refresh, Owner restore) now also refreshes Audiobookshelf episode and
  book slots, not only Emby. The bounded shutdown stop also lets an active
  Audiobookshelf session finalize before exit.
- **Removed:** pending-sync protection (`pending_sync`, `StopReportOutcome`,
  the `progress_report_accepted` plumbing, and its requirement and invariant).
  It protected a local progress store, and that store is no longer used as a
  source of truth.
- **Shutdown waits for the stop report.** Owner shutdown uses
  `stop_for_shutdown` with a bounded deadline, so the final stop report is sent
  before `process::exit`. The queue is persisted after the player stops.
- **Behaviour change:** going back to an Emby slot resumes from whatever Emby
  holds, not from a locally remembered position. Emby already receives the
  short-play position through progress pings, so the old local "keep 20 minutes"
  result only ever existed on one machine.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `playback-resume`: adds where the resume position comes from (Service server
  for Emby and Audiobookshelf, local for feeds) and how a failed Emby fetch is
  handled.
- `unified-playback-queue`: persisted state drops Service positions; the
  progress refresh covers Audiobookshelf as well as Emby, also runs on Owner
  restore and on a manual queue refresh, and takes server values as-is;
  pending-sync protection is removed; "a slot resumes the same however it is
  reached" now resumes Emby from the server.
- `local-daemon-stay-alive`: an explicit stop delivers the final stop report
  within a bounded time before exit, and persists after stopping.
- `daemon-lifecycle`: the coordinated-shutdown snapshot persists playback
  positions only for feeds; Emby and Audiobookshelf positions are held by
  their servers.
- `daemon-disconnect-handling`: crash "Restart and resume" restores the queue
  from the snapshot, and the position comes from the server at play time
  (from the snapshot for feeds).

## Impact

- `crates/mbv-player`: start-position decisions at load, append and jump; a
  retrying Emby position fetch.
- `crates/mbv-queue`: removes `pending_sync`, `StopReportOutcome`, and the
  max-merge in `merge_fetched_slot`.
- `crates/mbv-config`: clears Service positions on queue-state save and load.
- `crates/mbv-daemon`: `handle_shutdown` ordering and deadline; enrichment on
  restore; Audiobookshelf progress in the queue refresh; removes
  report-acceptance plumbing.
- `crates/mbv-ctrl`: drops the `progress_report_accepted` wire field. It was
  `#[serde(default)]`, so older peers stay compatible.
- `src/app`: test-only. Production destructures `Stopped` with `..` and
  never reads the removed field.
- Docs: retires `docs/invariants/02-pending-sync-protection.md`; updates
  invariant 6 (one-shot resume start).
