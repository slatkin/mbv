## 1. Move `queue_scroll` into `QueueComponent` (D3)

- [x] 1.1 Delete `App::queue_scroll` (`app_struct.rs:184`) and its
      construction site; `QueueComponent` already has a `scroll` field —
      verify `rtk cargo check -p mbv` finds every removed reader/writer.
- [x] 1.2 Remove the `queue_scroll = 0` reset in `set_queue_scope`
      (`queue_scope.rs:295`); have `QueueComponent::set_content` reset its
      own `scroll` when the incoming `scope` differs from its current
      `scope` field, before the existing cursor-identity reconciliation
      runs. Verify with a `components/queue_component_tests.rs` case that
      switching scope resets scroll to 0.
- [x] 1.3 Stop `sync_queue` from reading `self.app.queue_scroll` and stop
      `set_content` from accepting a pushed scroll value; remove the
      `self.scroll.max(scroll).min(self.cursor)` clamp and replace it with
      a clamp against the component's own `scroll`/`cursor`/slot count only.
      Verify `rtk cargo nextest run -p mbv queue` passes.

## 2. Replace the `queue_cursor` argument-channel with explicit parameters (D2)

- [x] 2.1 Give `move_queue_item_by` (`queue_actions.rs:121`) an explicit
      `from: usize` parameter instead of reading `queue.queue_cursor`
      internally; update `move_queue_item_up`/`move_queue_item_down`
      call sites that still need the legacy (non-component) behavior to
      pass `queue.queue_cursor` explicitly, preserving current behavior.
      Verify `rtk cargo check -p mbv` and `tests_queue_reorder.rs` pass
      unmodified.
- [x] 2.2 In `shell_queue.rs`, change `handle_queue_request`'s
      `QueueRequest::Remove` and `QueueRequest::Move` arms to resolve the
      index via `select_queue_slot`'s existing lookup and pass it directly
      to `remove_from_queue`/`move_queue_item_by` instead of writing
      `queue.queue_cursor` and dispatching a synthetic Delete/Shift+arrow
      key. Verify the existing `shell_queue.rs` test
      (`queue_shell_mounts_and_routes_slot_cursor`) still passes, updated
      only if its assertion targets the now-removed write.
- [x] 2.3 Do the same for `QueueRequest::Play`: resolve the index/slot and
      invoke the `QueuePlayCursor`-equivalent effect directly with that
      target instead of relying on `queue_cursor` being pre-set by a prior
      write. Verify a component-routing test exercising Enter on a queue
      row still triggers playback of the selected slot.

## 3. Remove the cursor write-back in `select_queue_slot` (D1, D2 prerequisite complete)

- [x] 3.1 Delete `self.app.queue_for_scope_mut(scope).queue_cursor = index`
      from `select_queue_slot` (`shell_queue.rs:137`); keep
      `set_queue_scope`, `set_panel_focus`, and
      `mark_queue_cursor_user_active` (all still legitimate shell
      concerns). Verify `rtk cargo check -p mbv` — no remaining caller
      expects this write.
- [x] 3.2 Confirm `QueueRequest::Cursor` (plain navigation) no longer
      touches `PlayerTab::queue_cursor` at all. Verify with a new/updated
      `shell_queue.rs` test that arrowing in the component does not change
      `app.player_tab.queue_cursor`, only the component's own cursor.

## 4. Verify non-component writers and reconciliation are unaffected

- [x] 4.1 Verify `rtk cargo nextest run -p mbv` — full suite, including
      `tests_queue_reorder.rs`, `tests_queue_mutation.rs`, and
      `tests_remote_reconciliation*.rs` — passes with no assertion changes
      beyond access-path updates for moved fields (`queue_scroll`).
- [x] 4.2 Manually trace (code read, not new tests per `writing-tests`
      guidance) that `player_event.rs:271-272,319`,
      `run_loop_events.rs:130`, `run_loop_events_session.rs:133`,
      `library_position_state.rs:117-119`, and `actions.rs:376` are
      untouched by this change and still write `queue_cursor` exactly as
      before.

## 5. Final gates

- [ ] 5.1 `rtk cargo check -p mbv` clean.
- [ ] 5.2 `rtk cargo nextest run` clean (workspace).
- [ ] 5.3 `rtk cargo clippy --workspace --all-targets` clean.
- [ ] 5.4 `rtk ast-grep scan` clean (interactive-component-boundary rule:
      no component receives `App`/`PlayerProxy`/`Config`/credentials/mpsc;
      no new `App`→component back-projection of cursor/scroll introduced).
- [ ] 5.5 Confirm the "Done when" criteria in issue #617: queue cursor and
      scroll each have exactly one owner, named in `design.md` D1/D3; no
      `App`→component back-projection of a component-owned queue
      cursor/scroll remains.
