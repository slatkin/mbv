# Proposal

## Why

Follow-ups from the Emby Session owner-output change (#945), filed as issue
#949:

- The ctrl command `UnifiedQueueLoadIdle` lost its last sender when the
  attached-session play path was deleted. It now exists only as dead wire
  vocabulary plus a dedicated daemon state machine (`pending_idle_load`).
- Live testing showed the device's own next-episode autoplay at queue end
  presents as foreign content with no advance; adopting it as the next slot
  was explicitly deferred.
- Live testing also attached a packaged Owner to a dev Client of the same
  version: `PeerBuild::MustMatch` compares only `CARGO_PKG_VERSION`, so Enter
  on a Session was a silent no-op. The build gate must compare a build
  identity, not only the version.

The fourth issue item (Brave Linux stall) was not reproduced and is out of
scope; it can be reopened if it comes back.

## What Changes

- Remove the `UnifiedQueueLoadIdle` ctrl command and the whole
  populate-only idle-load path it alone served: the command variant, the
  `UnifiedQueueLoadResult` event and `QueueLoadResult`/`QueueLoadRequestId`
  vocabulary, the `OwnerGateRejection::QueueLoadIdle` arm,
  `RemotePlayer::load_queue_idle`, the daemon's `pending_idle_load` pending
  machinery (`handle_queue_load_idle`, `install_idle_queue_load`,
  `complete_pending_idle_queue_load`, cancel/expire paths), the relay in
  `connect.rs`, and the dead `UnifiedQueueLoadResult` handling in the TUI.
  `UnifiedQueueReplace` is the surviving whole-queue replacement and always
  starts playback; a populate-only load into an owner ceases to exist.
  **BREAKING** for a stale same-version peer that still sends the command
  (none does: the last sender was deleted in #945). `CTRL_PROTOCOL_VERSION`
  stays 10 — no bump, per user decision.
- Adopt device autoplay at queue end for Emby Sessions: when the device
  begins an item the owner did not load after the queue's last playable
  slot, the owner appends that item to the queue as a new slot, makes it the
  active slot, and keeps tracking — repeating for each later autoplay.
  Mid-queue foreign content keeps the present-only rule and the D14
  stale-item correction is unchanged.
- Strengthen the local Owner build gate: `CtrlHello` gains an additive
  `build_id` (the build's git identity, embedded at compile time), and
  `PeerBuild::MustMatch` refuses when version **or** build identity differ —
  before any credential leaves the Client (invariant 15). The mismatch
  prompt shows both build identities.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `unified-playback-queue`: the populate-only/idle-load requirements and
  scenarios describe `UnifiedQueueLoadIdle`'s contract; they are removed or
  re-anchored to answered `UnifiedQueueReplace` semantics, and the
  answered-edits requirement loses its populate-only carve-outs.
- `queue-canonical-list`: the "consecutive idle loads repaint" scenario
  describes the removed path; re-anchor it to queue replacements.
- `owner-playback-output`: "content the owner did not load is presented, not
  advanced" gains the queue-end autoplay-adoption carve-out, with adoption
  scenarios.
- `local-daemon-single-instance`: "a local Client never attaches to an Owner
  from a different build" compares build identity in addition to
  `app_version`; the mismatch prompt names the identities.
- `application-logging`: the "Correlation fields" requirement's ctrl queue-load
  bullet goes with the removed operation, and the "Grouping by event" example moves
  off the retired `queue.load.rejected` to a live event name.

## Impact

- `crates/mbv-ctrl`: `commands.rs`, `events.rs`, `player.rs`, `hello.rs`
  (new `build_id` field, serde-defaulted), `lib.rs` exports, ctrl tests.
- `crates/mbv-remote-player`: `lib.rs` (`load_queue_idle`), `connect.rs`
  (result relay), `connect/handshake.rs` (build comparison), connect tests.
- `crates/mbv-daemon`: `control.rs` dispatch, `control/queue_load.rs` (whole
  pending-idle-load module), `core.rs` `pending_idle_load` field,
  `event_loop/player_events.rs` and `control_events.rs` pending-load arms,
  and the `queue_idle_load` test family plus touched persistence/source
  tests.
- `src/`: `app/dispatch/session/player_event.rs` dead result handling;
  `owner_restart.rs` prompt text if it names versions only.
- Build plumbing: a build-identity source (e.g. `build.rs` emitting a git
  SHA env) feeding `CtrlHello` in `mbv-ctrl`.
- Specs: deltas in the five capabilities above; `ctrl-protocol` needs none —
  no requirement names the removed command and the hello change is additive.
- Issue #949 items 1–3; item 4 (Brave stall) is explicitly deferred.
