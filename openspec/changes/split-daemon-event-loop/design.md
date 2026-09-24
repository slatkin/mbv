# Design

## Context

What the loop in `run_with_options` currently reads from setup: `owner: DaemonPlayerOwner`, `player: Player`, `shared_queue: SharedQueueState`, `ctrl_clients` (Arc<Mutex<CtrlClients>>), `client` (the config/Emby client mutex), `emby_runtime` (the Ws arm checks `generation`), `audiobookshelf_runtime`, `merged_tx`, `ws_send_tx`, `direct_commands`, `config.stay_alive`, `role`, `audio_only`, and the keepalive/capabilities timers.

Persistence runs once per loop pass when `role == DaemonRole::Local && owner_queue_dirty`, through `persist_stay_alive_owner_queue` (`daemon_control_queue.rs:3`), which calls `config::save_stay_alive_queue_state`. `DaemonEvent::Shutdown` persists, notifies and flushes clients, stops and joins the player, removes the pid file, then calls `process::exit(0)`.

Existing test seams: `daemon_tests.rs::cold_player()` (no mpv), `CtrlClients::default()` and registry-backed clients (`daemon_tests.rs:262`), plus direct tests of `apply_track_completed_observation` / `apply_stopped_observation` (`daemon_tests.rs:710-800`).

Open items resolved from the code:
- `DaemonEvent::Ws { generation, event }` holds a plain `WsEvent` enum (`ws.rs:54`). Tests can construct it without a socket. Use a variant that makes no network call (for example `WsEvent::Stop` or `Pause`), not `Play`, which fetches items.
- An empty `CtrlClients::default()` works as a sink when a test only checks queue state and persistence. Tests that assert broadcasts use the existing registry-client pattern.

## Goals / Non-Goals

**Goals:**
- `handle_event` processes exactly one event and never exits the process.
- Each of the four dirty-flag arms is covered by tests that check both the queue/lineage effect and whether a snapshot is persisted.
- `daemon_run.rs` and `daemon_loop.rs` are each 800 lines or fewer.

**Non-Goals:**
- Changing arm behaviour, fencing rules, broadcast order or the persistence format.
- Tests for the Ctrl, AudiobookshelfProgress, QueueEnriched or CtrlDisconnected arms (allowed, not required).
- Extracting the socket or thread setup.

## Decisions

- **`DaemonLoop` owns all loop state as fields.** `handle_event` takes the per-pass dirty flag as a local and runs the persist step at the end of the call. That keeps "one event = at most one persist", as today. The alternative of returning a dirty bool and letting the caller persist was rejected because it would put persistence logic back in setup.
- **`LoopFlow::Shutdown`.** The Shutdown arm does everything except pid removal and exit, then returns `Shutdown`. `run_with_options` removes the pid file and exits. The `recv_timeout` / keepalive tick stays in `run_with_options` and calls `handle_event` per event; a `tick(&mut self, now)` method holds the keepalive/capabilities work so it can move out too.
- **Injected store is a boxed `FnMut` field.** Production passes `config::save_stay_alive_queue_state`. Tests use a closure, so the field type is `Box<dyn FnMut(&StayAliveQueueState) -> Result<(), String>>` and tests can capture a `Vec` of snapshots. `persist_stay_alive_owner_queue` builds the snapshot and hands it to the store.
- **Each `continue` becomes `return LoopFlow::Continue`.** That skips persistence for the pass, which is the same as today.
- **Stopped arm:** `if let PlayerEvent::Stopped { slot_id, run_identity, position_ticks, played, error, .. } = &pe` is bound once, and the pending-match, cancel, apply, commit and broadcast logic uses those bindings. This removes the `unreachable!()`.
- **Test location:** new `daemon_loop_tests.rs`, so `daemon_tests.rs` (1071 lines) doesn't grow. A `test_loop()` builder combines `cold_player()`, an empty `CtrlClients`, `role = Local` and a recording store.
- **Idle-load install keeps its direct persist (out of scope).** `install_idle_queue_load` (`daemon_control.rs`) is a third `persist_stay_alive_owner_queue` caller, reached from `handle_ctrl_for_role` and from `complete_pending_idle_queue_load` on a committed pending idle load. It keeps the production store `config::save_stay_alive_queue_state`; the injected `DaemonLoop` store covers the loop-pass persistence only. Threading the store through it would change `handle_ctrl_for_role`'s signature and ~30 test call sites for no observable effect. Consequently §3's idle-load test asserts that the loop-pass store records exactly one snapshot for a committed pass (row 3.3 "persists once"); the install's pre-existing inline write is not asserted and, under test, lands in the framework's test state dir (`TestStateDirGuard` / `TEST_DEFAULT_STATE_DIR`, `config_test_support.rs`).

## Risks / Trade-offs

- A large code move can hide a behaviour change. Mitigation: the move is mechanical and arm bodies keep their order. The review diff should be read with `git diff --color-moved`.
- `Box<dyn FnMut>` adds one allocation and indirection for a once-per-pass call. Negligible.
