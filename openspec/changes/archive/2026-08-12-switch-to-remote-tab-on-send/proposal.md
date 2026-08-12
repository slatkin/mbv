## Why

Playing an item from the queue panel while under Direct remote control already switches the
queue-scope tab to Remote so the sent item is visible where it landed (`queue_actions.rs`'s
`PendingQueueAction::PlayItems` handling). `Command::QueuePlayCursor`'s own dispatch in
`action.rs`, used when the cursor is on a queue row and Enter is pressed, bypasses that path for
the two cases where items are handed off to a remote session (tracked-occurrence reconciliation
and the plain attached-sequence hand-off) and never switches queue scope. Under Direct remote
control this leaves the user staring at the now-stale Local tab after their selection was sent to
the Remote queue (see #198).

## What Changes

- `Command::QueuePlayCursor`'s remote hand-off branches (tracked-occurrence reconciliation and
  plain attached-sequence) now switch queue scope to match `playback_target_queue_scope()` before
  sending, the same way `PendingQueueAction::PlayItems` already does.
- Because `set_queue_scope` no-ops back to Local whenever `has_direct_remote_queue()` is false,
  this only visibly switches tabs for Direct remote control; a plain attached session (no
  `remote_player_tab`) is unaffected and stays on Local, matching existing queue-scope semantics.

## Capabilities

### New Capabilities
- `queue-scope-remote-handoff`: Sending the queue's current-cursor item to a remote session via
  `QueuePlayCursor` switches the visible queue scope to the destination queue when Direct remote
  control is active.

### Modified Capabilities

## Impact

- `src/app/action.rs` (`Command::QueuePlayCursor` handler)
- Test coverage: `src/app/action_tests.rs` (queue-scope assertion on the attached-session hand-off
  path), possibly `src/app/tests_queue_scope.rs`
