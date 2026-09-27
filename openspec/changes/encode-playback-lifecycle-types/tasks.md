# Tasks

Each numbered group is sized for one implementer session. Groups run in order.
Gate for every group: `cargo check --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo nextest run -p <crates touched>` all pass. Run `cargo fmt` after edits.

## 1. One progress gate (`ProgressObservation`)

- [x] 1.1 Add `ProgressObservation::{Completed, Stopped}` with `position_to_record(&QueueItem)` and `played()` to `crates/mbv-queue` (design D1), with a named `#[case]` table covering: completed below and at the 30 s floor, completed audio, completed played, stopped positive, stopped zero, stopped audio, stopped played. Verify: the new `mbv-queue` tests pass.
- [x] 1.2 Replace the two hand-written gates in `crates/mbv-daemon/src/run.rs` (`apply_track_completed_observation`, `apply_stopped_observation`) with `ProgressObservation`. Verify: `cargo nextest run -p mbv-daemon` passes.
- [x] 1.3 Replace the two gates in `src/app/dispatch/session/player_event.rs` (`apply_stopped_slot_progress` and the `TrackCompleted` arm) with `ProgressObservation`. If `MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS` now has no user outside the gate, move it into `mbv-queue`. Verify: `cargo nextest run -p mbv` passes and `rg MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS` shows only the gate.

## 2. Direction and the shared neighbour rule

- [x] 2.1 Add `Direction::{Next, Previous}` to `crates/mbv-ctrl`, with a conversion from `PlaybackIntentAction` returning `Option<Direction>`, and add `TransitionCause::{Jump, Step(Direction)}` to `Transition` in `crates/mbv-player/src/transition.rs`. Existing constructors default to `Jump`. Verify: the workspace compiles and existing transition tests pass.
- [x] 2.2 Add `PlayerOwnerState::relative_step_target(Direction) -> StepTarget::{Jump, Coalesced, AtEdge}` in `crates/mbv-player/src/owner_state.rs`, using the base and coalescing rules in design D3. Owner tests cover: Next after unsettled Next → `Coalesced`; Previous after unsettled Next → the original slot; Next at the last slot → `AtEdge`. Verify: the new tests pass.
- [x] 2.3 Add the #824 divergence regression test at the owner layer: an audio slot left mid-track, and a video slot completed at 12 s with a 20 min recorded position, are each reached by `relative_step_target` + dispatch and by a direct jump. Assert the dispatched `JumpTo.resume_ticks` is identical. Verify: the test passes.
- [x] 2.4 Make `step_to_neighbor_slot` (`crates/mbv-daemon/src/control/playback.rs`) call `relative_step_target` and delete its inline neighbour code and the `_ => next` match. On `Coalesced`, reply `PlaybackIntentOutcome::Coalesced` to the ctrl client. Verify: `cargo nextest run -p mbv-daemon` passes, including the existing single-flight tests.

## 3. Observed active slot published once

- [ ] 3.1 Add `SharedQueueState::publish_observed(&PlayerOwnerState)` and replace the four hand writes (`run.rs:83`, `event_loop/player_events.rs:57`, `control/queue_setup.rs:306`, `control/queue_load.rs:39`). Verify: `rg "observed_active_slot.lock\(\).unwrap\(\) =" crates/mbv-daemon/src` finds only `publish_observed`, and the `mbv-daemon` tests pass.

## 4. Jump origin and daemon transport inbound

- [ ] 4.1 Change `dispatch_slot_jump` (`crates/mbv-daemon/src/core.rs`) to take `JumpOrigin::{Ctrl(CtrlClientId), Transport}` (design D5) and update every caller. Confirm that `PlaybackRequestId` is only ever compared together with the slot. If it is compared without the slot anywhere, mint transport ids from a reserved range. Verify: the `mbv-daemon` tests pass.
- [ ] 4.2 Add `TransportCommand::{Step(Direction), Player(PlayerCommand)}` to `crates/mbv-ctrl` and `DaemonEvent::Transport(TransportCommand)`. The event loop resolves `Step` through `relative_step_target` + `dispatch_slot_jump(JumpOrigin::Transport)` and forwards `Player(..)` to the run. Verify: a daemon test sends `Transport(Step(Next))` and observes a `JumpTo` to the next slot with canonical resume.
- [ ] 4.3 Route Emby websocket `NextTrack`/`PreviousTrack` in `crates/mbv-daemon/src/ws.rs` through the same owner step path (give `handle_ws_control` the owner context). Add the #824 bypass regression test: a websocket `NextTrack` under the daemon yields an owner-resolved `JumpTo`, not a run-level step. Verify: the test passes.
- [ ] 4.4 Replace `DaemonPlayerHandle.command_tx` with a transport sender into `DaemonEvent::Transport`, and update `src/local_daemon.rs` and `crates/mbv-desktop/src/tray.rs` so the tray sends `TransportCommand`. Verify: `cargo check -p mbv-desktop -p mbv` passes and `rg "command_tx" src/local_daemon.rs crates/mbv-desktop` is empty.

