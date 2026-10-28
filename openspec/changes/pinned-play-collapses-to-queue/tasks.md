# Tasks

## 1. Collapse-only pinned view step

- [x] 1.1 In `src/app/dispatch/action.rs`, move the request part of `pinned_view_toggle` into a private `request_pinned_view(width)` and make `pinned_view_toggle` call it with `pinned_width.toggled()` (design D2). `request_pinned_view` carries the one-in-flight guard (the `pinned_request_pending.is_some()` early return) that sits in `pinned_view_toggle` today. Verify: the existing `src/app/tests/pinned_view_toggle.rs` tests pass unchanged.
- [x] 1.2 Add `collapse_pinned_view_after_play`: when `pinned_width == Expanded`, call `request_pinned_view(Collapsed)`; otherwise do nothing. The pending check is the one inside `request_pinned_view` (design D2, D3). Its doc comment states that every user tail that starts playback calls it after the accepted replace. Verify with tests in `src/app/tests/pinned_view_toggle.rs` for the contract "the step sends one collapse request only from the expanded width": at `Expanded` it records one `Width(Collapsed)` request and the drained outcome shows `QueueOnly` with queue focus; at `Collapsed` it records no request. (The pending guard is already owned by `a_second_x_press_while_a_request_is_pending_sends_nothing` through the shared step, so add no test for it.)

## 2. Collapse after a user play

- [x] 2.1 Call `collapse_pinned_view_after_play` after the `NotApplied` early return in `play_items_routed`, `play_series_continuation`, `play_single_item`, `submit_queue_item` (only when `start_playback`), and `start_pending_queue_playback` (design D1, D4). Do not call it from `replace_queue_on_owner` or the websocket path. Verify: `cargo check -p mbv`.
- [x] 2.2 Add tests for the contract "an accepted user play collapses the expanded pinned view; a non-applied play and an enqueue do not" at the `submit_queue_item` tail, with the existing answered-op stub that the queue tests use: an `Applied` play at `Expanded` records one `Width(Collapsed)` request; a `NotApplied` play records none; `start_playback == false` records none. Put them in the existing queue or pinned test module that already owns `submit_queue_item` behavior. Do not repeat them for each tail. Verify: `cargo nextest run -p mbv` passes.

## 3. Websocket Play leaves the UI alone

- [x] 3.1 Remove `set_panel_focus(PanelFocus::Queue)` from `handle_ws_play` in `src/app/dispatch/session/ws_event.rs`, and drop any import that this makes unused (design D5). Keep its toasts and its replace.
- [x] 3.2 Extend the existing `play_replaces_queue_with_remote_source` test (do not add a new test) for the `emby-remote-play` contract: start with library focus and `pinned_width == Expanded`, then assert that the library keeps focus, `pinned_width` stays `Expanded`, and no pinned request is recorded. Verify: `cargo nextest run -p mbv` passes.

## 4. Domain model

- [ ] 4.1 In `CONTEXT.md` *Pinned panel width*, replace "The running panel switches between the two values with `x` only." with a sentence that names all three triggers: `x`, the F2 Panel width rows, and the collapse after a user play from the expanded width. Keep the rest of the entry. Verify: the entry names the three triggers and no other text in it changes.

## 5. Gates

- [ ] 5.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. Verify: all three are green.
- [ ] 5.2 Just before the push, run `make check-code-file-lines`. Verify: no file is over 800 lines (`src/app/dispatch/action.rs` is at 643 now).

## Workflow follow-up

- The user does the live test in the pinned panel: play, play all, and shuffle from the expanded view collapse it; enqueue does not; a remote Emby Play does not move focus.
- Sync the deltas into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
