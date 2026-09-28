# Tasks

Groups 1–2 land as ONE commit: group 1 removes fields that group 2's callers
still read. No new lint suppression.

Final gate (end of group 2):
- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo nextest run -p mbv`

## 1. Deferral type (design D1)

- [x] 1.1 Create `src/app/state/queue_deferrals.rs` with `QueueDeferrals`,
  `SaveDeferral`, `GatedReplacement`, `LocalPlay` and exactly the transitions
  in the design D1 table. Fields stay private to the module, and
  `QueueDeferrals` derives `Default`. Add `#[cfg(test)]` read-only accessors
  `is_save_deferred`, `has_gated_replacement`, `has_local_play`. Register the
  module in `src/app/state.rs` and re-export `QueueDeferrals` the way
  `PendingQueueAction` is re-exported. Verify: `cargo check -p mbv` reports
  errors only at the old field sites (`rg -n "pending_queue_action|pending_queue_replacement|pending_local_play" src --type rust`).
- [x] 1.2 In `src/app/state/app_struct.rs`, replace the three fields and their
  doc comments with one `queue_deferrals: QueueDeferrals` field. Its doc
  comment names the module as the only place slot ownership lives. In
  `construct.rs`, initialize it with `QueueDeferrals::default()`. Verify: the
  three field names no longer appear in either file.

## 2. Callers (design D1, D2)

- [x] 2.1 Writers:
  - `replace_queue_or_prompt` → `defer_for_save_answer`.
  - `request_queue_replacement` → `hold_gated_replacement`. Keep its doc
    comment's reasoning and drop the field names.
  - `dispatch/actions.rs:99` → `hold_local_play`.

  Make `save_playlist_to_emby` (`dispatch/queue/playlist.rs`) return
  `Option<u64>`: the enqueued `mutation_id`, or `None` on each early return.
  Verify: `cargo check -p mbv` shows no errors in those three files.
- [x] 2.2 Readers in `input/confirm_keys.rs`:
  - `confirm_discard_or_save_dirty_playlist`: `save_answer_is_play`,
    `[s]` → `bind_to_save(self.save_playlist_to_emby())`, `[d]` →
    `take_on_discard`, and Esc/`[c]` → `cancel_save_answer`.
  - `confirm_replace_populated_queue`: `take_confirmed_replacement` /
    `cancel_gated_replacement`.
  - `confirm_play_locally`: `cancel_local_play`.

  Also:
  - `play_pending_local_play` (`dispatch/session/switch.rs`) →
    `take_confirmed_local_play`.
  - `handle_playlist_mutation_complete` (`dispatch/run_loop/session.rs`): call
    `take_on_save_complete(mutation_id)` first, then run the action, dismiss
    the sidebar and focus the Queue only when the existing
    `succeeded && origin_is_current && queue_playlist_id == playlist_id` guard
    holds (design D2).
  - Update the ownership comments in `input/confirm_keys.rs` and
    `crates/mbv-ui-model/src/confirm.rs` to name the `QueueDeferrals`
    transitions instead of fields.

  Verify: `rg -n "pending_queue_action|pending_queue_replacement|pending_local_play" src crates --type rust -g '!**/tests/**' -g '!*tests.rs'`
  is empty.
- [x] 2.3 Tests:
  - Move the existing assertions in `input/confirm_keys/tests.rs`,
    `dispatch/actions/tests/replacement_gate.rs` and
    `dispatch/actions/route_tests/{playback,library_routing}.rs` to the
    `#[cfg(test)]` accessors. Keep what they assert.
    `an_in_flight_save_completion_never_executes_an_unconfirmed_gated_replacement`
    already owns the gate-vs-save scenario and the Save-answer happy path.
  - Add to `input/confirm_keys/tests.rs` a named two-row `#[case]` table,
    `save_deferral_ignores_a_save_it_is_not_bound_to` (regression, issue #843).
    It covers a prompt dismissed without an answer and a different
    `mutation_id` completing for the same playlist, and asserts in both cases
    that the queue is unchanged after a successful `PlaylistMutationComplete`.
  - Add `failed_bound_save_drops_the_deferred_replacement` (regression, issue
    #843). It asserts that a successful completion of a later save does not
    run the replacement.

  Then run the final gate and commit groups 1–2 as one commit referencing
  #843. Verify: the gate is green.

## 3. Docs

- [x] 3.1 Rewrite `docs/invariants/10-deferred-queue-mutation-slot-ownership.md`:
  - Point at `src/app/state/queue_deferrals.rs` as where ownership is
    enforced.
  - Record the one remaining unenforced rule: a transition is named for its
    caller, but visibility cannot stop another `App` method calling it.
  - Drop the stale line references and the "two slots" framing.

  Sync the spec delta into `openspec/specs/unified-playback-queue/spec.md`
  and tick invariant 10 on #810. Verify:
  `openspec validate encode-queue-deferral-ownership` passes, and the doc
  names all three deferrals.
