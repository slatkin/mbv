# Tasks

Every new test names the spec requirement it owns, in a comment or in its name (`docs/invariants/14-test-ownership.md`). Tests covering Bare mode, optimistic edits, or Client persistence are deleted, not ported. Verification gate for every task, unless it says otherwise: `cargo check -p <crate>` for each touched crate, `cargo nextest run -p <crate>` for each touched crate, and `cargo clippy -p <crate> --all-targets -- -D warnings`.

## 1. Owner lifetime policy (crates/mbv-daemon, mbv-ctrl, mbv-remote-player)

- [ ] 1.1 Replace `DaemonLoop.stay_alive: bool` (`crates/mbv-daemon/src/event_loop.rs`) and `CtrlContext.stay_alive` (`control.rs`) with an injected reader, `Arc<dyn Fn() -> bool + Send + Sync>` (design D2).
  - Production (`run.rs`, where `stay_alive: config.stay_alive` is set today) reads the `stay_alive` key from the user's config file on each call, using the existing `mbv-config` loader.
  - `prepare_shutdown` calls the reader instead of reading the field.
  - Test: a loop test starts with a reader returning true, flips it to false, and asserts that `RequestShutdown` is accepted. It owns `daemon-lifecycle` "The daemon reads Stay Alive when it decides".
- [ ] 1.2 Check that the settings `StayAlive` toggle (`src/app/dispatch/settings.rs` `toggle_config_setting`) writes the config file before returning to the event loop. If it only mutates memory, persist it at that site.
  - Verify by reading the call chain. If a write was added, add a unit test asserting the persisted value after toggling, using `mbv_config::TestStateDirGuard`.
- [ ] 1.3 Add `DaemonEvent::LastClientGone` (design D3, "Last client lost").
  - `ClientRegistry` (`crates/mbv-daemon/src/ctrl.rs`) takes the loop's `merged_tx`. It sends the event when `remove` or the pruning in `broadcast_to_all` leaves it empty after having held a client.
  - The loop handles the event: when the reader returns false and the registry is still empty, run the `DaemonEvent::Shutdown` handling; otherwise continue.
  - Tests, each owning `daemon-lifecycle` "Ordinary disconnect is not shutdown":
    - a registry test: pruning the only client emits one event;
    - a registry test: a fresh registry emits none;
    - a loop test: with the reader false, `LastClientGone` persists through the injected `store` and flows to shutdown;
    - a loop test: with the reader true, it continues.
- [ ] 1.4 Add exclusive admission (design D3, "Admission").
  - Add `DisconnectReason::ExclusiveOwner { pid: u32 }` (`crates/mbv-ctrl/src/events.rs`) with an explicit serde rename, matching the existing variants.
  - In `crates/mbv-daemon/src/core_ctrl_spawn.rs`, after `Hello` validation, take the registry lock. If the reader returns false and the registry is non-empty, send `Disconnected(ExclusiveOwner{pid: std::process::id()})` and close. Otherwise `connect`, still under the same lock.
  - Map the reason to a new `RemotePlayerError::ExclusiveOwner { pid }` in `crates/mbv-remote-player` `connect_endpoint`.
  - Tests:
    - admission refuses when the reader is false and a client is attached, and admits when the reader is true or the registry is empty. Owns `daemon-lifecycle` "Stay Alive off admits one client".
    - a `connect_endpoint` test maps the refusal to the error.
- [ ] 1.5 Gate the local daemon's tray start on the reader as well as `show_systray_icon`. The site is the tray-ready hook in `src/local_daemon.rs` / `run_local_daemon_main`.
  - Verify by `cargo check`, plus a manual check in group 8. No unit test: the tray is a real D-Bus external.

## 2. Answered queue operations: protocol and owner (design D6)

