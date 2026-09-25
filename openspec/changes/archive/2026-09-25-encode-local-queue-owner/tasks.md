# Tasks

## 1. Source-of-truth types

- [x] 1.1 Add `src/app/state/queue_owner.rs` (register it in `state.rs`) containing `LocalQueueOwner { ThisProcess, StayAlive }`, `QueueEpoch(u64)` (derive `Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord`; `advance()` saturating), `QueueOrigin { ThisProcess { epoch }, StayAlive { epoch, lineage: mbv_core::ctrl::QueueLineage } }` with `epoch()`, and the `App` methods `local_queue_owner()` (match on `player_endpoint`, design D1), `queue_origin() -> Option<QueueOrigin>` and `origin_is_current(QueueOrigin) -> bool` (D3). Verify: `cargo check -p mbv` (unused-item warnings are expected until group 2).
- [x] 1.2 Rename `App::remote_queue_lineage: u64` to `queue_epoch: QueueEpoch` and `advance_remote_queue_lineage` to `advance_queue_epoch` (in `dispatch/session/command.rs`), and update every caller and test the compiler flags. Check every write to `player_endpoint` (connect/switch/teardown paths) and confirm an epoch advance follows it; add one where it is missing (design D5 relies on this). Verify: `cargo check -p mbv --tests` is clean, and `rg remote_queue_lineage src` returns nothing.
- [x] 1.3 In `state/types/playback.rs` `PlaylistMutation` and `state/types/events.rs` `SessionEvent`, replace each `queue_lineage: u64` with `origin: QueueOrigin` and delete `owner_queue_lineage` from `CreateAs` / `PlaylistCreateComplete`. Update `start_playlist_mutation`'s stale check to `!self.origin_is_current(*origin)`, and change its three thread closures to copy `origin`. Verify: `cargo check -p mbv`, with no `owner_queue_lineage` left in `src/`.

## 2. Dispatch through the owner

- [x] 2.1 Delete `stay_alive_owner_is_queue_authority()`. Rewrite each former caller as an exhaustive `match self.local_queue_owner()` with no wildcard: in `queue_scope.rs` `local_queue_is_owner_queue`, `stamp_queue_generation`, `set_queue_source_if_not_local_daemon` and `replace_playback_queue`, and in `dispatch/queue.rs` the PlayItems idle-load routing and the ClearQueue `player.clear_queue()`. Verify: `rg stay_alive_owner_is_queue_authority src` is empty, and `cargo nextest run -p mbv queue` passes.
- [x] 2.2 Persistence gate (D7): `save_queue_state`, `save_queue_state_no_clear` and `restore_queue_state` match on the owner. Delete `maybe_restore_queue_state` and point `shell/run.rs` and `tests/daemon_bootstrap.rs` at `restore_queue_state`. Verify: `cargo nextest run -p mbv daemon_bootstrap queue_state` passes.
- [x] 2.3 Request-time origin (D4): `save_playlist_to_emby`, `save_queue_as_playlist` (drop the `owner_queue_lineage` block) and `input/playlist_keys.rs` `do_overwrite_playlist` call `queue_origin()`. On `None` they flash the error "Stay-alive queue not available yet" and return without enqueueing. Verify: the new test in 3.2 passes.
- [x] 2.4 Add `apply_saved_playlist_source` (D5), and route both `PlaylistCreateComplete` and `PlaylistReplacementComplete` in `dispatch/run_loop/session.rs` through it. Keep the Save As success toast for `ThisProcess` only, and delete `reject_stay_alive_queue_source_update`. Verify: the existing `tests/queue/mutation/playlist_save.rs` Stay-alive tests pass unchanged apart from field shapes.
- [x] 2.5 Move the Stay-alive block in `dispatch/session/player_event.rs` `UnifiedQueueUpdated` into `adopt_owner_source(&unified)`, which matches on the owner (D6). Verify: `cargo nextest run -p mbv unified` and `playlist_save` pass.

## 3. Tests for changed behaviour

- [x] 3.1 In `tests/queue/mutation/playlist_save.rs`, using `local_daemon_save_as_fixture`: a Stay-alive overwrite (`do_overwrite_playlist`, then `PlaylistReplacementComplete` Ok) sends `CtrlCmd::UnifiedQueueSourceUpdate` with the fixture lineage and leaves `queue_dirty` true. A following owner snapshot with that source clears it. Verify: the test passes, and it fails when 2.4's Replace routing is reverted.
- [x] 3.2 Same file: a Stay-alive app with no owner snapshot (unified queue `None`) calling `save_queue_as_playlist` leaves `playlist_mutations` empty and flashes an error. Verify: the test passes.

## 4. Docs and gates

- [x] 4.1 Add a **Queue epoch** entry to CONTEXT.md next to **QueueLineage** (client-local counter advanced on each Client-side replace/clear/attach; fences async completions; never sent over ctrl; _Avoid_: remote queue lineage, client lineage). Update the "How the code maintains it today" section of `docs/invariants/13-stay-alive-owner-is-the-queue.md` to name `LocalQueueOwner`, `QueueOrigin` and `apply_saved_playlist_source`. Verify: `rg -n "stay_alive_owner_is_queue_authority|owner_queue_lineage" docs CONTEXT.md` is empty.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run -p mbv`, then commit. Verify: all three are green and the worktree is clean.
