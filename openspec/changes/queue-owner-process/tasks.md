# Tasks

Every new test names the spec requirement it owns, in a comment or in its name (`docs/invariants/14-test-ownership.md`). Tests covering Bare mode, optimistic edits, or Client-side queue writes are deleted, not ported. Verification gate for every task, unless it says otherwise: `cargo check -p <crate>` for each touched crate, `cargo nextest run -p <crate>` for each touched crate, and `cargo clippy -p <crate> --all-targets -- -D warnings`.

## 1. Owner lifetime and settings (crates/mbv-daemon, mbv-ctrl, mbv-remote-player, src/local_daemon.rs)

- [x] 1.1 Add the `OwnerSettings { stay_alive, consume_videos, consume_audio }` reader, `Arc<dyn Fn() -> OwnerSettings + Send + Sync>` (design D2).
  - It replaces `DaemonLoop.stay_alive` (`crates/mbv-daemon/src/event_loop.rs`), `CtrlContext.stay_alive` (`control.rs`), and the spawn-time `consume_*` read (`event_loop/player_events.rs`, the `ConsumePolicy` block).
  - `DaemonRole::Local` reads the config file on each call, falling back to the last successful read and then to the spawn config.
  - `DaemonRole::Packaged` returns the spawn config with `stay_alive: true`.
  - `prepare_shutdown` and the consume-policy read call the reader.
  - Tests, owning `daemon-lifecycle` "The daemon reads Stay Alive when it decides":
    - a loop test flips the reader from true to false, and `RequestShutdown` is accepted;
    - a loop test flips `consume_audio` on, and the next completion consumes;
    - a Packaged-role test: the reader reports `stay_alive: true` whatever the config says.
- [x] 1.2 In `src/app/dispatch/settings.rs`, make toggling `StayAlive`, `ConsumeVideos` or `ConsumeAudio` flush the debounced settings save (`settings_save_at`) at once. Make teardown flush any pending save before `request_teardown_shutdown`.
  - Test with `mbv_config::TestStateDirGuard`: the toggled value is on disk right after the toggle.
- [x] 1.3 Add `DaemonEvent::LastClientGone` (design D3, "Last client lost").
  - Replace `CtrlClients::default()` in `run.rs` with a constructor taking `merged_tx`.
  - Emit the event whenever `remove` or any of the four `retain` sites (`broadcast_to_all`, `broadcast_state_gated`, `broadcast_progress_gated`, `broadcast_book_progress_gated`) leaves the registry empty after it has held a client.
  - The loop handles it: if the role is Local, `!stay_alive`, and the registry is still empty, run the `DaemonEvent::Shutdown` handling.
  - Tests, owning `daemon-lifecycle` "Ordinary disconnect is not shutdown":
    - pruning the only client from a gated broadcast emits one event;
    - a fresh registry emits none;
    - with the Local role and the reader false, the loop persists through the injected `store` and shuts down;
    - with the reader true, or the Packaged role, it continues.
- [x] 1.4 Add admission (design D3).
  - Add `DisconnectReason::ExclusiveOwner { pid: u32 }` and `DisconnectReason::OwnerShuttingDown` (`crates/mbv-ctrl/src/events.rs`), with explicit serde renames.
  - Add a `shutting_down` flag to `CtrlClients`, set as the first step of `handle_shutdown`.
  - In `core_ctrl_spawn.rs`, after `Hello` validation and **before** `send_initial_queue_state`: take the registry lock; refuse on `shutting_down`; refuse with `ExclusiveOwner{pid: std::process::id()}` when the role is Local, `!stay_alive`, and a client is attached; otherwise `connect` under the same lock.
  - Map both reasons to `RemotePlayerError::{ExclusiveOwner{pid}, OwnerShuttingDown}` in `crates/mbv-remote-player` `connect_endpoint`.
  - Tests:
    - admission is refused (reader false and a client attached) or admitted (reader true, registry empty, or Packaged). Owns "Stay Alive off admits one client".
    - admission is refused while shutting down. Owns "A shutting-down daemon admits no client".
    - a refused connection receives no queue state.
    - `connect_endpoint` maps both reasons.
