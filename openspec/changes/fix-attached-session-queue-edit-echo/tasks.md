## 1. Gate predicate

- [x] 1.1 Add one shared helper on `App` (e.g. `queue_edits_reach_owner(&self) -> bool` returning `matches!(self.playback_target(), PlaybackTarget::Local)`) in `src/app/queue_scope.rs` or `queue_actions.rs`; verify with `cargo check -p mbv`.

## 2. Apply the gate to the three edit kinds

- [x] 2.1 `remove_from_queue` (`src/app/queue_actions.rs`): add the predicate to `sent_queue_remove`; with an attached session (or cast) the edit is purely local — verify with a unit test asserting no `QueueRemoveSlot`/ctrl command is emitted while `connected_session_id` is set.
- [x] 2.2 `apply_queue_move_by_slot` (`src/app/queue_actions.rs`): same gate on the owner send; unit test asserting no move command is emitted with an attached session.
- [x] 2.3 `sync_playback_queue_items_after_append` (`src/app/queue_scope.rs`): same gate so appends are not sent to the daemon while a session/cast is the playback target; unit test asserting no append command and success (no rollback flash) with an attached session.
- [x] 2.4 Direct-remote regression: unit test that a Remote-scope remove/move still dispatches the slot-addressed command (gate must not swallow direct-remote edits).

## 3. Regression coverage for the reported bug

- [x] 3.1 Unit test reproducing the report shape: daemon holds playlist A slots, client queue holds playlist B, session attached; remove B's first slot → client queue still shows B, `queue_source` still B, no owner command sent (would previously adopt the echoed A snapshot via `UnifiedQueueUpdated`).

## 4. Verification

- [x] 4.1 `cargo nextest run -p mbv` green; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean.
- [x] 4.2 Manual check (user, stay-alive + attached Emby session): load playlist B over A, play a mid-video through the session, remove/reorder/append in the queue — queue keeps showing B, no "slot not found" toast.
