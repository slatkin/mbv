# Tasks

## 1. Cast client: status, connect, shared transport

- [x] 1.1 In `crates/mbv-cast/src/client.rs`, add `CastIdleReason` (Finished, Cancelled, Interrupted, Error) and `CastStatus.idle_reason`, read from `StatusEntry.idle_reason` in `cast_status_from_entry`. `LOADING` cannot reach `cast_playback_state`: `rust_cast` 0.21 `PlayerState::from_str` rejects it, so today `media.get_status()` fails with `Error::Internal("Unknown player state LOADING")` during every load (design D7). In `CastClient`, convert that one error from `get_status` and from `load` into a `Buffering` status for the loaded content id. On `load` it means the load was accepted. Every other error stays an error. Verify: one new status-mapping test next to `idle_payload` asserts that a finished idle entry yields `idle_reason == Some(Finished)` (contract: the idle reason crosses the crate boundary); one test that the `LOADING` parse error maps to `Buffering` and a different `Error::Internal` stays an error (contract: a load in progress is not a disconnect).
- [x] 1.2 Make `CastClient::connect` take a connect mode (first select or reconnect). Read `receiver.get_status()` first. Decide from the application list with a pure function: join the running `DefaultMediaReceiver` (connect to its `transport_id`, adopt `media_session_id` from `media.get_status`), launch when no app runs, refuse a reconnect when another app runs (design D11). Verify: one `#[case]` table test on the pure decision function with the three outcomes.
- [x] 1.3 Move `CastTransport`, `CastJob`, and `spawn_cast_worker` from `src/app/state/types/cast.rs` into the public API of `mbv-cast`. Move the fake transport behind a `test` feature (same pattern as `mbv-player`'s `test` feature). The TUI imports them from `mbv-cast`. Verify: `cargo check --workspace` and the existing TUI Cast tests pass unchanged.

## 2. Ctrl vocabulary

- [x] 2.1 In `crates/mbv-ctrl`, add the capability `playback-output`, `OutputSelection::{Local, Cast { receiver_id, name, host, port }}`, `OutputState::{Local, Cast { receiver_id, name, connection }}` with `CastConnection::{Connecting, Connected, Disconnected}`, `CtrlCmd::SelectPlaybackOutput`, and `CtrlEvent::PlaybackOutput` (design D3). No protocol version bump. Verify: one serde round-trip test for the command and one for the event in the `mbv-ctrl` tests binary.

## 3. Owner output model and hosting

- [x] 3.1 Add `PlaybackOutput { Local, Cast(CastOutput) }` to `DaemonPlayerOwner` (default `Local`), and a `CastOutput` struct that holds the worker sender, receiver identity, connection state, last status, and the loaded item (design D1). Add a Cast connector to `DaemonRuntimeHooks`. The Local daemon (`src/local_daemon.rs`) supplies it. `mbvd` (`crates/mbvd/src/main.rs`) passes `None`. The hello advertises `playback-output` only when the connector is present. Verify: daemon tests assert that the hello has the capability with a connector and does not have it without one.
- [x] 3.2 Handle `SelectPlaybackOutput`. Without a connector, a Cast selection gets `CommandRejected` with a reason. With a connector, connect on the Cast worker and re-enter through `DaemonEvent::Cast`. A connect failure keeps the current output and reports. On success with no active slot, swap the output. Broadcast `PlaybackOutput` on change and on cold seed, only to peers with the capability. Verify: tests with a fake connector: rejection without a connector; broadcast after a successful select; connect failure keeps `Local`.
- [x] 3.3 Add the `DaemonEvent::Cast` arm and Cast timers on the daemon tick: keep-alive 3s, status poll 7s, worker watchdog 20s (design D8). A poll error or a watchdog timeout sets `Disconnected`, broadcasts, and reconnects in reconnect mode. Queue and active slot stay unchanged. Verify: a test with a fake transport whose status fails asserts the `Disconnected` broadcast and an unchanged queue revision. Inject time, use no sleeps.
- [x] 3.4 Add **Playback output** to `CONTEXT.md`, and redefine *Cast attachment* (design D14). Verify: the term entry exists and *Cast attachment* no longer says "held beside `connected_session_id`" or "attaches on launch".

## 4. Cast source resolution in the Owner

- [x] 4.1 Move the Cast resolve into `mbv-daemon`: `resolve_emby_cast_item` and `resolve_abs_episode_cast_item` from `src/app/dispatch/cast.rs`, and `CastProgressTarget`. `resolve_feed_dispatch` (`crates/mbv-cast/src/dispatch.rs:80`) and `build_cast_device_profile` (`crates/mbv-cast/src/dispatch.rs:51`) already live in `mbv-cast`; call them from `mbv-daemon`, do not move them. The result is `prepare_cast_source(item, ctx) -> Result<CastSource, Uncastable>` (design D6). The ABS episode session stays open and its lifecycle is returned. It is not closed at resolve. Verify: one `#[case]` table test over uncastable classification (ABS book, feed without a media URL) with the reason for each.
- [x] 4.2 Run the resolve on a resolve thread and re-enter through a `DaemonEvent` that carries the slot id and the transition generation. A result for a slot that is no longer the target is dropped. Verify: one test where a stale resolve result produces no load.

## 5. Routing at the funnels and transport commands

- [x] 5.1 Branch `play_resolved_items`, `handle_queue_replace`, the cold path of `send_jump_or_cold_start`, and `dispatch_slot_jump` on `PlaybackOutput::Cast`: set the active slot, register the in-flight transition, start the resolve, and on success send `load(item, start)` to the worker (design D4). An explicit play or jump of an uncastable slot names the item and the reason and loads nothing. Verify: a regression test for #944 (play of a 300-slot queue gives the fake transport exactly one `load`, with the selected slot's URL); one test that playing an ABS book loads nothing and reports.
- [x] 5.2 Settle a Cast transition when the status shows the loaded content id in Playing, Buffering, or Paused. Expire it on load or resolve failure (design D5). Verify: one test where two quick Next commands give one final `load` two slots ahead.
- [x] 5.3 Route `TogglePause`, `Seek`, `SeekAbsolute`, `SetVolume`, and `SetMute` to the receiver. Reject `SetAudio`, `SetSub`, and `SetSubtitlePrefs` with `CommandRejected`. Stop stops the receiver media. Verify: one `#[case]` table test (routed command lands on the fake transport, track command is rejected).

## 6. Status, advance, and progress

- [ ] 6.1 Translate Cast status into the Owner's `PlayerStatus` (active slot, position, duration, paused). Extrapolate on the tick while playing. Publish `StatusOnly` at the cadence of local playback (design D8). Verify: tests with an injected clock: playing advances the position; paused or buffering holds it; a report corrects drift.
- [ ] 6.2 Apply the advance table of design D7 with the end-handled flag. An uncastable step target is reported and stepped over, limited by the queue length. At an edge, the output stays idle. Verify: tests: `Finished` gives one `load` of the next slot; the same `Finished` seen in two polls gives only one `load` (double-advance guard); `Cancelled` and `Error` give no `load` (`Error` is reported); foreign content gives no advance; an uncastable next slot is reported and skipped.
- [ ] 6.3 Record a natural end through the same completion observation path that `TrackCompleted` uses (progress, consume rules). Verify: one test that the completed slot's canonical progress or consume state matches the local-playback outcome for the same slot kind.
- [ ] 6.4 Progress reporting (design D9): build an Emby `SessionReporter` from the `CastProgressTarget` ids, observe/sync/close the ABS lifecycle on status, advance, stop, and output change. Feed: the Owner emits the ctrl `PlayerEvent`s that drive the TUI's `persist_feed_slot_lifecycle` for local playback (design D9): `PausedChanged` on a receiver pause change, `OutputStarted` when a seek settles, `Stopped` on a user stop or output change, and `TrackCompleted` on a natural end (shared with 6.3). The Owner does not write the Feed state file. Report stop for the old output before start for the new one. Verify: one test with a mocked reporter boundary: progress is reported only while the receiver's content id matches the loaded item; one test that a receiver pause and a natural end of a Feed slot emit `PausedChanged(true)` and `TrackCompleted` to Clients.

## 7. Hand-off and lifecycle

- [ ] 7.1 Implement the hand-off of design D10 for Local→Cast, Cast→Local, and Cast→Cast. Connect first; stop the old output only after a successful connect. Verify: tests: Local playing at a position gives a receiver `load` at that position; a paused state carries over; a connect failure leaves local playback running.
- [ ] 7.2 Check whether a local cold start can begin paused at a resume position. If it cannot, start the run and then pause it. Verify: a test that Cast→Local from a paused receiver gives a paused local run at the receiver's position.
- [ ] 7.3 On Owner shutdown, drop the receiver connection without `stop_app`. A deliberate output change or detach tears down the app (design D12). Verify: tests with the fake transport: shutdown records no teardown; a select of `Local` records a teardown.
- [ ] 7.4 Reconnect recovery: after `Disconnected`, a reconnect that sees `Idle(Finished)` for the loaded content advances once. A reconnect refused because another app runs stays `Disconnected` (design D11). Verify: one test for each outcome.

## 8. TUI becomes a thin picker

- [ ] 8.1 F3 Cast selection and detach send `SelectPlaybackOutput`. Cast rows are offered only when the owner advertises `playback-output`. The connection mark, the output indicator, and the idle keep-panel rule read `CtrlEvent::PlaybackOutput` (design D13). Verify: one shell-layer test (per the Tests matrix in `docs/architecture/tui-frontend.md`) that a Cast row activation sends the select command.
- [ ] 8.2 In `sever_active_connection` (`src/app/state/remote_slot.rs:82`), replace the `cast_attachment.take()` arm: when the last `CtrlEvent::PlaybackOutput` says Cast, send `Stop` and then `SelectPlaybackOutput(Local)` to the Local daemon owner. `connect_to_session` (`src/app/dispatch/session/switch.rs:439`) calls this site for the Sessions panel and auto-reconnect (design D13; spec scenario "A session or direct remote is connected while a cast target is attached"). Verify: one shell-layer test that a session connect while the output is Cast sends `Stop` then `SelectPlaybackOutput(Local)`, and sends neither while the output is Local.
- [ ] 8.3 Delete the TUI Cast runtime: `CastAttachment`, `CastPlaybackTarget` and the Cast branch of `playback_target()`, `cast_status.rs` (poll, keep-alive, watchdog, extrapolation, progress), `play_selection_on_cast`, `dispatch_selection_to_cast`, `dispatch_queue_play_cursor_to_cast`, `apply_cast_dispatch`, `cast_active_queue_index`. Delete `src/app/tests/queue/cast_dispatch.rs` and the tests of deleted code. Keep `spawn_cast_discovery`. Verify: `cargo clippy -p mbv --all-targets -- -D warnings` is clean.
- [ ] 8.4 Delete the receiver-queue code in `mbv-cast`: the `dispatched_queue` field, `DispatchedQueue`, `jump`, `next_queue_index` and its two tests, `skip_next`/`skip_previous` on `CastClient` and on `CastTransport`, and `TransportKind::{Jump, JumpNoQueue, JumpNoAdjacent}` with their constructors and `Display`/`kind_name` arms. Verify: `rg "JumpNoQueue|JumpNoAdjacent|dispatched_queue"` finds nothing, and `cargo clippy -p mbv-cast --all-targets -- -D warnings` is clean.
- [ ] 8.5 Retire the *Dispatch:* term in the *Cast* section of `CONTEXT.md`, and add the Cast receiver as an output projection in the "Playback and queue authority" section of AGENTS.md. Verify: neither file describes a receiver-owned queue.

## 9. Integration

- [ ] 9.1 Run the full gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. Verify: all three pass.

## Workflow follow-up

- Run `make check-code-file-lines` before push.
- Sync the delta specs and archive the change.
- Close #944. Mark the Cast half of #946 as done in a comment.
