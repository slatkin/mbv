# Design

## Context

PR #1072 (`switch-output-without-hand-off`) added `SelectionIntent` (`Continue`, `SwitchOnly`) in
`crates/mbv-daemon/src/output/handoff.rs`. Every swap, in `cast/connect.rs`,
`emby_session/connect.rs`, and `hand_remote_output_back_to_local`, calls
`stop_old_output_for_handoff`, which calls `stop_active_output` with `StopScope::LoadedItemOnly`
and tears down an old receiver app. That change's design D1 chose "the old output stops" because
an Owner has one output. This change reverses D1 for remote old outputs. See proposal.md - Why.

## Goals / Non-Goals

**Goals:** a `SwitchOnly` swap sends the old Cast receiver or Emby Session device nothing that
stops it; the Owner stops tracking it; the rest of the swap is unchanged.

**Non-Goals:** a Client change, a protocol change, a way to reattach a released device, and
leaving the local run playing.

## Decisions

**D1. A third stop mode, `Release`, selected by the intent.** `StopScope` (`cast.rs`) gains
`Release`. The old-output stop takes the intent: `Continue` uses `LoadedItemOnly` as today,
`SwitchOnly` uses `Release`. `Release` sends no device stop and queues no receiver app teardown.
*Alternative:* skip `stop_old_output_for_handoff` for a switch. Rejected: the function also
forgets the loaded item and re-presents the output, which a release needs.

**D2. Release keeps the Owner-side bookkeeping.** The Cast arm still sends the stop report for the
presented position and emits the Feed `Stopped` (design D9 of the hand-off work): the Owner stops
tracking the item, so Emby and Feed state must not show it as playing at a stale position. It
forgets the loaded item, so a later receiver status for the released device can neither settle
nor advance anything. The Emby Session arm forgets the loaded item and sends no device stop.
*Alternative:* send no report either. Rejected: Emby would show a playback that nothing updates.

**D3. The receiver connection is dropped without a stop.** The old Cast output's connection worker
is dropped when the output is replaced. The drop must send nothing that stops media. A task
checks the worker's shutdown path and removes any stop it sends.

**D4. Local is the exception.** The Owner hosts the local run and feeds its status into the
queue authority, so a local run that plays under a remote output would break "one output". A
switch away from Local keeps `player.stop()`. The user can reverse this later as its own change.

**D5. Continue is unchanged.** A hand-off moves the slot, so the old output stops by definition.

**D6. Ordering.** This change edits code that exists only in PR #1072. Implement on top of that
branch or after it merges. Its delta sync follows #1072's delta sync.

## Risks / Trade-offs

- [A receiver app may close when its last sender leaves] → The Owner cannot control it. The
  requirement says the Owner sends no stop; the device's own behavior is outside the Owner.
- [Two devices play at once] → That is the stated intent of the user. The Owner drives only the
  target.
- [A released device later reports status] → The loaded item is forgotten, so the report is
  ignored (scenario "A released device does not move the queue").

## Migration Plan

No data or protocol migration. Rollback restores `LoadedItemOnly` for `SwitchOnly`.
