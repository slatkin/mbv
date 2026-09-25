# Design

## Context

See proposal.md - Why. `PlaybackRunIdentity` and the two `run_identity` fields
it backs (`PlayerEvent::Stopped`, `PlayerEvent::TrackCompleted`) are a
`(PlaybackRequestId, PlaybackGeneration)` pair where `PlaybackRequestId` is
always `0`. `PlaybackRequestId`/`PlaybackGeneration` remain meaningfully used
elsewhere (e.g. `TrackChanged::transition`, guarded playback intents in
`daemon-playback-intents`), so the type aliases themselves are not removed —
only this one always-zero field.

`CtrlCompatibility::for_peer` (`crates/mbv-core/src/ctrl.rs`) already rejects
any peer whose `protocol_version` does not exactly equal
`CTRL_PROTOCOL_VERSION` before any command flows. Note: `openspec/specs/
ctrl-protocol/spec.md` currently reads "Protocol version 9" while the code's
`CTRL_PROTOCOL_VERSION` is already `10` — a pre-existing, unrelated drift this
change does not attempt to fix. This change's delta targets the code's actual
current value (`10`) and bumps to `11`.

## Goals / Non-Goals

**Goals:**
- Replace the `(PlaybackRequestId, PlaybackGeneration)` shape with a plain
  `PlaybackGeneration` everywhere it backs `run_identity`/`stopped_run`.
- Keep `playback_run_identity_is_current` and the pending-idle-load
  match/cancel logic behaviorally identical (same generation comparisons,
  minus the dead request-id slot).
- Bump `CTRL_PROTOCOL_VERSION` so mismatched daemon/client builds fail at
  handshake, not at event deserialization.

**Non-Goals:**
- Fixing the pre-existing `ctrl-protocol` spec drift (documented "9" vs.
  actual code "10").
- Touching `PlaybackRequestId`/`PlaybackGeneration` usages that are not this
  always-zero slot (e.g. `TrackChanged::transition`, guarded intents).
- Any change to observable playback behavior — this is a type-shape
  simplification only.

## Decisions

- **Collapse to `PlaybackGeneration`, not a new newtype.** `PlaybackGeneration`
  is already `u64` and already carries the comparison semantics
  (`sequence_generation`). Introducing a wrapper newtype would add ceremony
  for a value that's compared for equality in exactly two call paths
  (`playback_run_identity_is_current`, `PendingIdleQueueLoad` matching).
  Alternative considered: keep `PlaybackRunIdentity` as a single-field struct
  — rejected because every current use is a bare equality/copy, so the struct
  wrapper adds no behavior over the plain `u64` it already logically is.
- **Bump `CTRL_PROTOCOL_VERSION` rather than relying on `#[serde(default)]`
  compat.** The existing `#[serde(default)]` on `run_identity` was there to
  let a pre-change peer's *index-shaped* `Stopped` decode with the field
  defaulted. A tuple-to-scalar shape change is not index-compatible: a v10
  peer sending `(0, 5)` would not deserialize as a bare `5`. Since
  `CtrlCompatibility::for_peer` already enforces exact version equality, a
  bump is the existing, idiomatic way this codebase handles a non-additive
  wire change (see `ctrl-protocol` Requirement: Protocol version, which
  documents the same pattern for the v9 bump).

## Risks / Trade-offs

- [Any daemon/client pair built across this change's boundary fails to
  connect] → Already true for every `CTRL_PROTOCOL_VERSION` bump; the daemon
  and TUI binary are built and shipped together in this project, so this is
  the existing, accepted deployment model, not a new risk.
- [Missing a construction/comparison call site during the mechanical sweep]
  → The compiler forces every site: changing the field type in
  `player/types.rs` and `daemon/core.rs` breaks the build at every
  `(0, ...)` tuple literal and every `.into()` call until they're updated.
