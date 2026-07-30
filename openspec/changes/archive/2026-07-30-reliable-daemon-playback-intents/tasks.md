## 1. Protocol Contract

- [x] 1.1 Define the guarded playback-intent envelope, request identity, per-connection generation, semantic Play/Stop/SetPaused/Next/Previous variants, lifecycle outcomes, and structured rejection reasons in the control protocol.
- [x] 1.2 Bump the strict control protocol version and update existing serialization, handshake, and mismatch fixtures for the coordinated client/daemon break.
- [x] 1.3 Add conversion boundaries so unguarded player commands retain their current route while guarded commands cannot fall back to unacknowledged `PlayerCmd` transmission.

## 2. Daemon Intent Coordination

- [x] 2.1 Add daemon-owned current-intent state keyed by controlling connection, request ID, generation, semantic target, and lifecycle phase.
- [x] 2.2 Implement daemon transition helpers for Accepted, Applied, Coalesced, Superseded, and Rejected outcomes, including reconnect and controller-disconnect invalidation.
- [x] 2.3 Move Play item resolution off the serialized daemon control loop and return completion events carrying their connection identity and generation.
- [x] 2.4 Generation-check every resolution completion before player or authoritative-state mutation so stale success and failure results are inert.
- [x] 2.5 Implement Play ordering: equivalent startup requests coalesce, different newer targets supersede, and settled same-item Play restarts normally.
- [x] 2.6 Implement Stop as an always-available intent that supersedes pending work and prevents its later completion from starting playback.
- [x] 2.7 Implement single-flight Next and Previous against confirmed queue/current-item state.
- [x] 2.8 Implement `SetPaused(bool)` coalescing and confirm its Applied outcome from authoritative paused state.
- [x] 2.9 Correlate player events with the current intent and emit Applied only after the requested target or state is concretely confirmed, excluding the early active flag.

## 3. Remote Client Lifecycle

- [x] 3.1 Add per-connection request ID and generation allocation to the remote player path and send guarded actions through the new intent envelope.
- [x] 3.2 Track pending guarded intents separately from confirmed playback state and reconcile each lifecycle outcome without optimistic queue/current-item replacement.
- [x] 3.3 Clear pending intent state on disconnection, protocol failure, rejection, or controller replacement while ignoring events from an earlier connection epoch.
- [x] 3.4 Derive `SetPaused(bool)` from the last confirmed state and keep repeated input on the same desired state until confirmation.

## 4. TUI Feedback and Input

- [x] 4.1 Present the action or target immediately when a guarded direct-daemon intent is dispatched, then refine or clear it from correlated lifecycle outcomes.
- [x] 4.2 Route keyboard and mouse Play, Stop, Pause/Resume, Next, and Previous through the same guarded action path.
- [x] 4.3 Remove the double-Space and double-Escape timing state and make the first available keypress dispatch immediately.
- [x] 4.4 Allow Stop while Play is pending even before confirmed active playback, without changing local or attached-Emby routing.

## 5. Verification

- [x] 5.1 Update existing daemon-control and remote-player test coverage for lifecycle correlation, latest-Play-wins, stale completion suppression, Stop invalidation, navigation single-flight, desired-state pause, and settled same-item restart without introducing a new unit-test subsystem.
- [x] 5.2 Exercise keyboard and mouse direct-daemon scenarios with delayed item resolution and player startup, confirming that the first input is visible and repeated commands follow the specified policy.
- [x] 5.3 Run formatting, the relevant existing test targets, and clippy for touched crates; record any unrelated pre-existing failures separately.
