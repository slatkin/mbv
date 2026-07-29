## Context

The direct-daemon playback path currently sends unacknowledged `CtrlCmd` values through an unbounded client-side channel. `PlayItems` resolution runs synchronously on the daemon control loop, the client updates some playback state optimistically, and the daemon can broadcast queue state before the player has acted. A slow library lookup or player startup therefore leaves the TUI with no reliable indication of whether a command is pending, applied, rejected, or obsolete.

The existing 300 ms double-Space and double-Escape guards cover only their keyboard paths. Mouse playback controls and commands such as Play, Next, and Previous bypass them. Repeated commands can consequently queue against stale state and execute later.

The control protocol currently requires an exact version match. This change can therefore make a coordinated protocol break rather than adding ambiguous compatibility behavior.

## Goals / Non-Goals

**Goals:**

- Make the first direct-daemon playback input visible and actionable immediately.
- Correlate every guarded playback request with daemon lifecycle outcomes.
- Define deterministic ordering, coalescing, supersession, and Stop behavior.
- Keep the daemon control loop responsive while play targets are resolved.
- Preserve deliberate restart of an item whose startup has settled.
- Apply the same intent policy to keyboard and mouse dispatch.

**Non-Goals:**

- Changing local playback behavior.
- Changing transport behavior for an attached Emby session.
- Predicting when pipe output becomes audible; that is covered by `surface-pipe-playout-latency`.
- Adding a general distributed command bus or persisting requests across daemon restarts.
- Maintaining compatibility between the old and new strict control protocol versions.

## Decisions

### 1. Introduce a request-correlated playback-intent protocol

The next strict control protocol version will carry a playback intent envelope containing:

- a request ID unique within the control connection;
- a monotonically increasing generation within that connection; and
- a semantic playback intent.

Guarded intents comprise Play, Stop, SetPaused, Next, and Previous. Other controls remain outside this lifecycle unless implementation evidence shows they need the same ordering guarantees.

The daemon will return request-correlated lifecycle outcomes:

- `Accepted`: the request passed basic validation and became current work;
- `Applied`: the requested effect was confirmed from player state or a concrete player event;
- `Coalesced`: an equivalent request is already in flight;
- `Superseded`: newer or higher-priority work made the request obsolete; and
- `Rejected`: the request cannot be performed, with a structured reason suitable for presentation.

Request ID and generation are separate because correlation and ordering are different concerns. The connection identity forms the generation epoch, so a reconnect starts a fresh sequence and cannot revive work from the previous connection.

Alternatives considered:

- A time-only debounce cannot distinguish a deliberate command from stale work and does not report failure.
- A client-only request queue cannot cancel daemon-side metadata resolution or stale completions.
- A single sequence number could correlate events, but separating identity from ordering keeps coalesced and superseded outcomes unambiguous.

### 2. Make the daemon authoritative for intent ordering

The daemon will maintain one current guarded playback intent for the controlling connection. Only the daemon may decide that an intent is applied, coalesced, superseded, or rejected.

Policy:

- A newer different Play supersedes an unresolved or starting Play.
- An equivalent Play is coalesced while that target is unresolved or starting.
- Play for the current item is accepted as a restart after the previous startup has reached `Applied`.
- Stop supersedes every unresolved guarded intent, invalidates its completion, and remains available even before playback becomes active.
- Next and Previous are individually single-flight until the requested track change reaches a terminal outcome.
- Repeated SetPaused with the same desired state is coalesced until confirmed.

This policy intentionally uses lifecycle state rather than a fixed debounce interval.

### 3. Replace remote toggle-pause with desired paused state

The TUI will translate Pause/Resume input from the last confirmed daemon state into `SetPaused(true)` or `SetPaused(false)`. While a state change is pending, repeated input resolves to the same desired state and is coalesced. After confirmation, the next input selects the opposite state.

This prevents an invisible pair of toggle commands from canceling each other. It does mean a user must wait for confirmation before intentionally reversing a pause transition, which is preferable to executing against stale state.

### 4. Resolve play targets off the daemon control loop

Potentially slow item resolution will run outside the daemon's serialized control loop using existing concurrency facilities. Its completion will return to the control loop with the originating connection identity, request ID, and generation.

The control loop will mutate authoritative playback state and invoke the player only if the completion still matches the current generation. Stale success and failure completions are discarded after the superseded request has received its terminal outcome.

This is logical cancellation rather than an attempt to forcibly interrupt an in-flight HTTP request. It keeps the implementation small and avoids a new dependency.

### 5. Confirm outcomes from concrete state transitions

Sending a command to the player thread is not sufficient for `Applied`.

- Play becomes applied when the requested target is confirmed as the active player target by a concrete player event/state transition.
- SetPaused becomes applied when the confirmed paused state equals the requested state.
- Next or Previous becomes applied when the confirmed current item changes to the requested adjacent target.
- Stop becomes applied when playback is confirmed inactive.

The current early `status.active = true` assignment MUST NOT alone settle a Play intent. Authoritative queue/current-item broadcasts will describe confirmed playback; the TUI can separately present the pending target.

### 6. Present pending state immediately in the TUI

Dispatch will install a visible local pending presentation before sending the request. The presentation will identify the action or target and remain until a terminal lifecycle outcome or disconnection.

`Accepted` may refine the wording but is not required before the first visible response. `Applied` clears the pending state into normal playback presentation. `Coalesced` leaves the canonical request visible. `Superseded`, `Rejected`, and disconnection clear or replace the pending state with an explanatory status.

The input/action path will remove the double-Space and double-Escape timers. Keyboard and mouse actions will enter the same guarded dispatch path.

### 7. Fail closed on connection loss and protocol mismatch

Disconnecting clears all client pending state and prevents late events from a previous connection from matching new requests. The daemon invalidates unresolved work belonging to the disconnected controller.

The protocol version will be bumped as a strict coordinated break. A mismatched client and daemon will retain the existing explicit incompatibility behavior rather than attempting partial support.

## Risks / Trade-offs

- **[Risk] A stale worker completion starts obsolete playback** → Carry the connection identity and generation through every completion and re-check both on the daemon control loop before mutation.
- **[Risk] `Applied` is emitted from an early or unrelated player event** → Match the event against the requested target or desired state; do not use `status.active` alone.
- **[Risk] UI and daemon pending state diverge after disconnect** → Clear client state on connection loss and invalidate daemon work from the old connection epoch.
- **[Risk] Single-flight navigation slows intentional multi-track skipping** → Allow the next keypress immediately after the confirmed item change; favor safety during the ambiguous latency window.
- **[Risk] Desired-state pause feels less toggle-like during latency** → Keep the pending state visible so the user knows why another press is being coalesced.
- **[Risk] Coordinated protocol deployment temporarily breaks mixed versions** → Use the existing strict mismatch error and deploy matching client/daemon builds together.
- **[Trade-off] Logical cancellation does not stop an already-running lookup** → It prevents stale effects without adding cancellation infrastructure; obsolete lookup cost is bounded and observable.

## Migration Plan

1. Add the new protocol types and bump the strict control protocol version.
2. Add daemon intent lifecycle state and generation-checked asynchronous resolution.
3. Route guarded remote controls through the new protocol and lifecycle.
4. Add immediate pending presentation and remove optimistic authoritative-state replacement.
5. Remove the legacy double-key guards and route mouse controls through the same policy.
6. Update existing protocol fixtures and perform direct-daemon latency scenarios before release.

Rollback requires deploying the previous matching client and daemon together because the protocol versions are intentionally incompatible.

## Open Questions

None. Pipe-specific extension of the startup guard is intentionally deferred to `surface-pipe-playout-latency`.
