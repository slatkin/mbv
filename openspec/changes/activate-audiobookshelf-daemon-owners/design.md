## Context

See `proposal.md` and the delta specs. The three preparatory children are done:
- #525 landed capability strings, ctrl wire types, capability-gated queue/progress transport, and the dormant `broadcast_audiobookshelf_progress` helper (annotated `#[allow(dead_code)]` pending this child).
- #526 landed `AudiobookshelfOwnerContext` (loaded setup, API key, `SetupGeneration`, `device_id`, persisted `revision`), the `ApplyServiceSetup` reconciliation path, and `mbvd --connect/--disconnect abs` administration. The daemon holds a live `Option<AudiobookshelfOwnerContext>` in `daemon_run` as `audiobookshelf_runtime`, but every ABS queue admission path remains rejected.

Bare-mode ABS playback is complete: `prepare_source` in `player_sources.rs` builds `AudiobookshelfPlayerContext` from runtime setup/key/generation, opens a playback session via `create_playback_session_bounded`, resolves the direct or HLS URL, sends Bearer per-file, and wires periodic sync through an `mpsc::Sender<AudiobookshelfProgressUpdate>`. Drain logic in `run_loop_drains.rs` consumes acknowledged updates to refresh queue slots and browse state. The `broadcast_audiobookshelf_progress` seam is the only missing link to capable clients.

## Goals / Non-Goals

**Goals:**
- Activate daemon owner admission when `audiobookshelf_runtime` is `Some` and the submitting client negotiated `abs-queue`.
- Construct `AudiobookshelfPlayerContext` from `audiobookshelf_runtime` and route it through the daemon's mpv projection the same way bare mode does.
- Wire `AudiobookshelfProgressUpdate` from the daemon's player progress sender to both the canonical Bound queue (provider-qualified slot update) and `broadcast_audiobookshelf_progress`.
- Trigger bounded ABS finalization before setup replacement/removal in the existing reconciliation path.
- Remove the `#[allow(dead_code)]` annotation from `broadcast_audiobookshelf_progress`.

**Non-Goals:**
- Client-side `PlayerEvent::AudiobookshelfProgress` reconciliation and live-queue adoption on attach (that is #528).
- Socket.IO, audiobooks, multiple ABS servers, credential transport over ctrl.

## Decisions

### 1. Daemon admission gate: `audiobookshelf_runtime.is_some()` at submission time, plus `abs-queue` capability

A daemon owner admits an `ABS QueueItem` only when `audiobookshelf_runtime` is `Some` (owner has installed setup) AND the submitting client's Hello advertised `abs-queue` (transport capability is negotiated). The `abs-queue` check is the existing per-connection gate already enforced for outbound queue state; the admission predicate mirrors the same variable.

Alternative: check against a separate `capable_client_count` counter. Rejected: the submitting client's own capability is the correct atomic check; reusing the per-client gate is the minimal change and avoids a counter staying stale.

### 2. `AudiobookshelfPlayerContext` built from `audiobookshelf_runtime` inline at playback start

When a daemon owner starts an ABS slot, it constructs `AudiobookshelfPlayerContext::new(generation, setup.clone(), api_key.clone(), device_id.clone())` directly from `audiobookshelf_runtime`, mirroring what the bare-mode player loop does from `AudiobookshelfRuntime`. The daemon's `prepare_source` call and entire playback source path are unchanged — the context type is shared.

Alternative: a daemon-specific context type. Rejected: `AudiobookshelfPlayerContext` is the shared boundary; a parallel type would require duplicating source preparation and lifecycle logic.

### 3. Progress routing: `AudiobookshelfProgressUpdate` sender wired to daemon queue update + broadcast

The daemon player's progress sender (an `mpsc::Sender<AudiobookshelfProgressUpdate>`) routes updates through the daemon's event loop. On each acknowledged update, the loop:
1. Matches the canonical Bound queue slot by `(library_item_id, episode_id)` and updates position/completion (mirrors the bare-mode `run_loop_drains` path).
2. Calls `broadcast_audiobookshelf_progress` with the acknowledged event (the dormant helper now activated).

This is a direct port of the bare-mode drain into the daemon event loop, not a new abstraction. The ordering guarantees canonical queue reflects the server acknowledgement before broadcast.

Alternative: route updates through `shared_queue` → shared-state broadcast. Rejected: shared-queue is for cross-owner state; the Bound queue is daemon-owner-local authority. The Bound-queue update must happen before the broadcast to keep queue and progress coherent.

### 4. Finalization before setup replacement/disconnect

The `reconcile_packaged_audiobookshelf` path (reconciled via `ApplyServiceSetup`) already clears `audiobookshelf_runtime`. Extend it to first signal the active player to finalize any live ABS session within the teardown budget before dropping the context. The same bounded finalization path bare mode uses (via the `PreparedSource::close` lifecycle call) is reused; the daemon signals it through the existing player's shutdown coordination point.

The `--disconnect abs` path in `mbvd` already calls `reconcile_packaged_audiobookshelf` after removing durable state; this extension naturally covers the disconnect purge as well.

Alternative: a separate "finalize ABS" message to the player. Rejected: the player's existing teardown coordination already handles this on context clear; a new message adds protocol without behavior gain.

### 5. `broadcast_audiobookshelf_progress` is activated in place

Remove the `#[allow(dead_code)]` annotation. The function already has the correct signature and capability gate. No API or behavior change required.

## Risks / Trade-offs

- **[Risk] Daemon `audiobookshelf_runtime` is taken/moved during concurrent reconcile** → The daemon event loop is single-threaded for player/queue operations; `audiobookshelf_runtime` is mutated only on `ApplyServiceSetup` messages, which are serialized through the same loop. No concurrent access.
- **[Risk] Canonical Bound queue slot not found at broadcast time** → Match is by `(library_item_id, episode_id)`, same as bare mode. If the slot was removed before acknowledgement the update is silently skipped, matching bare-mode behavior.
- **[Risk] Finalization prolongs reconcile path** → The bounded finalization budget is unchanged from bare mode; teardown cannot block indefinitely.
- **[Trade-off] Admission requires both installed setup AND transport capability negotiated** → A daemon without any capable client attached cannot admit ABS items. This is correct: without transport capability there is no slot the client can reconcile, so admission would produce unobservable state.

## Migration Plan

1. Activate daemon ABS admission: add `audiobookshelf_runtime.is_some()` + `abs-queue` capability check to the submission predicate.
2. Construct `AudiobookshelfPlayerContext` from `audiobookshelf_runtime` at ABS slot playback start in the daemon player path.
3. Wire the daemon's `AudiobookshelfProgressUpdate` receiver to canonical Bound queue update and `broadcast_audiobookshelf_progress`; remove the `dead_code` allowance.
4. Extend `reconcile_packaged_audiobookshelf` to trigger bounded ABS finalization before context drop.
5. Cover admission gate, progress routing, and finalization-on-reconcile in tests; verify `cargo check`, `nextest`, `clippy`, and `make check-code-file-lines`.
