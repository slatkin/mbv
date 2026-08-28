## Why

The Emby generic/Movies/HomeVideos browser (`BrowserComponent`,
`src/app/components/browser.rs`) is marked `migrated` in
`docs/architecture/interactive-surface-ledger.md`, but it still carries a
two-way interaction-state mirror the completion gate forbids (D14): a
per-frame scroll write-back inside the draw closure
(`render_emby_browser_component`, `src/app/shell_browser.rs:232-248`) and a
cursor-move round trip where the component predicts its own cursor, the shell
independently recomputes the same movement against `App`, and the next
content push overwrites the component's prediction with the App's result
(`shell_browser.rs:92-106` ↔ `components/browser.rs` local movement ↔
`lib_cursor_actions.rs`). It is slice 4 of 4 for issue #611 and the largest,
because `App::move_lib_cursor_rows`/`move_lib_cursor`/`jump_lib_cursor` are
not setters — they also drive pagination (`maybe_fetch_next_page`), position
persistence (`save_default_library_position`), and navigation-idle image
fetching (`mark_library_navigation`/`last_nav_at`) — and `BrowseLevel.cursor`/
`.scroll` have roughly 37 non-test production readers across `nav_stack`, so
the field cannot simply move to the component.

## What Changes

- Replace the delta-based cursor requests (`BrowserMoveRows`,
  `BrowserMoveColumn`, `BrowserJumpCursor`) with a single typed request
  carrying the component-resolved *index*, and add an `App` method that
  applies that index directly (writing `BrowseLevel.cursor` and running the
  existing `save_default_library_position` /  `mark_library_navigation` /
  `maybe_fetch_next_page` / `last_nav_at` tail) instead of independently
  recomputing the movement. Removes the duplicate-arithmetic parity risk
  between the component's local move and the App's recompute.
- Remove the per-draw `browser.scroll()` → `level.scroll` write-back in
  `render_emby_browser_component`. Persist `BrowseLevel.scroll` only at the
  navigation choke points where the visible level actually changes
  (`select_item`'s folder push, `go_back`'s pop, tab switch away), preserving
  folder-in/folder-out position restoration without a per-frame paint-coupled
  write.
- Detach the component's wide-Movies/HomeVideos layout signal from the
  legacy wide renderer's `movies_wide_right_area` output (D18 step 2): derive
  "wide" from the component's own `BrowserKey` kind plus its painted
  geometry width instead of `App::layout.main.is_wide_movies_active()`, then
  delete the now-unread Emby-specific legacy wide-renderer functions and
  `movies_wide_right_area` production for this surface. Does **not** remove
  the shared `self.app.render(f)` legacy-underpaint call in `shell_run.rs` —
  that remains scoped to issue #613 (`resolve-migrated-surface-correctness`),
  which is correctly sequenced after this change because other still-legacy
  surfaces (TV, Music) depend on the same base frame. See `design.md` for the
  full ordering resolution.
- No behavior change to the four typed selected-item effects
  (`BrowserActivate`/`BrowserPlay`/`BrowserEnqueue`/`BrowserToggleWatched`),
  context menu, shuffle, refresh/rescan, back navigation, or letter-pill
  cycling — those already resolve their target from the component or the
  shell's own tab state and carry no cursor mirror.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `interactive-component-framework`: the "Interactive components own only
  presentation authority" requirement's completion bar (no per-frame or
  two-way interaction-state synchronization) now has a concrete
  no-longer-exempt scenario for the Emby generic/Movies/HomeVideos browser —
  previously the migration ledger marked this surface `migrated` while its
  cursor/scroll mirror remained.

## Impact

- `src/app/shell_browser.rs`, `src/app/components/browser.rs`,
  `src/app/components/browser_navigation.rs`, `src/app/components/msg.rs`,
  `src/app/lib_cursor_actions.rs`, `src/app/actions_navigation.rs`,
  `src/app/shell_browser_tests.rs`.
- No `BrowseLevel` field is deleted (see scout handoff §5 for the ~37
  unrelated readers that block it) and no protocol/persistence format
  changes; `crate::config::LibraryPosition` and its on-disk/shared-document
  shape are unaffected.
- `docs/architecture/interactive-surface-ledger.md`'s Library/Browser row
  updates to record the mirror's removal.
- Read-only scout handoff (D17) already recorded at
  `openspec/handoffs/scout-remove-browser-cursor-scroll-mirror.md`.
