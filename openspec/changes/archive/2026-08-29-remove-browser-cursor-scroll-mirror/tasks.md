## 1. Cursor effect re-homing (D1)

- [x] 1.1 Add a typed `ShellRequest` (or fold into existing `Browser*Cursor*`
      variants) carrying the component-resolved cursor *index* rather than a
      delta, replacing `BrowserMoveRows`/`BrowserMoveColumn`/
      `BrowserJumpCursor` (`src/app/components/msg.rs`). Verify:
      `rtk cargo check -p mbv` compiles with the old delta variants removed.
- [x] 1.2 Update `BrowserComponent::handle_crossterm_key` and
      `browser_navigation.rs` (`move_rows`/`move_cursor_delta`/`jump_cursor`)
      to resolve the movement locally as today, then emit the new
      index-carrying request instead of a delta
      (`src/app/components/browser.rs`, `browser_navigation.rs`). Verify:
      existing component-level unit tests in `browser_component_tests.rs`
      still assert the correct resolved cursor for Up/Down/PageUp/PageDown/
      Home/End/column movement.
- [x] 1.3 Add `App::apply_lib_cursor_index(lib_idx, index)` (or equivalently
      named) in `src/app/lib_cursor_actions.rs` that writes
      `BrowseLevel.cursor = index` directly and then runs the same
      `save_default_library_position` / `mark_library_navigation` /
      `maybe_fetch_next_page` / `last_nav_at` tail `move_lib_cursor_inner`
      already runs, without recomputing the index from a delta. Route the
      new `ShellRequest` arm in `handle_browser_request`
      (`src/app/shell_browser.rs`) to it. Verify: `rtk cargo nextest run -p
      mbv emby_browser` passes, and a new/updated test in
      `shell_browser_tests.rs` asserts `save_default_library_position`/
      `mark_library_navigation`/`maybe_fetch_next_page` still fire exactly
      once per movement, matching pre-change behavior (parity evidence).
- [x] 1.4 Confirm `push_emby_browser_content` no longer needs to run
      immediately after a cursor-move `ShellRequest` purely to re-sync the
      component's cursor (it may still run for other content reasons at its
      existing choke points) — remove any redundant call added solely for
      cursor re-sync. Verify: `rtk cargo nextest run -p mbv` full suite
      green; manually trace (or add a test asserting) that the component's
      `self.cursor` after a movement equals the index it locally resolved,
      not a value re-read from `BrowseLevel`.
- [x] 1.5 Run `rtk cargo clippy --workspace --all-targets` and `rtk ast-grep
      scan`; both stay green with no new interactive-component-boundary
      violations.

## 2. Scroll ownership at navigation choke points (D2)

- [x] 2.1 Remove the per-draw `browser.scroll()` → `level.scroll`
      write-back in `render_emby_browser_component`
      (`src/app/shell_browser.rs:232-248`). Verify: `rtk cargo check -p mbv`
      compiles with the write-back deleted and `painted_scroll` no longer
      read there.
- [x] 2.2 In `select_item`'s folder-push branch and `go_back`'s pop branch
      (`src/app/actions_navigation.rs`), persist the outgoing level's live
      scroll from the component (via `BrowserComponent::scroll()` through
      the existing shell↔component bridge) into `BrowseLevel.scroll` before
      it stops being the visible level, alongside the existing
      `save_default_library_position` call. Verify: a new/updated test in
      `shell_browser_tests.rs` or `tests_library_position_restore.rs` drives
      scroll down, enters a folder, goes back, and asserts the parent
      level's scroll matches what was visible before descending
      (folder-in/folder-out position restoration preserved).
- [x] 2.3 Confirm `flush_library_position_now`
      (`src/app/library_position_state.rs`) still captures the
      last-known scroll at teardown/tab-switch-away even when the user
      never leaves the current level this session. Verify: existing
      `tests_library_position.rs`/`tests_library_position_refresh.rs`
      coverage for scroll persistence still passes; add a case if the
      choke-point change leaves a gap (session-only scroll change, no
      folder transition, then quit).
- [x] 2.4 Run `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
      --all-targets`, `rtk ast-grep scan` — all green.

## 3. Browser-wide input isolation (D18 step 2 / D3)

- [x] 3.1 Replace `set_wide_movies`'s `wide` input from
      `App::layout.main.is_wide_movies_active()` with a component-owned
      derivation from the component's own `BrowserKey` kind (Movies/HomeVideos)
      plus painted geometry at the existing
      `shared_hero_presentation`/`wide_library_panes` breakpoint
      (`src/app/components/browser.rs`, `src/app/shell_browser.rs`). Verify:
      `rtk cargo nextest run -p mbv emby_browser` preserves the current
      wide/narrow layout selection and navigation stride at the same terminal
      widths (parity, not improvement).
- [x] 3.2 Confirm the shared `movies_wide_right_area` producer/readers,
      `is_wide_movies_active()`, and legacy-body deletion remain untouched:
      their cross-surface geometry cleanup is scoped to issue #613's
      `remove-migrated-surface-underpaint` change. Verify: this unit's diff
      contains no removal of those shared symbols and no removal of
      `self.app.render(f)` in `shell_run.rs` (the permitted teardown scroll
      capture from unit 2 is not underpaint work).
- [x] 3.3 Update `docs/architecture/interactive-surface-ledger.md`'s
      Library/Browser row to record cursor/scroll mirror removal and
      Browser-wide input isolation without claiming shared-underpaint removal.
      Verify: the row matches the landed component ownership boundary and
      names #613 as the owner of remaining shared-underpaint cleanup.
- [x] 3.4 Run `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
      `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`, and
      `openspec validate remove-browser-cursor-scroll-mirror --strict`.
      Report known baseline diagnostics separately; no new violations.