- [x] 1.5 Local-role startup settings in `crates/mbv-daemon/src/run.rs` and `src/local_daemon.rs`:
  - honour `show_audio_window` for `DaemonRole::Local`, keeping it forced off for `Packaged`;
  - start the tray only when `stay_alive` is true at startup.
  - Verify with `cargo check`, plus the manual checks in 8.2. No unit test: the audio window and the tray are real externals.

## 2. Answered queue operations: protocol and owner (design D6)

- [ ] 2.1 In `crates/mbv-ctrl`:
  - add `QueueOpId(u64)`;
  - add `#[serde(default)] op: Option<QueueOpId>` to `UnifiedQueueReplace`, `UnifiedQueueAppend`, `UnifiedQueueRemoveSlot`, `UnifiedQueueRemoveSlots`, `UnifiedQueueMoveSlot`, `UnifiedQueuePlaySlot`, `UnifiedQueueSourceUpdate`. Leave `UnifiedQueueLoadIdle` unchanged;
  - add the struct variant `UnifiedQueueClearOp { op }`, keeping the unit `UnifiedQueueClear`, and include it wherever `mutates_owner_queue`-style matches list `UnifiedQueueClear`;
  - add `#[serde(default)] before: Option<u64>` to `UnifiedQueueAppend`;
  - add `UnifiedQueueRefresh { op }` and `UnifiedQueueApplyProgress { op, updates: Vec<ProgressUpdate> }`, where `ProgressUpdate` carries a provider-qualified identity, the position in ticks, and `finished`;
  - add the event `QueueOpResult { op, outcome: QueueOpOutcome::{Applied(Box<UnifiedQueueStateData>), Rejected(String)} }`;
  - add the hello capability `answered-queue-ops`.
  - Test: one serde test showing that legacy command JSON without the new fields, and the unit `UnifiedQueueClear`, still parse. Owns `unified-playback-queue` "Unified ctrl behavior is capability-gated and additive".
- [ ] 2.2 Owner answers in `crates/mbv-daemon` (`control.rs`, `control/queue_edit.rs`, `control/queue_setup.rs`, `control_queue.rs`):
  - Thread `op` through `CtrlContext`. **Every** handler path answers when `op` is present, including early returns (an empty append, an empty removal set, and so on). A no-op answers `Applied` with the unchanged snapshot.
  - Build `Applied` with the sender's ABS gating (`unified_queue_state_for_peer`).
  - Add `except: Option<CtrlClientId>` to `broadcast_queue_state`, and exclude the sender.
  - `reject_command` sends `QueueOpResult{Rejected}` when `op` is present.
  - Handle `UnifiedQueueClearOp` through the same function as `UnifiedQueueClear`.
  - Tests in `crates/mbv-daemon/src/tests/queue_ops/`, owning `unified-playback-queue` "Queue edits are answered before the next input":
    - the sender gets the result and no broadcast, while another client gets the broadcast;
    - an empty-set removal still answers;
    - a legacy command without `op` still broadcasts to everyone.
- [ ] 2.3 `UnifiedQueueAppend.before`: insert with `PlaybackQueue::insert`, then keep the player run in order with `PlayerCommand::QueueAppend` followed by `PlayerCommand::QueueMove(slot, index)`. An absent anchor is rejected as stale.
  - Tests:
    - the owner queue and the recorded player commands both place the item before the anchor;
    - a stale anchor is rejected with the op id.
- [ ] 2.4 `UnifiedQueueRefresh` and `UnifiedQueueApplyProgress`:
  - Refresh starts the existing enrichment fetch that ends in `run.rs` `apply_queue_enriched`, and answers `Applied` at once.
  - ApplyProgress applies each update to matching **inactive** slots through the owner's progress path, respecting invariant 02 protection, and answers with the resulting snapshot.
  - After the owner applies its own acknowledged ABS progress (`crates/mbv-daemon/src/audiobookshelf.rs`, both `apply_progress` sites), it also calls `broadcast_queue_state`.
  - Audit every unified queue handler for blocking I/O on the loop thread. Move any you find to the worker/merged-event pattern, and list the moves in `design.md` Risks.
  - Tests:
    - refresh is answered at once;
    - ApplyProgress skips the active slot;
    - acknowledged ABS progress is followed by a queue broadcast. Owns `audiobookshelf-podcast-playback` "Attached clients reconcile daemon-owned acknowledged progress".
