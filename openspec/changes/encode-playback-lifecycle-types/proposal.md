# Proposal

## Why

Invariants 03 (exactly-once stop report) and 06 (progress applied at every queue
copy) are upheld only by hand-coordinated call sites, and the coordination has
already failed. The Playback run keeps its own queue copy (`ExecutionSequence`)
and writes raw completion positions into it without the canonical 30 s floor or
the audio keep-previous rule. Relative Next/Previous then resumes from that
copy, while a jump resumes from the canonical queue, so the same slot resumes
differently depending on how the user reaches it. Four external senders (Emby
websocket in the daemon, the tray, MPRIS, and Bare `PlayerProxy::next`) address
the run directly and skip the Player owner. MPRIS Next/Previous under Stay-alive
is refused by the ctrl transport and does nothing. Issue #824; blocker #814
(crate split) has landed, so the code homes are now final.

## What Changes

- **One progress gate.** A `ProgressObservation::{Completed, Stopped}` type next
  to `QueueItem` in `mbv-queue` decides the position to record. It replaces the
  four hand-written gates in `mbv-daemon/src/run.rs` and
  `src/app/dispatch/session/player_event.rs`. Completion keeps the 30 s floor
  and audio keep-previous rule; Stop keeps "any positive position".
- **The Player owner resolves relative navigation.** Next/Previous become
  owner-issued slot jumps that carry canonical resume, in Bare and Stay-alive
  alike. One neighbour rule, `PlayerOwnerState::relative_step_target(Direction)`,
  serves the daemon and Bare. `Direction` replaces matching on
  `PlaybackIntentAction` with a `_ => next` fallthrough.
- **BREAKING (internal vocabulary):** `PlayerCommand::Next` and
  `PlayerCommand::Previous` are removed. `step_to_index`,
  `ExecutionSequence::apply_progress`, and the run's progress copy are
  removed, along with the design D4 exception ("relative nav has no transition").
- **Transport vocabulary split.** A new `TransportCommand` is what external
  controls send (MPRIS, tray, Emby websocket). It goes to the owner's inbound
  path. MPRIS and the tray no longer hold a `PlayerCommand` sender.
- **Fix: MPRIS Next/Previous under Stay-alive**, which is currently dropped as
  "no ctrl wire form".
- **Jumps carry their origin.** `dispatch_slot_jump` takes a `JumpOrigin`
  (ctrl client or transport control) instead of requiring a `CtrlClientId`.
- **Active-file jumps honour carried resume** for non-Audiobookshelf items
  instead of reading the run's copy. Audiobookshelf items keep the resume point
  from the service's playback session.
- **Item lifecycle as a type (invariant 03).** `begin_item_lifecycle(StopAction)`
  owns `stop_report` and `load_state`, which become private to transition
  methods. `LoadState` collapses to a two-state type. The four `forced_*`
  fields become one `Option<ForcedJump>`.
- **One published active slot.** `relative_step_target` reads the owner's
  `observed_active_slot`. The daemon's shared mutex becomes a snapshot that one
  `SharedQueueState::publish` method writes.
- **Docs.** Delete `docs/invariants/03-exactly-once-stop-report.md`. Shrink
  `06` to what types cannot enforce: mpv's one-shot `start=` and the
  jump-before-`TrackCompleted` race.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: adds requirements that relative navigation is
  resolved by the Player owner for every sender, that recorded progress follows
  one rule per observation kind, and that a slot resumes the same however it is
  reached.
- `daemon-playback-intents`: "Navigation is single-flight" extends to Next and
  Previous from non-ctrl senders (Emby websocket, tray, MPRIS) under the daemon
  owner.

## Impact

- `crates/mbv-queue` (new `ProgressObservation`; `ExecutionSequence` loses
  `apply_progress`).
- `crates/mbv-ctrl` (`PlayerCommand` loses Next/Previous; new
  `TransportCommand`, `Direction`).
- `crates/mbv-player` (`owner_state.rs` neighbour rule; `run/` lifecycle types,
  `ForcedJump`, `StopAction`, `LoadState`; removal of `step_to_index`;
  `controller.rs` and `proxy.rs` lose `next`/`previous` on the Local branch).
- `crates/mbv-daemon` (`ws.rs`, `run.rs`, `control/playback.rs`, `core.rs`
  `DaemonEvent`, `DaemonPlayerHandle`, `SharedQueueState`).
- `crates/mbv-desktop` (MPRIS and tray send `TransportCommand`).
- TUI: `src/local_daemon.rs`, `src/app/dispatch/action.rs`,
  `src/app/dispatch/session/{player_event,ws_event,switch,connect,daemon_restart}.rs`,
  `src/app/state/playback_target/local.rs`.
- `docs/invariants/03-*.md` (deleted), `docs/invariants/06-*.md` (shrunk).
- No wire-protocol change: `PlayerCommand::Next`/`Previous` already had no
  wire form, and ctrl intents are unchanged.
