# Proposal

## Why

`App` holds three deferred queue mutations as `pub(in crate::app)` fields:
- `pending_queue_action`: waiting on the dirty-playlist save/discard answer,
  then on the save.
- `pending_queue_replacement`: the populated-queue gate.
- `pending_local_play`: the "play locally instead" prompt.

Two of them share the type `Option<PendingQueueAction>`. That shape caused
519583ea, where a replacement the user never confirmed was fired by an
unrelated playlist save. Ownership is held by comments
(`docs/invariants/10-deferred-queue-mutation-slot-ownership.md`), and the doc
itself is already wrong: it lists two slots and one reader for
`pending_queue_action`, but there are three slots and two readers.

The save boundary also still has the 519583ea shape. `PlaylistMutationComplete`
fires `pending_queue_action` on any successful save of the queue's playlist,
not only on the save the user's `[s]` answer started. Some paths close the
save/discard prompt without running its arm: the `clear:yes` notification,
playlist deletion, and a replacing modal. Those paths leave the deferral armed.
A later consume auto-save (`on_video_consumed` / `on_audio_consumed`) or quit
save of the same playlist then plays the action the user walked away from. A
failed save leaves it armed the same way. Issue #843, child of #810.

## What Changes

- The three slots move into one `QueueDeferrals` value with private fields
  and one type per slot. Each exposes only the transitions its owner uses, so
  depositing into another slot's field or reading it directly fails to
  compile.
- The save deferral becomes a two-state machine: waiting for the answer, then
  waiting for one specific save. `[s]` binds the payload to the mutation id of
  the save it started.
- **Behaviour:** only the completion of that specific save runs the deferred
  action. Another save of the same playlist (consume auto-save, quit save, a
  later manual save) never does.
- **Behaviour:** a failed bound save drops the deferred action instead of
  leaving it armed. A `[s]` that cannot start a save (no playlist or no origin,
  which already shows a warning) also drops it.
- `docs/invariants/10-deferred-queue-mutation-slot-ownership.md` is rewritten
  to cover only what the type doesn't enforce, or deleted.

The populated-queue gate and the local-play prompt keep today's behaviour.
Only their storage changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: adds the requirement that a queue replacement
  deferred behind the unsaved-playlist prompt runs only when that prompt's own
  save or discard answer completes.

## Impact

- New `src/app/state/queue_deferrals.rs`. The three fields in
  `src/app/state/app_struct.rs` and `construct.rs` are removed.
- Writers: `dispatch/queue/replacement.rs` (`replace_queue_or_prompt`,
  `request_queue_replacement`) and `dispatch/actions.rs` (local-play prompt).
- Readers: `input/confirm_keys.rs` (three arms),
  `dispatch/run_loop/session.rs` (`handle_playlist_mutation_complete`), and
  `dispatch/session/switch.rs` (`play_pending_local_play`).
- `dispatch/queue/playlist.rs`: `save_playlist_to_emby` returns the mutation
  id it enqueued.
- Tests: `input/confirm_keys/tests.rs`,
  `dispatch/actions/tests/replacement_gate.rs`, and `dispatch/actions/route_tests/`.
- Comments in `crates/mbv-ui-model/src/confirm.rs` that name the old fields.
