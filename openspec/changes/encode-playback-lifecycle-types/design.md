# Design

## Context

Requirements are in `specs/`. Motivation is in `proposal.md` and issue #824.
This section records only what the code looks like today at the points the
design changes.

**Progress copies and gates (invariant 06).**

| Copy | Written by | Gate |
|---|---|---|
| Daemon canonical queue (`PlayerOwnerState.queue`) | `mbv-daemon/src/run.rs` `apply_track_completed_observation`, `apply_stopped_observation` | hand-written x2 |
| Shell `PlaybackQueue` | `src/app/dispatch/session/player_event.rs` `apply_stopped_slot_progress` (~l.337) and TrackCompleted arm (~l.437) | hand-written x2 |
| Run `ExecutionSequence` | `mbv-player/src/run/events/queue_advance.rs:118` | **none** (raw `completed_pos`) |

The run's copy is read by `step_to_index` (`run/queue_commands.rs:112`) for
relative Next/Previous. In active-file mode it is also read by
`select_active_slot` → `prepare_item` → `resume_start_pos(item)`, because
`cmd_jump_to_active_file` ignores the command's `resume_ticks`.

**Relative navigation senders today.**

```
keyboard (Bare)      playback_target/local.rs:43 -> PlayerProxy::next -> Local -> PlayerCommand::Next -> run
Emby ws (Bare)       session/ws_event.rs:27      -> PlayerProxy::next -> (same)
MPRIS (after local fall-through) switch.rs:292   -> PlayerProxy::command_sender -> run
keyboard/MPRIS (attached)  proxy.rs Remote       -> PlaybackIntent Next -> daemon step_to_neighbor_slot   (keyboard)
                           RemotePlayer::send_command(Next) -> refused, dropped                           (MPRIS)
Emby ws (daemon)     mbv-daemon/src/ws.rs:160    -> Player::next -> run
tray (local_daemon)  src/local_daemon.rs:221     -> DaemonPlayerHandle.command_tx -> run
```

There are two neighbour rules. The daemon's `step_to_neighbor_slot`
(`control/playback.rs:184`) steps from the queued target, else the in-flight
target, else the observed slot, else the queue's active slot. The run's
`relative_step_base` (`run/queue.rs:33`) steps from the active slot, else
`current_idx`.

**Lifecycle bookkeeping (invariant 03).** `stop_report` and `load_state` are
assigned at 14 sites across `run/queue.rs`, `run/commands/load.rs`,
`run/events/{queue_advance,restart,termination}.rs`. `begin_item_lifecycle`
(`run/queue.rs:431`) resets everything except that pair. `LoadState::Pending`
holds a `NonZeroU8` that is only ever 1. `forced_slot_id`,
`forced_transition`, `forced_resume_ticks`, and `forced_jump_from_idle` are
cleared together at about 20 sites.

**Observed active slot.** `PlayerOwnerState.observed_active_slot` is the
authority. `SharedQueueState.observed_active_slot` (a mutex) is a copy for
other threads, read by `core_ctrl_spawn` and `control_queue` on threads that
have no owner access. It is republished by hand at `run.rs:83`,
`event_loop/player_events.rs:57`, `control/queue_setup.rs:306`, and
`control/queue_load.rs:39`. `step_to_neighbor_slot` reads the mutex even
though it holds `ctx.owner`.

## Goals / Non-Goals

**Goals:**
- Exactly one place decides each of: the recorded position per observation
  kind, the neighbour slot for a step, the resume position for a jump, and the
  per-item stop-report and drain state.
- The type system rules out a transport control addressing the Playback run's
  navigation.

**Non-Goals:**
- mpv-native playlist navigation (the user pressing `>` in the mpv window).
  The run still adopts it via `adopt_mpv_entry`, and it still resumes from
  mpv's baked `start=`. That limitation stays documented in invariant 06.
- The narrow race where a jump arrives before the owner applies the target's
  preceding `TrackCompleted`. No speculative patch without instrumentation.
- Ctrl wire protocol. `PlayerCommand::Next`/`Previous` never had a wire form,
  and `PlaybackIntentAction::Next`/`Previous` stay as they are.
- Behaviour of Stop, pause, seek, and volume from transport controls. They are
  forwarded unchanged.

## Decisions

### D1. `ProgressObservation` in `mbv-queue`

```rust
pub enum ProgressObservation { Completed { position_ticks: i64, played: bool },
                               Stopped   { position_ticks: i64, played: bool } }
impl ProgressObservation {
    pub fn position_to_record(&self, item: &QueueItem) -> i64;
    pub fn played(&self) -> bool;
}
```

It sits next to `QueueItem`, the only input it reads. The four call sites
become `obs.position_to_record(&slot.item)`. The 30 s floor
(`MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS`, now in `mbv-emby-model`) moves
into `mbv-queue` beside the gate if the gate is its only user once this lands.
Otherwise it stays and the gate imports it.