- [ ] 2.5 In `crates/mbv-remote-player`:
  - add `send_queue_op(QueueOp) -> Result<Option<QueueOpId>, RemotePlayerError>` (`None` means the legacy form was sent);
  - the reader thread forwards `QueueOpResult` as `PlayerEvent::QueueOpResult` (`crates/mbv-ctrl/src/player.rs`) on `player_rx` in order, and updates `unified_queue` from `Applied`;
  - delete the `items` and `queue_source` fields, and the optimistic writes to them and to `status.current_idx` in `adopt_queue`, `play` and `play_queue`;
  - move every reader of `remote.items`/`remote.queue_source` (`RemoteSnapshot::take`, `switch_to_direct_remote`, and others) to `unified_queue_state()`.
  - Tests:
    - with the capability, `send_queue_op` returns `Some` and puts the op on the wire; without it, `None` and no op;
    - an inbound result reaches `player_rx` after an earlier `UnifiedQueueState`;
    - `adopt_queue` writes no queue field before an answer.

## 3. Every local launch attaches to the local daemon (design D1, D4, D8)

- [ ] 3.1 In `src/main.rs`:
  - `Fresh` always releases the guard, spawns (`local_daemon::spawn_detached`), and attaches. Delete the `App::new_independent` branch.
  - `ExclusiveOwner{pid}` prints the refusal required by `local-daemon-single-instance` "Refusing a second terminal explains how to proceed" and exits 1.
  - `OwnerShuttingDown` makes it re-run `single_instance::resolve` every 100 ms for up to 10 s, then show the refusal naming `mbv -q`.
  - `Resolution::Refuse` prints that the owner process is not accepting connections and names `mbv -q`.
  - The `-q` help text drops "bare mbv".
  - Verify with `cargo check -p mbv`, plus the manual checks in 8.2.
- [ ] 3.2 Parity in `App::new_remote_optional_with_config` (`src/app/state/construct/remote.rs`):
  - call `should_open_services`/`open_services_settings`;
  - set `setup.emby_startup_request` when Emby is configured and no client was passed in;
  - set `system_notifications` from config when `stay_alive` is false, and `false` otherwise.
  - Tests, each owning `daemon-lifecycle` "The local daemon is independent of Emby setup":
    - with no Emby setup, services settings open;
    - with `stay_alive` false and `system_notifications` true, the flag is true;
    - with Emby configured and no client, an Emby startup request is set.
- [ ] 3.3 Make `SuspendedLocalSession` link-only (`src/app/state/playback.rs`): delete `player_tab`, `queue_source`, and the other queue/WS fields that only Bare used, plus their write-back in `install_suspended_local` (`switch.rs`).
  - Home-link suspension: `switch_to_direct_remote`, the Library-route switch and `connect_daemon_route_endpoint` (`connect.rs`) move a current home link into `SuspendedLocalSession` instead of `disconnect_remote()`, sending Stop first only when `stay_alive` is false.
  - Any path that would connect to `DaemonEndpoint::Local` while a home link exists (the `endpoint.is_local()` branch of `switch_to_direct_remote`, `connect_daemon_route_endpoint` with `Local`, `restore_local_mode`) reinstates the suspended link instead.
  - `restore_local_mode` keeps the owner-lost restart path (`daemon_restart.rs`) for when the home link is gone.
  - Tests, owning `player-target-locality` "Daemon target transitions update classification":
    - switching from the home link keeps it suspended and connected;
    - a route to `Local` while suspended reinstates it and opens no new connection;
    - with `stay_alive` false, switching away sends Stop.
