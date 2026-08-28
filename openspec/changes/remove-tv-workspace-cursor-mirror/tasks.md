## 1. Characterize the current mirror dependents (discovery, per D17)

- [x] 1.1 Write a characterization test proving `App::activate_selected_series(lib_idx)`'s
      only dependency on the mirrored cursor is `selected_series_item(lib_idx)`
      (`src/app/render/components/detail.rs:137`) — trace and record its two
      guards (`collection_type == "tvshows"`, `item.item_type == "Series"`) so
      D1's item-targeted method can reproduce them structurally instead of by
      re-reading `App`. Verify: test passes against current `main`/pre-change
      behavior.
- [x] 1.2 Write a characterization test proving `App::go_back(lib_idx)`
      (`src/app/actions_navigation.rs:217`) never reads
      `nav_stack.last().cursor` — only the popped level's `parent_id` and the
      parent level's cursor. Assert this by driving `go_back` with the
      mirrored cursor deliberately stale (matching the existing
      `tv_episode_activation_uses_component_cursors_and_cached_season_id`
      pattern at `shell_tv_workspace.rs:283`) and confirming identical
      `nav_stack` results with and without a prior mirror call. Verify:
      `rtk cargo nextest run -p mbv` passes; if the assertion fails, stop and
      report the discovery instead of proceeding to task 3.
- [x] 1.3 Write a characterization test proving `App::cycle_letter_pill(lib_idx, delta)`
      (`src/app/music_actions.rs:218`) never reads `.cursor` — only
      `letter_filter`. Same stale-cursor technique as 1.2. Verify: `rtk cargo
      nextest run -p mbv` passes; a failure is a blocking discovery result,
      not license to change `cycle_letter_pill`.

## 2. Add the item-targeted activation entry point (D1)

- [x] 2.1 Add `App::activate_selected_series_item(&mut self, item: &EmbyItem) -> bool`
      (or the name chosen at implementation time) in
      `src/app/input_browse_dispatch.rs`, sharing the `is_wide_tv_active()`
      branch (`enter_series_selection` / `open_series_selection_modal`) with
      the existing `activate_selected_series(lib_idx)`, factored so both
      call one shared branch-dispatch helper — do not duplicate the
      wide/narrow logic. Leave `activate_selected_series(lib_idx)` in place
      for `mouse_gestures.rs:166,234` (mouse remains accepted-broken, D16).
      Verify: `rtk cargo check -p mbv` passes; `mouse_gestures.rs` unchanged.
- [x] 2.2 Add `item: mbv_core::api::EmbyItem` to `ShellRequest::TvActivate` in
      `src/app/components/msg/shell.rs`. Verify: `rtk cargo check -p mbv`
      surfaces every call site needing updates (compiler-forced).
- [x] 2.3 In `TvWorkspaceComponent::handle_key`'s `Key::Enter if self.pane == Pane::Series`
      arm (`src/app/components/tv_workspace.rs:246`), resolve the selected
      item via the same lookup `selected_item_id()` performs and attach it
      to `ShellRequest::TvActivate { item }`. If no item is resolvable
      (defensive), do not emit the request. Verify: `rtk cargo nextest run
      -p mbv` — extend `typed_tv_requests_keep_component_cursor_authoritative`
      (`shell_tv_workspace.rs:200`) or add a sibling test asserting `TvActivate`
      carries the expected `EmbyItem`.
- [x] 2.4 Update `handle_tv_request`'s `ShellRequest::TvActivate` arm
      (`src/app/shell_tv_workspace.rs:32-34`) to call
      `self.app.activate_selected_series_item(&item)` instead of
      `activate_selected_series(lib_idx)`. Verify: `rtk cargo nextest run -p
      mbv`; `tv_episode_activation_uses_component_cursors_and_cached_season_id`
      (`shell_tv_workspace.rs:229`) still passes unmodified in its series
      Enter-key step.

## 3. Remove the mirror (D3)

- [x] 3.1 Remove both `self.mirror_tv_workspace_cursor(lib_idx)` calls in
      `handle_tv_request` (`shell_tv_workspace.rs:30`, `:42`). Verify: `rtk
      cargo check -p mbv` compiles clean (confirms nothing else still depends
      on the mirrored value).
- [x] 3.2 Delete `Model::mirror_tv_workspace_cursor` (`shell_tv_workspace.rs:59-76`).
      Verify: `rtk cargo check -p mbv` and `rtk cargo clippy --workspace
      --all-targets` — no dead-code warnings, no remaining callers
      (`rtk grep -n "mirror_tv_workspace_cursor" -- src/` returns nothing).

## 4. Extend the named characterization tests to lock the end state

- [x] 4.1 Extend `typed_tv_requests_keep_component_cursor_authoritative`
      (`shell_tv_workspace.rs:200`) to assert `App.libs[0].nav_stack[0].cursor`
      is unchanged by `TvMoveRows`, `TvMoveColumn`, `TvJumpCursor`, and
      `TvCycleLetterPill` (component cursor moves; `App`'s does not) — the
      inverse of the pre-change assertion at line 219, which expected the
      mirror to have written `App`'s cursor to 1. Verify: `rtk cargo nextest
      run -p mbv`.
- [x] 4.2 Extend `tv_episode_activation_uses_component_cursors_and_cached_season_id`
      (`shell_tv_workspace.rs:229`) to assert episode activation still
      succeeds with a stale `App` cursor and that `TvBack` after activation
      restores the parent series-list cursor via `go_back`'s own
      `parent_id` lookup, not via any mirror. Verify: `rtk cargo nextest run
      -p mbv`.
- [ ] 4.3 Add or extend an ast-grep rule / assertion (see `rtk ast-grep scan`
      fixtures under `rules/`) if one already forbids `App` back-projection
      of component-owned cursor state for migrated surfaces; otherwise skip —
      do not introduce a new architectural gate as part of this slice.
      Verify: `rtk ast-grep scan` stays green.

## 5. Full verification gate

- [ ] 5.1 `rtk cargo check -p mbv`
- [ ] 5.2 `rtk cargo nextest run`
- [ ] 5.3 `rtk cargo clippy --workspace --all-targets`
- [ ] 5.4 `rtk ast-grep scan`
- [ ] 5.5 `rtk cargo fmt` (accept its output) and confirm `rtk cargo fmt
      --check` is clean