## 5. MPRIS and Bare senders

- [ ] 5.1 Change `mbv_desktop::mpris::{start, rebind}` to take `Fn(TransportCommand)`. Update the remote closures in `src/app/state/construct/remote.rs`, `src/app/dispatch/session/{switch,connect,daemon_restart}.rs` to map `Step(d)` to `send_playback_intent` and `Player(c)` to `send_command`. Add a test that the remote transport mapping turns `Step(Next)` into a playback intent. Verify: the test passes.
- [ ] 5.2 Add `App::request_relative_step(Direction)` next to `request_slot_jump` in `src/app/dispatch/action.rs` (design D4, Bare route). Route `jump_track` (`src/app/state/playback_target/local.rs`) and Bare websocket `NextTrack`/`PreviousTrack` (`src/app/dispatch/session/ws_event.rs`) to it when the player is local. Delete the `ws_event.rs` `PlayerCommand::Next` test case. Verify: `cargo nextest run -p mbv` passes.
- [ ] 5.3 Replace `PlayerProxy::command_sender` with `transport_sender`. The Local branch sends into a new App-owned `transport_rx`, drained on the shell tick next to `ws_rx`, and `Step` goes to `request_relative_step`. Update `rebind_mpris_to_current_player` in `switch.rs`. Verify: a shell test sends `Step(Next)` on the channel and observes a Bare slot jump.

## 6. Run loses relative navigation and its progress copy

- [ ] 6.1 Delete `PlayerCommand::Next`/`Previous` (`crates/mbv-ctrl/src/player.rs`), their arms in `crates/mbv-ctrl/src/commands.rs` and `crates/mbv-player/src/run/commands.rs`, `step_to_index` and `relative_step_base`, the Local branches of `PlayerProxy::next`/`previous` (`proxy.rs`), and `Player::next`/`previous` (`controller.rs`). Also delete the D4 comment about the relative-nav exception. Verify: the workspace compiles and `rg "PlayerCommand::(Next|Previous)\b"` is empty.
- [ ] 6.2 Delete `ExecutionSequence::apply_progress` and its tests (`crates/mbv-queue/src/execution_sequence.rs`), and the write at `run/events/queue_advance.rs:118` with its comment. Verify: the workspace compiles and `rg "fn apply_progress" crates/mbv-queue/src/execution_sequence.rs` is empty.
- [ ] 6.3 Make `cmd_jump_to_active_file` honour `resume_ticks` for non-Audiobookshelf items by overriding the prepared source's `start_seconds` (design D2). Audiobookshelf keeps its session position. Add a run-level test: a non-Audiobookshelf slot in an active-file queue loads with `start=` from `resume_ticks`. Verify: the test passes.

## 7. Item lifecycle types (invariant 03)

- [ ] 7.1 Collapse `LoadState` to `{Ready, DrainingReplacedFile}` and delete `StillPending` and the `NonZeroU8` (`crates/mbv-player/src/run/state.rs`). Verify: `cargo nextest run -p mbv-player` passes.
- [ ] 7.2 Replace `forced_slot_id`, `forced_transition`, `forced_resume_ticks`, and `forced_jump_from_idle` with `forced_jump: Option<ForcedJump>` (`run/types.rs`) and update every clear and read site. Verify: `rg "forced_(slot_id|transition|resume_ticks|jump_from_idle)" crates/mbv-player/src` is empty and the `mbv-player` tests pass.
- [ ] 7.3 Introduce `StopAction` and `begin_item_lifecycle(StopAction)`. Make `stop_report` and `load_state` private to `run/state.rs` behind transition methods (design D6), and convert all 14 assignment sites. Verify: `rg "stop_report = |load_state = " crates/mbv-player/src/run` matches only inside `state.rs`, and the `player_tests_session*` suites pass.

## 8. Docs and specs

- [ ] 8.1 Delete `docs/invariants/03-exactly-once-stop-report.md`. Shrink `docs/invariants/06-queue-progress-application-sites.md` to mpv's one-shot `start=` (including mpv-native navigation) and the jump-before-`TrackCompleted` race, and point to `ProgressObservation` and `relative_step_target`. Verify: `rg -l "invariants/03|03-exactly-once" docs openspec/specs AGENTS.md` is empty.
- [ ] 8.2 Update `CONTEXT.md` with `Transport command` and `Relative step` if they are not already defined. Verify: the terms are present and don't collide with *Avoid* entries.
- [ ] 8.3 Final gate: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, and `make check-code-file-lines`. Verify: all three pass.