*Alternative:* one function per event kind (`completed_position`,
`stopped_position`). Rejected: the enum makes the choice between Completed and
Stopped explicit at the call site, where the bug the invariant warns about
(Stop mistakenly carrying the floor) would show up.

### D2. The run holds no progress

`ExecutionSequence::apply_progress` and the `queue_advance.rs:118` write are
deleted. The run's queue remains a slot/identity projection for mpv
coordinates only. Every resume the run applies arrives on a command:

- `JumpTo { resume_ticks }` for full-playlist and **active-file** jumps.
  `cmd_jump_to_active_file` passes `resume_ticks` into the prepared source and
  overrides `start_seconds` for non-Audiobookshelf items. Audiobookshelf items
  keep the session's `current_time_seconds`, because the service owns that
  resume point.
- A fresh submission (`SubmitQueue`/`LoadNew`) bakes `start=` from the items it
  carries, which the owner just took from the canonical queue.

### D3. `Direction` and `relative_step_target` on `PlayerOwnerState`

`Direction::{Next, Previous}` lives in `mbv-ctrl`, with
`From<&PlaybackIntentAction>` returning `Option<Direction>`.

```rust
pub enum StepTarget { Jump(QueueSlotId), Coalesced, AtEdge }
impl PlayerOwnerState {
    pub fn relative_step_target(&self, dir: Direction) -> StepTarget;
}
```

