## Context

The global visualiser binding is registered in the input context stack and handled by `App::handle_key_visualizer`. An attached Sessions-panel connection is represented by `connected_session_id`. The current handler checks only daemon spectrum support before calling `toggle_visualizer`, so a connected remote session can still mutate `visualizer_enabled` and invoke visualiser lifecycle code.

## Goals / Non-Goals

**Goals:**

- Make an unmodified `v` press have no effect while `connected_session_id` is present.
- Prevent visualiser toggling, preference writes, worker lifecycle changes, and spectrum control commands in that mode.
- Keep the key consumed, matching the existing global binding behavior.
- Preserve current behavior when no attached remote session exists.

**Non-Goals:**

- No changes to remote session connection or disconnection behavior.
- No changes to direct remote daemon visualisation where `connected_session_id` is absent.
- No changes to visualiser rendering, CAVA supervision, or spectrum protocol handling.

## Decisions

### 1. Guard in the existing visualiser key handler

Add an early `connected_session_id.is_some()` check in `handle_key_visualizer` after confirming the `v` chord and before unsupported-daemon checks or `toggle_visualizer()`. Return `Some(false)` so the key is consumed without changing state.

This keeps the mode-specific rule at the action boundary and avoids duplicating the guard in the context stack or lower-level visualiser lifecycle methods.

### 2. Use session presence as the mode predicate

Use the existing `connected_session_id` field because it is the established marker for an attached Sessions-panel remote session and is already used by input snapshots and playback routing. Do not infer remote mode from `PlayerProxy::is_remote()`, which also covers supported direct daemon connections.

### 3. Verify side-effect absence

Tests should set `connected_session_id`, initialize visualiser state, invoke the handler with `v`, and assert that the enabled flag remains unchanged and no remote control command is emitted. A local-session test should retain coverage that the same key still toggles normally.

## Risks / Trade-offs

- A stale session id could suppress the key until session cleanup completes. This is consistent with the existing session-state predicate and should be covered by the session teardown path separately if needed.
- Consuming `v` means it will not be handled by lower-priority contexts while attached, which is intentional for a global binding that is explicitly a no-op in this mode.
