## Why

`PlayerTab::queue_cursor` currently serves three unrelated roles behind one
`usize`, and `App::queue_scroll` serves a fourth. `sync_queue` pushes both
into `QueueComponent` every frame, and `select_queue_slot` (`shell_queue.rs:151`)
writes the component's own navigation resolution straight back into
`queue_cursor` — a closed shell↔component mirror of the kind #611 (parent
issue) is retiring across Settings/Services, TV workspace, and the Emby
browser. Queue was deferred (#617) because, unlike those surfaces,
`queue_cursor` also has legitimate non-component writers (playback advance,
remote-session reconciliation, queue-edit follow, restore/select-on-enqueue)
that must keep writing it. Removing the mirror requires first separating
"what the user is looking at" from "where playback/remote/edits say the
queue should be," which is not mechanical — it is answered here and recorded
so the mirror can be removed without regressing any of those non-component
writers.

## What Changes

- Record the cursor-ownership decision in `design.md`: `queue_cursor` splits
  into a component-owned **user cursor** (already partially present as
  `QueueComponent::cursor`) and a shell-owned **follow position** on
  `PlayerTab` (kept as `queue_cursor`, re-scoped to only the follow role).
- Move `App::queue_scroll` wholesale into `QueueComponent` as owned
  interaction state; delete the shell field and its per-scope reset
  (`queue_scope.rs:295`).
- Stop `select_queue_slot` from writing the component-resolved index back
  into `PlayerTab::queue_cursor`. Where that write was standing in as an
  implicit argument to shell-owned edit effects (`remove_from_queue`,
  `move_queue_item_by`, `QueuePlayCursor`), pass the resolved index/slot_id
  as an explicit parameter instead of writing-then-reading a shared field.
- Convert the remaining `sync_queue` cursor/scroll read-back into a targeted
  push of the follow position at its validated writer seams (playback
  advance, remote reconciliation, queue-edit follow, restore/select-on-
  enqueue), matching the shell→component push pattern used elsewhere
  (`shell_home.rs:251` is the exemplar D17 already established).
- No change to the canonical queue, remote reconciliation semantics, or
  playback behaviour. `input_queue_keys.rs`'s raw-key cursor navigation,
  `mouse_gestures.rs`, `context_menu_actions.rs`, and the legacy
  `render/screens/queue.rs` renderer are untouched — they are covered by the
  separate in-flight `remove-legacy-keyboard-endpoint` change and by D16's
  accepted-broken mouse scope, and adding no new callers of them is enough
  for this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None — this is an internal ownership/architecture refactor with no
spec-level behavior change. The canonical queue, remote reconciliation
semantics (`openspec/specs/remote-playback-reconciliation/`), and playback
behaviour are explicitly required to stay unchanged, and existing coverage
(`tests_queue_reorder.rs`, `tests_queue_mutation.rs`,
`tests_remote_reconciliation*.rs`) pins that. `.openspec.yaml` sets
`skip_specs: true` accordingly.

## Impact

- `src/app/shell_queue.rs` — `select_queue_slot`, `sync_queue`.
- `src/app/components/queue.rs` — gains scroll ownership; `set_content`
  drops the scroll merge/clamp against a pushed value.
- `src/app/app_struct.rs` — removes `queue_scroll`.
- `src/app/queue_scope.rs` — removes the `queue_scroll = 0` reset.
- `src/app/queue_actions.rs`, `src/app/queue_actions_playlist_mutation.rs` —
  `move_queue_item_by` (and any sibling relying on the write-before-call
  pattern) takes an explicit index instead of reading `queue_cursor` as an
  implicit argument.
- Non-component writers left in place and unchanged in behaviour:
  `player_event.rs:272,319`, `run_loop_events.rs:130`,
  `run_loop_events_session.rs:133`, `library_position_state.rs:117-119`,
  `actions.rs:376`, `types_player_tab.rs:141,162`,
  `App::pending_queue_edit_cursor` / `pending_remote_move_cursor`.
- Tests: `tests_queue_reorder.rs`, `tests_queue_mutation.rs`,
  `tests_remote_reconciliation*.rs`, `shell_queue.rs`'s own test, and
  `components/queue_component_tests.rs` must keep passing unmodified in
  observable outcome (assertions on cursor/scroll values may need updated
  access paths where ownership moved).
