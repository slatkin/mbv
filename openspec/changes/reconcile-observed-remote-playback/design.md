## Context

See `proposal.md` for motivation and the delta specs for behavioral contracts.

Attached Emby sessions differ from local and direct-mbv playback: mbv can submit an ordered item list and poll `/Sessions`, but the resulting session record exposes only the current media item and play state. It does not expose the client's queue, queue index, or playlist occurrence identity. The current UI searches the local item list for the first matching media ID, which cannot distinguish duplicates and does not drive consume.

The existing attached-session path has four useful seams:

- sequence playback is sent through `session_play_items`;
- transport commands are routed through `RemotePlaybackTarget` and session command helpers;
- successful session polls converge in `handle_session_event`;
- playlist loads already preserve Emby's per-occurrence `PlaylistItemId` on `MediaItem`.

Current playlist saving replaces every playlist entry. That is unsuitable for observed remote consume because it can overwrite unrelated server-side edits. Emby's existing playlist API also accepts exact entry IDs for deletion, which provides a narrower mutation primitive.

## Goals / Non-Goals

**Goals:**

- Keep reconciliation rules deterministic, occurrence-aware, and independently testable from terminal rendering and network I/O.
- Centralize all generic-session sequence submissions and item-changing commands so tracking cannot silently miss mbv-originated intent.
- Preserve the immutable Submitted sequence while deriving current Position candidates and consume effects from bounded evidence.
- Make playlist consume exact-entry and idempotent where Emby exposes enough identity.
- Integrate exceptional tracking status and recovery into the queue panel without building an event-history subsystem.

**Non-Goals:**

- Discovering, mirroring, or controlling a generic Emby client's internal queue.
- Persisting Tracking sessions or unresolved playlist outcomes across process exit.
- Constraining shuffle, repeat, or controls used directly on the Emby client.
- Depending on multicast intent sharing from issue #427.
- Changing authoritative queue behavior for local playback, the local daemon, or direct mbv remotes.
- Introducing a general event-sourcing framework or user-facing observation log.

## Decisions

### 1. Use a pure occurrence-aware reconciliation model

Add a reconciliation model outside the TUI rendering and API layers. It owns an immutable snapshot of Submitted-sequence occurrences, each with a runtime occurrence ID, media ID, optional playlist entry ID, and source metadata needed for consume. It also owns:

- current Tracking state and reason;
- current Position candidates;
- the previous and current relevant Remote observations;
- one current Expected transition plus its lifecycle;
- Tracking-epoch evidence;
- consumed and Bypassed occurrence IDs;
- passive unresolved playlist outcomes.

The model receives timestamped observations and mbv intents and returns explicit effects such as state changes, current-position changes, inferred completion, invalidation, and available re-anchor targets. Network calls, UI messages, and playlist writes remain outside it.

Use occurrence identity rather than raw indexes for every retained decision. Indexes are derived views because consume and external playlist changes can shift visible positions.

Alternative considered: add conditionals directly to `handle_session_event`. Rejected because duplicate hypotheses, epoch transitions, and consume safety would become inseparable from polling and UI mutation.

Alternative considered: maintain a complete event journal. Rejected because only bounded evidence can affect the next reconciliation decision, and the specs require no history UI or persistence.

### 2. Keep the Submitted sequence immutable

The tracker never removes or reorders Submitted-sequence occurrences. Applied consume is recorded by occurrence ID, and re-anchor marks unresolved earlier occurrences Bypassed. This permits later rewind observations and prevents playlist mutation from rewriting playback history.

Manual queue editing terminates tracking after confirmation. Automatic consume does not terminate tracking because it changes the saved playlist, not the historical sequence sent to the client.

Alternative considered: mutate the tracker sequence alongside the saved playlist. Rejected because returning to an already consumed occurrence would then be indistinguishable from unrelated playback.

### 3. Centralize attached-session sequence submission and intents

Introduce one App-level submission path used by initial play, queue-cursor play, previous/next fallback that resends the list, and any other `session_play_items` caller. It performs three coordinated actions:

1. snapshots the exact ordered occurrences and requested start occurrence;
2. creates or replaces the process-local tracker in `STARTING`;
3. sends the Emby command and routes its success or failure back to the tracker.

Submitting a single item terminates any tracker for that remote target before dispatch because it replaces the tracked sequence without creating another one.

