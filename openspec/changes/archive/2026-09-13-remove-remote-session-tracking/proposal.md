## Why

Generic Emby Sessions expose the currently playing media but not their queue, queue position, or occurrence identity. mbv's process-local Tracking feature tries to reconstruct that missing authority and then applies inferred status and consume effects to mbv's queue; the feature has not worked reliably and adds queue mutation, command-correlation, lifecycle, and UI complexity to an already latency-prone queue path.

## What Changes

- **BREAKING** Remove sequence Tracking for playback submitted to an attached generic Emby Session, including startup, health states, expected transitions, occurrence candidates, completion inference, suspension, and re-anchoring.
- Stop inferred remote completion from consuming or otherwise editing mbv queue slots.
- Stop refreshing a previously observed Session item from Emby and replacing the first matching local queue slot to import watched/progress state.
- Remove Tracking-specific queue presentation, Stop Tracking and re-anchor actions, popup state, and first-edit confirmation.
- Preserve Session watch: attachment, current title and position display, polling, disappearance handling, and ordinary supported remote transport commands.
- Preserve Direct remote control of another mbv Player owner and its authoritative Bound queue behavior.
- Leave local and Player-owner progress, consume, persistence, and queue synchronization behavior unchanged; local queue latency is separate work.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `remote-playback-reconciliation`: Remove the capability in full; attached generic Emby Sessions become observation and transport targets only and no longer project inferred sequence state into mbv's queue.

## Impact

- Deletes the reconciliation model and tests from `mbv-core`.
- Simplifies attached-session submission, polling, command events, queue mutation, lifecycle, and application state.
- Removes the Tracking and re-anchor Interactive Component and queue-title presentation.
- Removes the current remote watched/progress refresh path from Session polling.
- Updates the remote-session domain model and removes the `remote-playback-reconciliation` specification when the change is archived.
- Adds no dependency or persisted-data migration because Tracking is already process-local.
