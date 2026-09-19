## Why

When a Local daemon (Stay-Alive) or packaged `mbvd` owner starts cold and
adopts a client's persisted queue snapshot (`UnifiedAdoptQueue`,
`crates/mbv-core/src/daemon_control.rs:126-180`), it installs the on-disk
items verbatim (`PlaybackQueue::from_queue_items`) with no fetch to Emby. The
client does run a best-effort async enrichment after adoption
(`spawn_enrich_queue_state`, `src/app/queue_actions_playlist_mutation.rs:304`),
but it only merges the result into the client's own local `PlaybackQueue`
mirror (`merge_refreshed_queue`, `src/app/queue_scope.rs:176`) — it never
pushes refreshed positions back into the daemon's canonical queue (only
pruned/removed slots get synced back, via `QueueRemove`). The daemon owner is
authoritative for a Bound queue, so its next broadcast (any subsequent
`TrackChanged`/status event) replaces the client's locally-enriched values
with its own still-stale copy.

The user-visible result: every queue item that hasn't been played yet this
session shows no progress in the QueueList, even when it genuinely has
resume progress on the Emby server, because nothing ever refreshes the
daemon's own copy of that data. An item only starts showing (and then keeps)
correct progress once it is actually played, because playing it is the only
thing that writes a correct value into the daemon's canonical queue
(the already-fixed `TrackCompleted`/`Stopped` progress-application path, see
`docs/invariants/06-queue-progress-application-sites.md`).

## What Changes

- The daemon's `UnifiedAdoptQueue` handler spawns an async Emby fetch (the
  daemon's own `EmbyClient`, `get_items_by_ids`) immediately after adopting a
  persisted queue snapshot, on the same trigger and shape as the client's
  existing `spawn_enrich_queue_state`.
- A new `DaemonEvent` variant carries that fetch's result back into the
  daemon's single-threaded event loop (the fetch itself must happen off the
  event-loop thread).
- The daemon's event loop merges the fetch result into its own canonical
  queue using the existing `PlaybackQueue::merge_refresh` (already used by
  the client; never called anywhere on the daemon side today), then
  broadcasts the refreshed queue state to attached clients via the existing
  `broadcast_queue_state`.
- The client's post-adoption `spawn_enrich_queue_state` call is removed for
  the daemon-adoption path (`src/app/daemon_restart.rs`, `src/app/construct.rs`):
  once the daemon performs and broadcasts its own refresh, the client
  refreshing its own now-superseded-anyway local mirror is redundant work
  and a second source of the exact race this change closes. The plain local
  (Bare-mode, no daemon) restore path (`restore_queue_state` in
  `src/app/queue_actions_playlist_mutation.rs`) is unaffected — it has no
  daemon to defer to and keeps its existing client-side enrichment.
- The per-item "saved positions" override (`QueueState.positions`: a resume
  position saved locally at quit time, used to override Emby's UserData when
  it may still be lagging a just-sent `Stopped` report by a few seconds) is
  dropped for the daemon-adoption path rather than threaded through the wire.
  See design.md for the reasoning.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `unified-playback-queue`: adds a requirement that a Player owner cold-adopting
  a persisted queue snapshot asynchronously refreshes that queue's progress
  against the owning Service before treating it as settled, and that the
  refresh reaches the canonical queue (not only a client-side snapshot).

## Impact

- `crates/mbv-core/src/daemon_control.rs` — `UnifiedAdoptQueue` handler spawns
  the enrichment fetch.
- `crates/mbv-core/src/daemon_run.rs` — new `DaemonEvent` variant and its
  handler (merge + broadcast).
- `crates/mbv-core/src/playback/queue.rs` — no new logic; `merge_refresh`
  gets its first daemon-side caller.
- `src/app/daemon_restart.rs`, `src/app/construct.rs` — remove the
  now-redundant post-adoption `spawn_enrich_queue_state` call.
- `src/app/queue_actions_playlist_mutation.rs` — no change to the plain-local
  restore path; doc comment on `LocalDaemonBootstrap.positions` updated to
  reflect that the daemon-adoption case no longer uses it (see design.md).
- Tests: a daemon-side test proving `UnifiedAdoptQueue` triggers a merge that
  updates canonical queue positions and broadcasts them (mocked Emby client,
  no live network); update/remove client-side tests that asserted the
  now-removed post-adoption enrichment call.
- Emby-only. No Audiobookshelf, Feed, or Cast behavior changes.