- [ ] 3.4 In `src/app/shell/run/drains.rs`, add `drain_suspended_home_events`, which drains the suspended home link **until empty** every tick:
  - `UnifiedQueueUpdated` and `QueueOpResult` go to a new `adopt_home_snapshot`, which writes only the Local queue (`player_tab` until group 6) and never `self.player.status`;
  - `RemoteDisconnected` goes to `raise_daemon_lost_modal`;
  - `DaemonShutdownAnnounced` goes to the clean-exit path;
  - everything else is dropped.
  - Add `App::queue_link(scope)`.
  - Tests:
    - a suspended-link snapshot updates the Local queue while Remote is viewed, and leaves `player.status` alone. Owns `local-daemon-thin-client` "Every local Client shows the owner's accepted queue".
    - a suspended-link disconnect raises the owner-lost modal.
- [ ] 3.5 Teardown (`src/app/dispatch/run_loop/teardown.rs`): send `RequestShutdown` over the home link, current or suspended, and delete the short-lived `DaemonEndpoint::Local` path.
  - Test: `stay_alive` false while routed remote sends the request over the suspended link. Owns `daemon-lifecycle` "Quitting with Stay Alive off stops this machine's local daemon".
- [ ] 3.6 Fall-through (`switch.rs` `prepare_local_player`, `play_pending_local_play`):
  - When the current player **is** the home link, return "already local": stop only the controlled Emby session, and neither stop nor disconnect the home link.
  - When the home link is suspended, reinstate it.
  - Otherwise (explicit-endpoint launch), run the local-launch resolution and return `Err` on refusal.
  - Delete `App::construct_local_session`, the `LOCAL_PLAYER_PREPARE_OVERRIDE` seam, and the `expect(clippy::unnecessary_wraps)`.
  - Tests, owning `non-audio-fall-through` "Local Player preparation precedes ending the attachment":
    - the home-link-current case keeps the link connected;
    - the suspended case reinstates it;
    - explicit-endpoint refusal keeps the attachment and reports the failure.

## 4. Delete Bare ownership (compiler-driven; design D1)

- [ ] 4.1 In `crates/mbv-player/src/proxy.rs`, delete `PlayerProxy::local`, `PlayerProxyInner::Local`, `inhibit_mpv`, and `PlayerProxy::is_remote()`. Then fix every compile error under `src/app/dispatch/` (production and test files), deleting the in-process branches: local stop/join, `stop_for_shutdown`, and Bare transitions.
  - Verify: `cargo check -p mbv --all-targets 2>&1 | rg 'src/app/dispatch/'` is empty.
- [ ] 4.2 Fix the remaining `--all-targets` compile errors in the rest of `src/app` and `src/main.rs`.
  - Production: `state/playback_target*`, `shell/`, `state/construct*`.
  - Test harnesses: `tests/tick_integration/harness.rs`, `tests/audiobookshelf_runtime.rs`, `tests/actions_tests_queue_state_reseat.rs`, and any others.
  - Delete `bare_owner`, `reset_bare_transitions`, `expire_bare_transition` (with its call in `shell/run.rs`), `bare_in_flight_slot`, `App::new_independent`, and the `ThisProcess` branch of `local_queue_is_owner_queue`.
  - Verify: `cargo check -p mbv --all-targets` and `cargo nextest run -p mbv` pass, and `rg 'new_independent|PlayerProxy::local|is_remote\(\)|bare_owner|bare_transition' src` is empty.
- [ ] 4.3 Delete Client queue persistence and the fence:
  - `save_queue_state*`, `restore_queue_state`, `build_queue_state`, `persist_local_queue_state_if_needed`, `spawn_enrich_queue_state`, `handle_queue_enriched`;
  - `PlayerTab.sequence_generation` and `stamp_queue_generation`;
  - `LocalQueueOwner` with `owns_local_persistence`, with `QueueOrigin` becoming a struct `{ epoch, lineage }`;
  - `set_queue_source_if_not_local_daemon` and its callers' writes.
  - Keep `mbv_config::load_queue_state`, which the daemon's legacy takeover uses.
  - Verify: the `mbv` gate passes, and `rg 'owns_local_persistence|ThisProcess|stamp_queue_generation' src` is empty.

## 5. Answered edits and event side effects in the Client (design D6, D7, D9)