Item-changing transport actions create typed intents before dispatch. The minimum intent set is sequence submission, Next, Previous, direct occurrence selection, restart/seek-to-start, Seek, and Stop. Next, Previous, and direct selection identify a target occurrence and suppress completion inference for the source when confirmed. Seek and restart explain same-occurrence position regression without changing occurrence identity. Stop explains stopped playback without completion inference. A contradicted item-changing intent falls through to the Unprompted-transition rules; an expired intent with no item change leaves the current candidate unchanged. A newer incompatible intent supersedes the prior live intent. Intent expiry uses the existing session-poll timing rather than blocking command dispatch and continues while tracking is suspended.

Alternative considered: infer mbv commands afterward from UI state. Rejected because asynchronous dispatch and commands from multiple input paths would make provenance unreliable.

### 4. Preserve tick precision in remote observations

Extend parsed session state to preserve raw position and runtime ticks in addition to any existing second-based presentation fields. Reconciliation requires runtime greater than zero and uses an overflow-conscious integer equivalent of `position / runtime >= 19 / 20` for the inclusive 95 percent completion boundary.

A Remote observation includes a monotonic local poll generation, session identity, current media ID or stopped state, position ticks, runtime ticks, observed time, and available pause/play facts. Only observations newer than the last accepted generation reach reconciliation. Repeated same-item observations are accepted as evidence but create no transition without a meaningful item or position change, and only relevant prior/current observations are retained.

The existing single-flight poll path assigns generations when polls are started. A failed request changes no tracking state. A successful response that still includes the attached session with no current item is stopped playback, not disappearance. A successful response that omits the session uses the existing three-consecutive-miss policy before emitting disappearance and entering `SUSPENDED`. No inference is made for unobserved time between accepted generations.

Candidate evolution is deliberately local. From each prior candidate, an observation may retain that occurrence for same-media continuity, advance to its immediate successor for an item change, or select the exact target of a live Expected transition. A same-media reset with an immediate duplicate successor retains both current and successor candidates. Matching media IDs elsewhere in the sequence are not candidates unless reached by these edges or explicit re-anchor. After elimination, zero candidates means `INVALID`, one means `TRACKING`, and multiple mean `AMBIGUOUS`.

For consecutive duplicates, a same-media position reset retains the current occurrence and immediate duplicate successor as Position candidates and enters `AMBIGUOUS`; this change does not infer which occurrence started. A material same-occurrence reset or backward transition without an applicable mbv intent enters `INVALID`, where the existing re-anchor flow provides explicit recovery. Ordinary reporting jitter remains governed by the existing remote-position reconciliation tolerance; a decrease beyond that tolerance is material.

Alternative considered: continue using integer seconds. Rejected because truncation around exact percentage boundaries and short audio tracks creates avoidable classification errors.

### 5. Implement explicit state transitions and effect gating

The tracker implements the five spec states as a closed state machine:

```text
STARTING ── compatible observation ──▶ TRACKING
    │                                     │
    ├── session absent ───────────────▶ SUSPENDED
    └── contradiction/expiry ─────────▶ INVALID

TRACKING ── multiple paths ───────────▶ AMBIGUOUS
    │  ▲                                  │
    │  └──── unique path recovered ───────┘
    ├── session absent ───────────────▶ SUSPENDED
    └── unexplained jump ─────────────▶ INVALID

SUSPENDED ── exact/adjacent return ───▶ TRACKING
    ├────── multiple candidates ──────▶ AMBIGUOUS
    └────── non-adjacent/incompatible ▶ INVALID
INVALID   ── explicit re-anchor ──────▶ TRACKING (new epoch)
```

Only transitions emitted while `TRACKING` with one resolved occurrence can create new consume effects. `AMBIGUOUS`, `INVALID`, and `SUSPENDED` retain enough evidence for recovery but cannot promote uncertain or retroactive completion.

Remote stopped/idle remains separate from this health state machine: a stopped session can remain `TRACKING` at its most recent resolved anchor. Re-anchor selects one occurrence, starts a new epoch, and marks unresolved earlier occurrences Bypassed. Selecting a non-adjacent occurrence through mbv is an automatic anchor because the target intent supplies occurrence identity.

Alternative considered: numeric confidence scoring. Rejected because deterministic classes are explainable in the UI and produce testable consume boundaries.

### 6. Separate playback reconciliation from playlist application

Occurrence completion produces a consume request only when media-type consume is enabled and the Submitted sequence is associated with a saved playlist. A separate playlist-application path performs network work and reports one of:

- applied or already absent;
- unresolved and requiring passive status.

Playback tracking continues independently while an application is pending or unresolved. The tracker records that an occurrence has emitted a consume request so repeated observations cannot submit duplicate blind mutations.

