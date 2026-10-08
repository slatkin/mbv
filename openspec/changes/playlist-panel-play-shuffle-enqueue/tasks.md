# Tasks

## 1. Remove `autostart` from `PendingQueueAction` (design D4, D3)

- [x] 1.1 Delete the `autostart` field from `PendingQueueAction::PlayItems` (`src/app/state/playback.rs`). Delete the `if !autostart` branch and the parameter from `execute_pending_play_items` (`src/app/dispatch/queue/pending_playback.rs`), and make the remote-disconnected guard unconditional. Remove the field from every builder and from the `match` in `src/app/dispatch/queue/replacement.rs`. Leave `load_idle_queue_on_owner` and `send_idle_queue_load` in place (their other callers are in `src/app/dispatch/actions.rs`). Verify with `cargo check -p mbv`.
- [x] 1.2 Add `ReplacementExecutor::PlaylistsSidebar` (`src/app/state/playback.rs`). Keep `Pending` for `play_grouped_track` (`src/app/dispatch/navigation.rs`).
  - In `run_replacement`, the `PlaylistsSidebar` arm runs `execute_queue_replacement`. Then, when no overlay was raised, it closes the Playlists sidebar and focuses the Queue, with no source check.
  - The `Pending` arm only runs `execute_queue_replacement`. Delete its `source: Playlist` `matches!` and its dismiss/focus step.
  - Switch the two F4 call sites (`src/app/shell/playlists.rs` open view, `load_and_play_playlist`) to `PlaylistsSidebar`. Task 2.3/2.4 rewrite them later.
  - Update the doc comments on the enum and on `run_replacement`.
  
  Verify with `cargo check -p mbv`.
- [x] 1.3 Update the tests that built `autostart`:
  - `src/app/dispatch/actions/tests/album_artist_playback.rs`: drop the `autostart` binding and its assert.
  - `src/app/input/confirm_keys/tests.rs`: drop the field. The playlist half of `empty_queue_runs_a_shuffle_and_a_playlist_load_without_a_modal` now asserts that a replace is sent, not an idle load. Rename the test to match.
  - `src/app/tests/queue/queue_op.rs` `idle_queue_load_does_not_block_input_and_leaves_the_view_until_the_result`: call `load_idle_queue_on_owner` directly instead of the deleted load-only `PlayItems`, and reword its comment around the populate-only load.
  
  Verify with `cargo nextest run -p mbv`.

## 2. Panel actions: message, component keys, shell handling (design D1, D2)

- [x] 2.1 In `crates/mbv-ui-msg/src/shell.rs`, replace `ShellRequest::PlaylistsActivate { open, index }` with `PlaylistsAction { open, index, action: MusicTreeAction }`. Reword the doc comment on `MusicTreeAction` (`intents.rs`) so it covers both the Music tree and the Playlists panel. Update the two arms in `src/app/shell/messages.rs` that list it. Verify with `cargo check --workspace`.
- [x] 2.2 In `crates/mbv-components/src/playlists.rs`:
  - Enter → `Play`, `s` → `Shuffle`, `a` → `Enqueue`, all with no modifiers.
  - Each sends `PlaylistsAction` with the current view's `open` and cursor `index`. In the list view, nothing is sent when the cursor is past the end.
  - The mouse double-click sends `Play`.
  - Replace the old activation test with one named `#[case]` table that covers the three keys in each view.
  
  Verify with `cargo nextest run -p mbv-components playlists`.
- [x] 2.3 In `src/app/dispatch/library/load.rs`, replace `load_and_play_playlist` with the resolver `playlist_playable_items(playlist_id) -> Option<Vec<EmbyItem>>`. It keeps the current fetch, the folder filter, and the unavailable/error/empty/nothing-playable flashes. Verify with `cargo check -p mbv`.
- [x] 2.4 In `src/app/shell/playlists.rs`, handle `PlaylistsAction` according to the design D2 table. Play and Shuffle go through `request_queue_replacement(..., ReplacementExecutor::PlaylistsSidebar)`. Shuffle uses `QueueSource::Shuffle` and `rand` shuffle. Enqueue sends one `append_on_owner(viewed_queue_scope(), items)` and leaves the sidebar open. Add two shell tests in the existing `src/app/shell/playlists.rs` tests module:
  - Shuffle replaces with `QueueSource::Shuffle` (contract: "Saved playlist order is untouched").
  - Open-view Enqueue sends one append holding only the cursor item and no replace (contract: "Enqueue one item from an open playlist").
  
  Verify with `cargo nextest run -p mbv playlists`.

## 3. Panel presentation (design D5)

- [x] 3.1 In `crates/mbv-render/src/components/playlists.rs`, make the hint bars read `[↵]play [s]shuffle [a]add [→]browse [n]rename [d]delete [r]refresh [Esc]close` in the list view and `[↵]play [s]shuffle [a]add [←]back [Esc]close` in the open view. Verify with `cargo check -p mbv-render`.
- [x] 3.2 Replace `render_open_playlist_row` and the open view's variable-height scroll and scrollbar code with the list-row painter, generalised over its leading and trailing spans:
  - List rows: bold title + muted `(count)`.
  - Open rows: muted `NN. ` + title.
  - One shared bg/fg rule (selected → `SELECTED_ROW_BG`/`SELECTED_ROW_FG`, odd absolute index → `PLAYLIST_STRIPE_BG`), one-line rows with truncated names, and the same scroll clamp and `render_sidebar_scrollbar` call.
  - `geometry.open_rows` is still filled, one row per item.
  
  No geometry-assertion tests. Verify with `cargo nextest run -p mbv-render` and `cargo clippy --workspace --all-targets -- -D warnings`.

## 4. Integration

- [x] 4.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All must pass.
- [ ] 4.2 The user tests live in F4: Enter and `s` in both views, `a` in both views, the populated-queue confirmation, and the striped single-line open rows. Mark this done only on the user's confirmation.

## Workflow follow-up

- Run `make check-code-file-lines` before pushing.
- Archive the change (sync the specs) after the user signs off.