- [ ] 5.1 Add `App::queue_op(scope, QueueOp)` and `await_queue_op` (`src/app/dispatch/queue/queue_op.rs`), and `App.deferred_player_events: VecDeque<PlayerEvent>`.
  - The pump reads `queue_link(scope)`'s receiver until `QueueOpResult{op: id}` or `QUEUE_OP_ANSWER_BOUND = 250 ms`:
    - it adopts `UnifiedQueueUpdated` inline (through `adopt_home_snapshot` for a suspended link);
    - it adopts the matching result as the own answer;
    - it pushes every other event onto `deferred_player_events`.
  - `drain_player_events` takes from `deferred_player_events` before `player_rx`.
  - A late or unmatched `Applied` is adopted as a background snapshot.
  - Tests with an injected receiver, owning `unified-playback-queue` "Queue edits are answered before the next input":
    - an earlier snapshot is adopted before the answer;
    - an interleaved `RemoteDisconnected` is deferred to the next tick, not handled in the pump;
    - a timeout flashes and leaves the view unchanged, and a late `Applied` is adopted afterwards;
    - `None` (legacy) returns without waiting.
- [ ] 5.2 Move removal, range removal, move and undo onto `queue_op` in `src/app/dispatch/queue.rs` (`remove_from_queue`, `remove_slots_from_queue`, move handlers) and the undo handler.
  - Delete the local `remove_slot_at`/`move_slot`/`insert_item_at` calls, `sync_playback_queue_items_after_append`, `queue_edit_reaches_player`, and `queue_edits_reach_owner`.
  - `UndoEntry` becomes `Remove{item, index}` / `Move{slot_id, from}` (design D7).
  - Tests:
    - three consecutive removals each act on the answered state (scenario "Rapid repeated removals");
    - undoing a removal sends `Append{before}` anchored at the slot now at that index. Owns "Queue undo is an owner operation".
- [ ] 5.3 Move append/enqueue, clear (`UnifiedQueueClearOp`), replace (`replace_playback_queue`, `dispatch/queue/replacement.rs`, `pending_playback.rs`), play-slot, source update (`apply_saved_playlist_source`) and refresh (`dispatch/library/load.rs` `refresh_queue`) onto `queue_op`.
  - Idle loads keep their existing `UnifiedQueueLoadIdle` request/result path, unblocked.
  - Delete `App::merge_refreshed_queue`.
  - Tests:
    - refresh sends `UnifiedQueueRefresh` and doesn't change the view before the answer. Owns "Clients hold no editable queue", scenario "Queue refresh".
    - an idle load doesn't block input. Owns scenario "Idle load while an item plays".
- [ ] 5.4 Event-driven queue writes, per the design D9 table:
  - Delete the Client queue-slot writes in `player_event.rs` (`record_reported_progress` and the consume removal `consume_slot_from_active_playback_queue`), feed hydrate (`action.rs`), `feed_tab.rs`, `event_reconcile.rs`, and `playlist.rs`.
  - Keep `on_video_consumed`/`on_audio_consumed`, `FeedEntryStore` writes, and `last_played_item_id` as table-listed side effects.
  - Relay ABS Socket.IO `user_item_progress_updated` to the home owner as `UnifiedQueueApplyProgress`, fire-and-forget, keeping the browse-state merge.
  - Apply ABS acknowledged-progress events to browse state only.
  - Tests:
    - a consuming `TrackCompleted` triggers `on_audio_consumed` and leaves the view unchanged until the owner's snapshot. Owns "Clients hold no editable queue".
    - a socket progress event sends `UnifiedQueueApplyProgress` and updates browse state. Owns `audiobookshelf-progress-refresh` "user_item_progress_updated merges into cached progress by provider-qualified identity".

## 6. `QueueView`: an adopt-only queue (design D5)

- [ ] 6.1 Create `src/app/state/queue_view.rs` with `QueueView`: private fields; read accessors including `source()` and `lineage()`; `set_cursor`; `from_snapshot`; `adopt(&UnifiedQueueStateData, AdoptCause)`. It mints a Client-local revision on each adoption.
  - Tests:
    - one `#[case]` table over the selection outcomes in "Queue selection follows the selected slot": move keeps the selected slot; removal selects the next entry; removing the last entry selects the new last; background not held follows the active slot; background held keeps the selection; replacement selects the start.
    - each adoption yields a new revision. Owns `queue-canonical-list` "A queue revision names one queue state".