Alternative considered: mutate the playlist directly from observation handling. Rejected because asynchronous writes, retries, and external edits would feed network uncertainty back into playback-position validity.

### 7. Delete exact playlist entries instead of replacing the playlist

Add an API operation that deletes one or more exact Emby playlist entry IDs through `DELETE /Playlists/{playlist_id}/Items?EntryIds={entry_id}`. Before deletion, perform one `GET /Playlists/{playlist_id}/Items?UserId={user_id}` reload and verify both `PlaylistItemId` and the media item `Id`. If the stable entry ID is absent, the desired removal is already applied. If it exists but identifies different media, or either identity is unavailable, do not mutate the playlist.

Immediately before deletion, re-check that the correlated tracker and epoch still exist in valid resolved tracking and that the occurrence remains eligible. This preserves unrelated additions, removals, and reordering. Otherwise, perform at most one exact-entry deletion attempt for the occurrence. HTTP success is applied; any transport or HTTP error is unresolved because the server may have acted before the response failed. An identity mismatch, failed reload, failed request, or unknown result creates a passive unresolved outcome rather than a retry or call to the existing full-replacement save path.

Alternative considered: reload, merge, and replace the whole item-ID list. Rejected because Emby provides no compare-and-swap revision and replacement recreates occurrence IDs while racing external edits.

### 8. Keep asynchronous outcomes correlated and process-local

Sequence submissions, playlist reloads, exact deletion, and verification return through correlated events carrying Tracking-session identity, epoch identity, and occurrence identity. A completion is ignored when any identity no longer matches the current tracker. This prevents a late response from an old target or epoch mutating a new session.

No tracking or unresolved-outcome state is added to queue-state persistence. On disconnect, target replacement, Stop Tracking, manual-edit confirmation, or application exit, the tracker is dropped. In-flight results become inert through identity checks.

Alternative considered: persist unresolved tracking and mutation state. Rejected because the agreed lifecycle intentionally treats the server playlist as authoritative after restart.

### 9. Put compact state and recovery in the queue panel

Extend the queue panel's existing title/source area with compact remote target and tracking state data. `TRACKING` and current position remain quiet. `AMBIGUOUS`, `INVALID`, and `SUSPENDED` add a concise reason; unresolved playlist-outcome count is passive. Re-anchor and Stop Tracking are queue-context actions. Duplicate re-anchor opens a small occurrence selector.

The Sessions panel only marks that the connected session has active tracking. It does not become a second management surface. This change does not add an interactive repair or review overlay.

Alternative considered: a dedicated tracking panel and history. Rejected as disproportionate to the process-local feature and likely to inundate users.

## Risks / Trade-offs

- **[Risk] Duplicate or repeat behavior cannot be resolved safely** → Enter `AMBIGUOUS` for plausible duplicate candidates and `INVALID` for unexplained resets or rewinds; require explicit re-anchor instead of adding temporal inference in this change.
- **[Risk] Emby reports stale or coarse positions** → Retain `STARTING` until confirmation, preserve tick values, and tolerate live Expected transitions.
- **[Risk] Full tracking logic makes attached-session handling brittle** → Isolate a pure state machine with table-driven traces; keep network and UI effects outside it.
- **[Risk] A late asynchronous result mutates a replaced Tracking session** → Correlate every result by Tracking-session, epoch, and occurrence identity and discard stale results.
- **[Risk] External playlist edits race consume** → Perform one reload and at most one exact-entry deletion attempt; never retry automatically or silently fall back to whole-playlist replacement.
- **[Risk] An uncertain removal remains unresolved** → Show passive process-local status and defer interactive repair to a future change.
- **[Risk] Queue-panel state becomes noisy** → Keep normal tracking compact and expose reasons or unresolved-outcome counts only for exceptional states.
- **[Trade-off] Tracking is lost on exit** → Accept by design; this avoids durable recovery semantics and treats current Emby state as authoritative after restart.
- **[Trade-off] Generic-client queue boundaries remain unknowable** → Continue exposing transport controls without claiming actual client queue length or position beyond the reconciliation projection.

## Migration Plan

1. Introduce the pure reconciliation model without activating generic-session tracking.
2. Route attached-session sequence submission and item-changing commands through the centralized correlation seam.
3. Feed session observations into the tracker and expose compact queue-panel state and recovery actions.
4. Add bounded exact-entry playlist deletion and passive unresolved-outcome status, then enable remote consume effects.

Rollback removes the attached-session tracker and returns to current untracked remote behavior. Exact-entry playlist deletion is additive; the existing full-playlist save path remains for explicit user saves. No persisted-data migration is required.
