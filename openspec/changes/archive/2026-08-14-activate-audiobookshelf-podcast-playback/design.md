## Context

See `proposal.md` and the delta specs. #513 provides the provider-specific inert Audiobookshelf action handler. #516 provides the queued item, typed identity, persistence, Service-aware admission, and owner-local context seam while keeping all owners ineligible. #517 provides validated direct/HLS session preparation and owner-driven active-file projection, still behind that gate.

This final child enables only the in-process bare-mode owner after reporting and finalization exist. Audiobookshelf progress differs from Emby: it uses current position, duration, and incremental wall-clock listening time, while playback `sessionId` is ephemeral lifecycle state.

## Goals / Non-Goals

**Goals:**

- Enable exactly one complete owner capability rather than exposing partial source support.
- Account for listening time without pause, seek, speed, or retry distortion.
- Make all lifecycle exits converge on ordered bounded finalization.
- Reconcile acknowledged progress to queue and browse state.
- Wire provider-specific episode play and enqueue actions.

**Non-Goals:**

- Audiobookshelf ctrl transport, Local daemon playback, Socket.IO, remote routing, or audiobooks.
- Reworking Emby/Feed reporting semantics.
- Moving credentials or lifecycle state into App browse code.

## Decisions

### 1. Enable admission only after the lifecycle is complete

The in-process Player advertises semantic Audiobookshelf playback eligibility only while its current generation-tagged context, prepared-source support, and reporting/finalization lifecycle are all available. Every ctrl owner remains ineligible; no ctrl capability is added.

Explicit unsupported submission reports failure without local fall-through. Composed editing remains unrestricted. Credential rejection clears context and makes Bound admission ineligible while preserving repairable staged/persisted snapshots; confirmed replacement/removal additionally purges them per #516.

### 2. Use a closed active-item lifecycle strategy

Replace the Emby-or-none reporting branch with an active-item lifecycle enum: Emby, Audiobookshelf, or None. It lives beside active slot state in `PlaybackRun`, which already owns periodic events, pause/seek, transitions, EOF, load failure, and shutdown.

The Audiobookshelf variant stores a redacted context snapshot, setup generation, `sessionId`, duration, last acknowledged position, monotonic playing-time accumulator, and at most one in-flight synchronization. A trait object or optional Emby fields are unnecessary for this closed set.

### 3. Accumulate wall-clock time only in Playing state

At each monotonic observation, add elapsed wall-clock time only if mpv remained Playing since the prior observation. Pause and seek first account for elapsed playing time up to the event, then synchronize the resulting position; pause duration and seek distance add nothing. Playback speed does not scale wall-clock listening time.

At dispatch, move the accumulator into one request and clear it. An ambiguous result does not restore that interval, preferring undercounting to duplicate listening statistics. The synchronization cadence remains the existing bounded reporting policy rather than a protocol contract.

### 4. Serialize finalization before the next session

One idempotent finalization path drains any ordered in-flight report within a bound, snapshots final position and undispatched listening time, sends final synchronization/close, and clears lifecycle state regardless of outcome. EOF, stop, skip, slot change, queue replacement, active removal, source failure, Service invalidation, run shutdown, and process teardown all use it.

Normal transitions do not open the next Audiobookshelf session until finalization completes or exhausts its budget. Teardown uses the existing bounded join budget and cannot hang. The mechanism may use existing worker/channel primitives; no new trait or serializable command shape is mandated.

### 5. Emit acknowledged provider-qualified progress

Successful synchronization emits provider-qualified identity, acknowledged position/completion, and setup generation from the Player boundary. App reconciliation updates matching canonical slots and Audiobookshelf browse progress only when the generation remains current. Filter membership derives from the updated progress; no polling or Socket.IO is introduced.

### 6. Wire actions only through #513's provider handler

The Audiobookshelf browse handler extracts the selected downloaded episode plus its read-only progress snapshot and builds the provider-native QueueItem. Ordinary play selects/creates and submits a slot; enqueue mutates the chosen Composed or eligible Bound queue without starting playback. Shows and unavailable rows remain inert.

Alternative rejected: reuse Emby `select()` / `current_lib_item()`, add negative provider guards, or expose source credentials to browse code.

## Risks / Trade-offs

- **[Risk] Sync and close reorder or duplicate listening time** -> Permit one ordered request in flight and clear intervals at dispatch.
- **[Risk] Service invalidation races late completions** -> Carry setup generation on preparation, reporting, finalization, and App reconciliation.
- **[Risk] Final close delays transition** -> Bound all waits and proceed after the budget while clearing stale lifecycle state.
- **[Risk] Activation bypasses owner admission** -> Submit only through the canonical queue/owner boundary established by #516.
- **[Trade-off] Ambiguous sync can lose one interval** -> Prefer undercounting to duplicate server statistics.

## Migration Plan

1. Confirm #513, #516, and #517 are applied and the in-process owner is still ineligible.
2. Add the active-item lifecycle and listening-time accounting behind the gate.
3. Add ordered finalization and generation-safe progress events.
4. Enable the in-process owner capability.
5. Wire provider-specific play/enqueue and local progress reconciliation.
6. Verify all lifecycle exits and full bare-mode direct/HLS playback.

Rollback disables owner eligibility and activation first; dormant queue/source support from #516/#517 remains safe.