- [ ] 6.2 Replace `player_tab`/`remote_player_tab` with `local_view`/`remote_view`, delete `App.queue_source` (read `view.source()`), and delete `src/app/state/player_tab.rs`.
  - Route adoptions through `adopt`: `handle_unified_queue_updated` uses `Background{held: queue_cursor_held_by_user()}`, or `Replacement` on a lineage change; `adopt_home_snapshot` uses `Background`; `await_queue_op` uses `OwnAnswer`; attach/connect/switch bootstraps use `Replacement`.
  - Delete `pending_queue_edit_cursor` and `pending_remote_move_cursor`.
  - Verify: the `mbv` gate passes, and `rg 'PlayerTab|pending_queue_edit_cursor|pending_remote_move_cursor|\.queue_source\b' src/app` is empty.
- [ ] 6.3 Projection: read the pending slot only from the adopted snapshot (`playback_target.rs` `pending_playback_slot`, `displayed_playback_state`), and delete `queue_row_playback_state`'s fence clause.
  - Test: a snapshot whose pending transition differs from the observed slot projects the pending slot with no progress. Owns `queue-canonical-list` "Queue projection is bounded presentation data".

## 7. Documentation

- [ ] 7.1 Add `docs/adr/0030-owner-process-is-the-only-local-player-owner.md`, covering design D1, D3, D4 and D6 with the rejected alternatives.
  - Add status notes pointing to 0030 to ADR 0006 (Refuse no longer means a Bare TUI), 0011, 0014 (the multi-connection model gains exclusive and shutting-down admission), 0015, 0016, 0017 (the Composed stage is retired), 0019 and 0029.
  - Verify: each amended ADR links to 0030.
- [ ] 7.2 Update `CONTEXT.md` per design D10: add **Owner process**, redefine **Stay-alive** as a lifetime policy, and make "Stay-alive process" and "Bare mode" _Avoid_ aliases. Also revise:
  - Bare mode, Stay-alive, Stay-alive process / Owner process, Client, Composed, Owner-held queue and source, Tray;
  - fix the Audiobookshelf eligibility line.
  - Update AGENTS.md's first paragraph.
  - Verify: `rg -n -i 'bare mode' CONTEXT.md AGENTS.md` shows only _Avoid_ entries.
- [ ] 7.3 Rewrite `docs/invariants/13-stay-alive-owner-is-the-queue.md` to keep only properties 2 (owner-minted lineage guards source updates) and 3 (run identity filters stale observations), which types don't enforce. Rename the file accordingly, and state that properties 1 and 4 are enforced by `QueueView` and `queue_op`.
  - Update invariant 01, point 3, to name `QueueView`.
  - Comment on GH #810 with 13's new status, and with 14 as "kept documented: a process rule types cannot express".

## 8. Integration verification

- [ ] 8.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All must pass.
- [ ] 8.2 Manual checks against the built binary. Record each result on this line when done.
  - Stay Alive off:
    - launching starts one local daemon, and quitting leaves no mbv process;
    - relaunching immediately after quitting starts cleanly;
    - a second terminal is refused with the pid and both remedies;
    - `kill -9` on the TUI makes the daemon exit;
    - there is no tray, and the audio window shows when configured;
    - switching to a direct remote stops local playback.
  - Stay Alive toggled off mid-session, then quitting, makes the daemon exit.
  - A Consume audio toggle mid-session applies.
  - Pressing `d` three times rapidly removes three entries, and `u` restores the last one in place.
  - During direct remote control, a Local-scope edit reaches the local daemon.
  - A fall-through video plays on the local daemon, including while controlling an audio-only Emby session.
  - The queue survives quitting and relaunching with Stay Alive off.
  - Packaged `mbvd` still admits two clients and survives both leaving.
- [ ] 8.3 Just before pushing, run `make check-code-file-lines` and split any governed file over 800 lines along responsibility seams.