- [ ] 2.1 In `crates/mbv-ctrl`:
  - add `QueueOpId(u64)`;
  - add `#[serde(default)] op: Option<QueueOpId>` to `UnifiedQueueReplace`, `UnifiedQueueLoadIdle`, `UnifiedQueueAppend`, `UnifiedQueueRemoveSlot`, `UnifiedQueueRemoveSlots`, `UnifiedQueueMoveSlot`, `UnifiedQueuePlaySlot`, `UnifiedQueueClear`, `UnifiedQueueSourceUpdate` (`commands.rs`);
  - add `#[serde(default)] before: Option<u64>` to `UnifiedQueueAppend`;
  - add the command `UnifiedQueueRefresh { op }`;
  - add the event `QueueOpResult { op, outcome: QueueOpOutcome }`, with `QueueOpOutcome::{Applied(Box<UnifiedQueueStateData>), Rejected(String)}`;
  - add the hello capability `answered-queue-ops`, next to the existing capability strings and `supports_*` helpers.
  - Test: one serde test showing that a command JSON without `op`/`before` still parses. Owns `unified-playback-queue` "Unified ctrl behavior is capability-gated and additive".
- [ ] 2.2 In `crates/mbv-daemon/src/control/queue_edit.rs`, `queue_setup.rs` and `control_queue.rs`:
  - When a command carries `op` and the client advertised `answered-queue-ops`, send `QueueOpResult{Applied}` with the resulting snapshot to the sender only, and broadcast to every other client. Add `except: Option<CtrlClientId>` to `broadcast_queue_state`.
  - `reject_command` sends `QueueOpResult{Rejected}` instead of `CommandRejected` when `op` is present.
  - `UnifiedQueueAppend.before` inserts before that slot. An absent anchor is rejected as stale.
  - Tests in `crates/mbv-daemon/src/tests/queue_ops/`, owning `unified-playback-queue` "Queue edits are answered before the next input":
    - the sender receives the result and no broadcast, while a second client receives the broadcast;
    - a stale `before` anchor is rejected with the op id.
- [ ] 2.3 Handle `UnifiedQueueRefresh`: start the daemon's existing Emby queue enrichment fetch for the current Emby slots (the path that ends in `run.rs` `apply_queue_enriched`) and answer `Applied` with the current snapshot at once.
  - Audit every unified queue handler for network or other blocking I/O on the loop thread. Move any you find to the existing worker/merged-event pattern, and list what was moved in `design.md` under Risks.
  - Test: refresh is answered at once, and a later `QueueEnriched` event produces an ordinary broadcast.
- [ ] 2.4 In `crates/mbv-remote-player`:
  - add `send_queue_op(QueueOp) -> Result<Option<QueueOpId>, RemotePlayerError>`, which mints ids per connection. It returns `None` and sends the legacy form when the peer lacks `answered-queue-ops`.
  - The reader thread forwards `QueueOpResult` as a new `PlayerEvent::QueueOpResult` (`crates/mbv-ctrl/src/player.rs`) on the same `player_rx`, keeping delivery order.
  - Tests: capability present → `Some` with an op on the wire; absent → `None` with no op; an inbound result reaches `player_rx` after an earlier inbound `UnifiedQueueState`.

## 3. Every local launch attaches to the local daemon (design D1, D4, D8)

