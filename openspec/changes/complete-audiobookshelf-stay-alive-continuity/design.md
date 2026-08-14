## Context

See `proposal.md` and the delta specs. The client-facing acknowledged-progress loop is the only piece of milestone 4 still open:

- The ctrl event and capability-gated broadcast plumbing exist from #525. `crates/mbv-core/src/daemon_core.rs::broadcast_audiobookshelf_progress` serializes `CtrlEvent::AudiobookshelfProgress` and calls `broadcast_progress_gated`, but it is `#[allow(dead_code)]` and has zero callers — no active owner path emits it.
- The client already decodes the wire event: `remote_player_connect.rs` maps `CtrlEvent::AudiobookshelfProgress` to `PlayerEvent::AudiobookshelfProgress`. But `src/app/player_event.rs` handles that variant with a dormant stub: `// Dormant: no browse-reconciliation consumer exists yet.`
- The reconciliation itself already exists for bare mode. `src/app/lib_event_actions.rs` handles `LibEvent::AudiobookshelfProgressAcknowledged`: it gates on `audiobookshelf_runtime.accepts(generation)`, matches queue slots by `(library_item_id, episode_id)`, calls `queue.apply_progress(...)`, writes each `audiobookshelf_browse[*].progress`, and persists via `save_queue_state()`.
- Live-queue adoption authority is already generic over `QueueItem`. Tests such as `local_daemon_app_keeps_live_queue_over_stale_disk_snapshot` and `local_daemon_keeps_live_queue_when_shared_snapshot_arrives` prove the daemon's live queue wins over a saved snapshot; #525 already routes Audiobookshelf items through unified transport and reconnect adoption.

So #528 is small: emit at one owner point, consume at one client point by reusing the existing apply path, and prove the whole matrix.

## Goals / Non-Goals

**Goals:**

- Emit acknowledged Audiobookshelf progress from active daemon owner playback through the existing gated broadcast helper.
- Reconcile that event on capable clients by reusing the bare-mode apply path — one shared reconciliation, two entry points (bare `LibEvent`, daemon `PlayerEvent`).
- Keep reconciliation generation-gated and identity-qualified; drop stale-generation events.
- Verify stay-alive continuity and live-queue adoption for Audiobookshelf items across the full lifecycle matrix.

**Non-Goals:**

- Socket.IO live refresh, audiobook playback, multiple Audiobookshelf servers, per-client credentials, credential transport over ctrl, cross-provider browsing.
- Any change to owner-side source resolution, listening-time accounting, or session finalization delivered by #527.
- Any protocol-version bump or new capability string; #525's capabilities are reused unchanged.

## Decisions

### 1. Emit from the owner's existing acknowledged-progress point

`broadcast_audiobookshelf_progress` is invoked at the same owner point where #527 acknowledges progress with the Audiobookshelf server (periodic sync accepted, and final progress at completion). The acknowledged position/completion and current setup generation are already in hand there; emission adds one call and no new lifecycle. This removes the `#[allow(dead_code)]` marker.

The emit is intentionally driven by *acknowledgement*, not by every local tick, so clients reconcile only values the server accepted — matching the "acknowledged" contract already in the ctrl and playback specs.

### 2. Fill the client stub by reusing the bare-mode apply path

`PlayerEvent::AudiobookshelfProgress(event)` converts the ctrl `AudiobookshelfProgressEvent` into the same `AudiobookshelfProgressUpdate` the bare-mode path applies, and routes it through the identical reconcile used by `LibEvent::AudiobookshelfProgressAcknowledged` (queue-slot match by identity, `apply_progress`, browse-progress write, `save_queue_state`), gated by `audiobookshelf_runtime.accepts(generation)`.

Reusing the existing function is the root-cause-correct fix: one reconciliation body serves both the bare owner and the daemon client, so completion/resume/filters behave identically regardless of who owns playback. A parallel daemon-only reconcile was rejected as duplicate logic that would drift.

### 3. Adoption relies on the existing generic authority, verified for Audiobookshelf

No new adoption mechanism. The client's existing "live daemon queue wins over saved snapshot" path already carries Audiobookshelf slots (via #525 transport); #528 adds targeted tests proving a later client adopts the live Audiobookshelf queue, active slot, status, and last-acknowledged progress, and reconciles browse state on adoption. Browse reconciliation on adoption reuses the Decision 2 apply path against the adopted queue's acknowledged values.

### 4. Continuity is a property of existing stay-alive plus Decisions 1–2

Owner playback already survives client exit (`local-daemon-stay-alive`). #528 does not add continuity machinery; it verifies that Audiobookshelf sync/finalization continue with no client and that emission resumes to a later capable client. Mixed-version and multi-client coherence fall out of #525's per-connection gating, re-covered by tests.

## Risks / Trade-offs

- **[Risk] Emission fires for an episode a reattaching client has not yet adopted** -> The client reconcile is a no-op when no slot matches (spec scenario), so an early event is harmless; adoption then supplies the live position.
- **[Risk] A stale-generation event reconciles after setup replacement** -> `audiobookshelf_runtime.accepts(generation)` already drops superseded generations on both entry points; covered by `stale_audiobookshelf_progress_ack_is_ignored_after_generation_advance`-style tests extended to the daemon route.
- **[Risk] Double reconciliation if bare and daemon paths both fire** -> A given run is exactly one owner: bare in-process OR daemon client. The daemon client has no local Audiobookshelf player emitting `LibEvent` acks, so only one entry point is live per process.
- **[Trade-off] Emit is coupled to the server-acknowledgement cadence, not mpv ticks** -> Clients see server-accepted values only, at sync cadence, which is the intended "acknowledged progress" semantics and avoids leaking un-acknowledged positions.

## Migration Plan

1. Invoke `broadcast_audiobookshelf_progress` at the owner acknowledged-progress point; drop the dead-code marker. No client change yet — capable clients simply begin receiving events the stub still ignores.
2. Replace the client `PlayerEvent::AudiobookshelfProgress` stub with the shared reconcile (Decision 2).
3. Add tests: client reconcile from daemon event (match/no-match/stale generation), browse filter reconcile, live-queue adoption of Audiobookshelf items, continuity across client exit/reattach, mixed-version gating, and explicit shutdown finalization.

Rollback restores the client stub and removes the owner emit call; the ctrl event and capabilities from #525 remain and go dormant again with no protocol change.