The **base** is the latest requested target (queued, else in-flight, else
`observed_active_slot`, else the queue's active slot). This keeps the daemon's
current rule, because it is the one that makes "Next then Previous" return to
where the user was. The run's active-slot-only rule is dropped.

**Coalescing.** A step whose direction equals the direction of the latest
*unsettled* relative transition returns `Coalesced`. The direction is carried
on the transition itself, so no second piece of state has to be kept in step:
`Transition` gains `cause: TransitionCause::{Jump, Step(Direction)}`. Settling,
superseding, or rejecting the transition ends its coalescing window.

The ctrl intent layer (`owner.intents.accept`) still coalesces equivalent ctrl
intents first and replies with the lifecycle events. When the owner returns
`Coalesced` for a ctrl-origin step that the intent layer let through, the
daemon replies `PlaybackIntentOutcome::Coalesced` to that client through its
`JumpOrigin` (D5).

*Alternative:* the base is always the observed slot, so a repeated Next targets
the same slot and falls out as a no-op. Rejected: it changes "Next then
Previous" to move back one slot past the start point.

### D4. `TransportCommand` and the owner inbound path

```rust
// mbv-ctrl
pub enum TransportCommand { Step(Direction), Player(PlayerCommand) }
```

`PlayerCommand` loses `Next` and `Previous`, so `Player(..)` can never carry a
step. Every `TransportCommand` goes to the owner, which resolves `Step` through
D3 and forwards `Player(..)` to the run unchanged.

| Sender | Mode | Route |
|---|---|---|
| Emby ws | daemon | `ws.rs` `handle_ws_control` gets owner access. `NextTrack`/`PreviousTrack` → `step(Direction, JumpOrigin::Transport)` |
| Tray | local_daemon | `DaemonPlayerHandle.command_tx: Sender<PlayerCommand>` becomes `transport_tx: Sender<DaemonEvent>`-backed `TransportSender`. New `DaemonEvent::Transport(TransportCommand)` handled in the event loop |
| MPRIS | attached | `mpris::start`/`rebind` take `Fn(TransportCommand)`. The remote closure maps `Step(d)` → `send_playback_intent(new_playback_intent(d.into()))` and `Player(c)` → `send_command(c)`. **Fixes the dropped Next** |
| MPRIS | local fall-through | `PlayerProxy::command_sender()` becomes `transport_sender()`. The Local branch sends into an App-owned `transport_rx` drained on the shell tick next to `ws_rx`, then `Step` → `request_relative_step` |
| Keyboard, Emby ws | Bare | Already on the shell thread. `jump_track` and `ws_event.rs` call `request_relative_step(Direction)` directly when local. The Remote branch keeps sending the playback intent |

`PlayerProxy::next`/`previous` lose their Local branch. The methods become
remote-only intent senders, or the callers match on `is_remote()`: whichever
leaves fewer call sites. `Player::next`/`previous` in `controller.rs` are
deleted.

Bare `request_relative_step(dir)`: `sync_canonical_queue`, then
`bare_owner.relative_step_target(dir)`, then on `Jump(slot)` it runs the
existing `request_slot_jump` path with `TransitionCause::Step(dir)`.

### D5. `JumpOrigin`

`dispatch_slot_jump(ctx, client_id, transition)` becomes
`dispatch_slot_jump(ctx, origin: JumpOrigin, transition)` with
`JumpOrigin::{Ctrl(CtrlClientId), Transport}`. Rejection replies go to the
ctrl client for `Ctrl` and are logged for `Transport`. Transport steps get
their request identity from an owner-scoped allocator (the same one Bare's
`mint_local_transition` uses), so stale-observation protection is identical
for every sender.

### D6. Item lifecycle types (invariant 03)

```rust
pub(crate) enum StopAction {
    ReportedNow(StopReport /* from mark_sent(report_stopped(..)) */), // replace paths
    Deferred,        // cmd_load_new: transition_to reports in the background
    NothingPlaying,  // first-ever submission
}
pub(crate) enum LoadState { Ready, DrainingReplacedFile }
pub(crate) struct ForcedJump { slot_id: QueueSlotId, transition: Option<Transition>,
                               resume_ticks: Option<i64>, from_idle: bool }
```

- `begin_item_lifecycle(StopAction)` is the only writer of `stop_report` and
  `load_state` at item boundaries. It sets the pair from the action and arms
  the drain. The fields become private to `run/state.rs` behind transition
  methods: `on_drained()`, `mark_reported(..)`, `accept_replacement()`, and
  `is_unreported()`. No `cmd_*` or event handler names the fields.
- `LoadState::Pending(NonZeroU8)` → `DrainingReplacedFile`. `drain()` returns
  `HitZero` or `AlreadyReady`; `StillPending` is deleted.
- `forced_*` → `forced_jump: Option<ForcedJump>`. Each lockstep clear becomes
  `self.forced_jump = None` or `.take()`. `on_end_file`'s settle takes the
  whole value at once.
- The sites that set `stop_report` mid-item (`events/queue_advance.rs:328,390`,
  `events/termination.rs:53`, `events/restart.rs:33`, `run/queue.rs:199`) go
  through the same private transition methods. The shutdown-aware reporter
  split (invariant 03, failure 3) is out of scope beyond routing it through
  those methods.

### D7. One publish of the observed active slot

`SharedQueueState::publish_observed(&PlayerOwnerState)` replaces the four hand
writes. `relative_step_target` reads the owner field only. The mutex stays as
a read-only snapshot for the ctrl-spawn and broadcast threads.

*Alternative:* delete the mutex and pass the value through the event loop.
Rejected: the ctrl-spawn thread builds a new client's initial state without
access to the event loop.

## Risks / Trade-offs

- **Bare Next now coalesces and resolves from the requested target.** Today
  Bare steps from the active slot. → This is the intended unification (spec:
  "Previous right after Next"). The Bare test uses the owner-level API.
- **Active-file resume source changes for mixed queues.** Non-Audiobookshelf
  items in an active-file queue now resume from the canonical queue instead of
  the run's copy. → Covered by the spec scenario and a run-level test with a
  `resume_ticks` override. Audiobookshelf is explicitly unchanged.
- **Removing `PlayerCommand::Next` touches ~15 files at once.** → Task order
  adds the new route first, moves each sender, and removes the variant last,
  so the compiler lists any remaining sender.
- **Tray/MPRIS Stop and pause still go straight to the run** (via
  `TransportCommand::Player`). → Out of scope; unchanged behaviour.
- **Daemon-minted transport ids share a number space with ctrl-client request
  ids.** `settle` matches request id *and* slot against the single in-flight
  transition, so a false settle needs a stale observation with the same id and
  the same slot. → Accepted. If `PlaybackRequestId` turns out to be compared
  anywhere without the slot, mint transport ids from a reserved range
  (checked in task 4.1).
- **mpv-native navigation still resumes from the baked `start=`.** →
  Documented in the shrunk invariant 06, not fixed here.

## Migration Plan

This is an internal refactor with no persisted-state or wire changes. It ships
as one change on `main`. Rollback is a revert.

## Test contracts

Per `docs/invariants/14-test-ownership.md`; each test names its contract.

| Contract | Layer | Test |
|---|---|---|
| Recorded position per observation kind | `mbv-queue` | `ProgressObservation::position_to_record` `#[case]` table: completed under/over floor, audio, played; stopped positive/zero/audio/played |
| Same slot resumes the same via step or jump (#824 regression) | `mbv-player` owner | `relative_step_target` + dispatched transition carries the canonical resume for an audio slot and a video completed under 30 s |
| Neighbour rule: base, edges, coalescing | `mbv-player` owner | `relative_step_target` cases: Next after Next (Coalesced), Previous after unsettled Next (returns to start), Next at last (AtEdge) |
| Emby ws Next under Stay-alive goes through the owner (#824 regression) | `mbv-daemon` | ws `NextTrack` produces a `JumpTo` with owner-resolved slot and resume, not a run-level step |
| MPRIS Next attached is delivered | TUI session | The remote transport closure maps `Step(Next)` to a playback intent (replaces the refused `send_command`) |
| Active-file jump honours carried resume | `mbv-player` run | Non-Audiobookshelf slot in an active-file queue loads with `start=` from `resume_ticks` |
| Lifecycle transitions | `mbv-player` run | Existing `player_tests_session*` drain/shutdown suites keep passing against the new methods. No new test unless a transition has no current coverage |

Tests that assert on removed APIs (`ExecutionSequence::apply_progress`,
`step_to_index`, the `ws_event.rs` `PlayerCommand::Next` case) are deleted,
not ported.