- [ ] 3.1 In `src/main.rs` `run_local_instance`:
  - The `Fresh` branch always releases the guard, calls `local_daemon::spawn_detached`, and attaches (today's `stay_alive` path). Delete the `App::new_independent` branch.
  - A `RemotePlayerError::ExclusiveOwner{pid}` from `connect_endpoint` in the `Attach` branch prints the refusal message required by `local-daemon-single-instance` "Refusing a second terminal explains how to proceed" and exits 1.
  - `Resolution::Refuse` prints that the owner process is not accepting connections and names `mbv -q`.
  - Verify with `cargo check -p mbv`, plus the manual checks in group 8.
- [ ] 3.2 Parity in `App::new_remote_optional_with_config` (`src/app/state/construct/remote.rs`):
  - call `should_open_services` / `open_services_settings` as `new_independent` does;
  - set `system_notifications` from config when `stay_alive` is false, keeping `false` otherwise.
  - Test: with no Emby setup, the remote constructor opens services settings. Owns `daemon-lifecycle` "The local daemon is independent of Emby setup".
- [ ] 3.3 Keep the home link (design D4) in `src/app/dispatch/session/switch.rs` (`switch_to_direct_remote`, the Library-route switch) and `connect.rs`:
  - When the current player is the home link (`home_is_local_daemon` and the endpoint is `Local`), move it into `SuspendedLocalSession` instead of `disconnect_remote()`. Send Stop through it first only when `stay_alive` is false.
  - `restore_local_mode` reinstates the suspended home link. Delete its `home_is_local_daemon` reconnect branch, except the owner-lost path.
  - Tests: switching from the home link keeps it suspended and connected; restoring reinstates it. Each owns `player-target-locality` "Daemon target transitions update classification".
- [ ] 3.4 While the home link is suspended, drain its `player_rx` once per tick in the shell drains (`src/app/shell/run/drains.rs`):
  - adopt `UnifiedQueueUpdated` and `QueueOpResult` into the Local queue (`player_tab` until group 6);
  - drop every other event.
  - Add `App::queue_link(scope)`, returning the home link (current or suspended) for Local and the current player for Remote.
  - Test: an owner snapshot arriving on the suspended home link updates the Local queue while Remote is viewed. Owns `local-daemon-thin-client` "Every local Client shows the owner's accepted queue".
- [ ] 3.5 Teardown (`src/app/dispatch/run_loop/teardown.rs`): send `RequestShutdown` over the home link, current or suspended, and delete the short-lived `DaemonEndpoint::Local` connection path.
  - Test: teardown with `stay_alive` false while routed remote sends the request over the suspended home link. Owns `daemon-lifecycle` "Quitting with Stay Alive off stops this machine's local daemon".
- [ ] 3.6 Fall-through (`prepare_local_player` in `switch.rs`):
  - Reinstate the suspended home link when present.
  - Otherwise, for a client launched on an explicit endpoint, run the local-launch resolution: spawn if absent, then attach. Return `Err` on failure, including `ExclusiveOwner`.
  - Delete `App::construct_local_session`, and the `expect(clippy::unnecessary_wraps)` on `prepare_local_player`, because the `Err` path is now real.
  - Test: an explicit-endpoint client whose local attach is refused keeps its attachment and reports the failure. Owns `non-audio-fall-through` "Local Player preparation precedes ending the attachment".

## 4. Delete Bare ownership (compiler-driven; design D1)

- [ ] 4.1 In `crates/mbv-player/src/proxy.rs`, delete `PlayerProxy::local`, `PlayerProxyInner::Local`, `inhibit_mpv`, and `PlayerProxy::is_remote()`. Then fix the compile errors in `src/app/dispatch/session/` and `src/app/dispatch/run_loop/` only, by deleting the `!is_remote()` / in-process branches:
  - the `player.stop()`, `join`, and `join_or_timeout` local paths;
  - `stop_for_shutdown`;
  - the Bare `SuspendedLocalSession` construction from an in-process player.
  - Leave errors in other directories for 4.2. Delete tests that construct a local player.
  - Verify: no errors remain under those two directories (`cargo check -p mbv 2>&1 | rg 'src/app/dispatch/(session|run_loop)'` is empty).
- [ ] 4.2 Finish the `is_remote()` compile errors in the rest of `src/app`: `state/playback_target*`, `dispatch/queue*`, `dispatch/action.rs`, `shell/`, and any others.
  - Delete `bare_owner` (`app_struct.rs`, `construct.rs`), `reset_bare_transitions`, `expire_bare_transition` (and its call in `shell/run.rs`), `bare_in_flight_slot`, `App::new_independent`, and `local_queue_is_owner_queue`'s `ThisProcess` branch.
  - Verify: `cargo check -p mbv` and `cargo nextest run -p mbv` pass.
- [ ] 4.3 Delete Client queue persistence and the generation fence:
  - `save_queue_state`, `save_queue_state_no_clear`, `save_queue_state_after_explicit_clear`, `restore_queue_state`, `build_queue_state`, `persist_local_queue_state_if_needed` (`dispatch/queue/playlist_mutation.rs`, `state/queue_scope.rs`);
  - `spawn_enrich_queue_state` and `handle_queue_enriched`;
  - `PlayerTab.sequence_generation` and `stamp_queue_generation`;
  - `LocalQueueOwner` with `owns_local_persistence` (`state/queue_owner.rs`), with `QueueOrigin::ThisProcess` removed and `QueueOrigin` becoming a struct `{ epoch, lineage }`;
  - `set_queue_source_if_not_local_daemon`. Delete its callers' writes, because the source now arrives only by adoption.
  - Keep `mbv_config::load_queue_state` / `save_queue_state` only if a non-test caller outside `src/app` remains. The daemon's legacy takeover uses `load_queue_state`.
  - Verify: the `mbv` gate passes. `rg 'owns_local_persistence|ThisProcess|sequence_generation' src` returns only `PlayerStatus`/ctrl uses of `sequence_generation`.

## 5. Answered edits in the Client (design D6, D7)

- [ ] 5.1 Add `App::queue_op(scope, QueueOp) -> QueueOpOutcome`-like result (`src/app/dispatch/queue/queue_op.rs`).
  - Resolve the link with `queue_link(scope)`, call `send_queue_op`, and when it returns `Some(id)`, call `await_queue_op`.
  - `await_queue_op` pumps that link's receiver through the ordinary player-event handler, in order, until `PlayerEvent::QueueOpResult{op: id}` or `QUEUE_OP_ANSWER_BOUND = 250 ms`. It adopts `Applied` as the Client's own answer, and flashes on rejection or timeout.
  - Tests with an injected `player_rx`, owning `unified-playback-queue` "Queue edits are answered before the next input":
    - an earlier snapshot queued ahead of the answer is adopted before it;
    - a timeout leaves the view unchanged and flashes;
    - `None` (legacy peer) returns without waiting.
- [ ] 5.2 Move removal, range removal, move and undo onto `queue_op`, in `src/app/dispatch/queue.rs` (`remove_from_queue`, `remove_slots_from_queue`, move handlers) and the undo handler:
  - delete the local `remove_slot_at` / `move_slot` / `insert_item_at` calls, `sync_playback_queue_items_after_append`, `queue_edit_reaches_player`, and `queue_edits_reach_owner`;
  - `UndoEntry` becomes `Remove{item, index}` / `Move{slot_id, from}`, with the `before` anchor resolved at undo time (design D7).
  - Tests:
    - three consecutive removals each act on the answered state. Owns "Queue edits are answered before the next input", scenario "Rapid repeated removals".
    - undoing a removal sends `Append{before}` anchored at the slot now at that index. Owns "Queue undo is an owner operation".
- [ ] 5.3 Move append/enqueue, clear, replace/load (`replace_playback_queue`, `dispatch/queue/replacement.rs`, `pending_playback.rs`), play-slot, source update (`apply_saved_playlist_source`), and refresh (`dispatch/library/load.rs` `refresh_queue`) onto `queue_op`.
  - Delete `App::merge_refreshed_queue` and `consume_slot_from_active_playback_queue`.
  - Test: queue refresh sends `UnifiedQueueRefresh` and does not change the view before the answer. Owns "Clients hold no editable queue", scenario "Queue refresh".

## 6. `QueueView`: an adopt-only queue (design D5)

- [ ] 6.1 Create `src/app/state/queue_view.rs` with `QueueView`: private fields, the read accessors, `set_cursor`, `from_snapshot`, and `adopt(&UnifiedQueueStateData, AdoptCause)` with `AdoptCause::{OwnAnswer, Background{held}, Replacement}`. It mints a Client-local revision on each adoption through `QueueRevisionMint`.
  - Tests: one `#[case]` table over the selection outcomes in `unified-playback-queue` "Queue selection follows the selected slot": selected slot kept after a move; selected slot removed → next entry; last entry removed → new last; background not held → active slot; background held → kept; replacement → start.
  - A separate test: each adoption yields a new revision. Owns `queue-canonical-list` "A queue revision names one queue state".
- [ ] 6.2 Replace `App.player_tab: PlayerTab` with `local_view: QueueView` and `remote_player_tab: Option<PlayerTab>` with `remote_view: Option<QueueView>`, and delete `src/app/state/player_tab.rs`.
  - Route every adoption site through `adopt` with the right cause: `handle_unified_queue_updated` → `Background{held: queue_cursor_held_by_user()}`, or `Replacement` when the lineage changed; `await_queue_op` → `OwnAnswer`; attach/connect/switch bootstraps → `Replacement`.
  - Delete `pending_queue_edit_cursor` and `pending_remote_move_cursor` (`app_struct.rs`), and every branch that reads them.
  - Verify: the `mbv` gate passes, and `rg 'PlayerTab|pending_queue_edit_cursor|pending_remote_move_cursor' src` is empty.
- [ ] 6.3 Update the projection to read the pending slot only from the adopted snapshot (`playback_target.rs` `pending_playback_slot`, `displayed_playback_state`). Delete `queue_row_playback_state`'s fence clause.
  - Test: a snapshot whose pending transition differs from the observed slot projects the pending slot with no progress. Owns `queue-canonical-list` "Queue projection is bounded presentation data".

## 7. Documentation

- [ ] 7.1 Add `docs/adr/0030-owner-process-is-the-only-local-player-owner.md`, recording design D1, D4 and D6 with the rejected alternatives.
  - Add a status note to ADR 0017 retiring the Composed stage: every queue is Bound, and the local daemon holds an idle Bound queue.
  - Add a status note to ADRs 0015, 0016, 0019 and 0029 wherever they describe Bare in-process ownership.
  - Verify: each amended ADR links to 0030.
- [ ] 7.2 Update `CONTEXT.md` with the naming the maintainer approved (design Open Questions):
  - Bare mode, Stay-alive, Stay-alive process / Owner process, Client, Composed, Owner-held queue and source, Tray;
  - correct the Audiobookshelf eligibility line under Bare mode, since the local daemon admits episodes and books.
  - Update the AGENTS.md first paragraph ("playback runs Bare, via the Stay-alive process, or packaged `mbvd`").
  - Verify: `rg -n -i 'bare mode' CONTEXT.md AGENTS.md` shows only _Avoid_ entries.
- [ ] 7.3 Delete `docs/invariants/13-stay-alive-owner-is-the-queue.md`, because `QueueView` and `queue_op` enforce it.
  - Update invariant 01, point 3, to name `QueueView` instead of `PlayerTab::default`.
  - Comment on GH #810, ticking 13 and 14. Invariant 14 is "kept documented: a process rule types cannot express".

## 8. Integration verification

- [ ] 8.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All must pass.
- [ ] 8.2 Manual checks against the built binary. Record each result in this task's line when done.
  - Stay Alive off:
    - launching starts one local daemon, and quitting leaves no mbv process;
    - a second terminal is refused with the pid and both remedies;
    - `kill -9` on the TUI makes the daemon exit;
    - there is no tray.
  - Stay Alive toggled off during a session, then quitting, makes the daemon exit.
  - Pressing `d` three times rapidly removes three entries, and `u` restores the last one in place.
  - During direct remote control, a Local-scope edit reaches the local daemon.
  - A fall-through video plays on the local daemon.
  - The queue survives quitting and relaunching with Stay Alive off.
- [ ] 8.3 Just before pushing, run `make check-code-file-lines` and split any governed file over 800 lines along responsibility seams.
