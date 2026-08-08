## 1. Implementation

- [x] 1.1 In `src/app/action.rs`'s `Command::QueuePlayCursor` handler, call
      `self.set_queue_scope(self.playback_target_queue_scope());` before the remote hand-off
      (both the tracked-occurrence reconciliation branch and the plain `submit_attached_sequence`
      branch), matching the pattern already used in `queue_actions.rs`'s
      `PendingQueueAction::PlayItems` handling.

## 2. Tests

- [x] 2.1 Extend or add a test alongside
      `queue_play_cursor_while_attached_to_session_hands_off_to_session` in
      `src/app/action_tests.rs` asserting queue scope switches to Remote when Direct remote
      control is active.
- [x] 2.2 Add/extend a test asserting queue scope stays Local when hand-off happens over a plain
      attached session with no `remote_player_tab`.

## 3. Verification

- [x] 3.1 `cargo test -p mbv-core` and the relevant `src/app` test module pass.
- [x] 3.2 `cargo clippy --workspace --all-targets` is clean.
