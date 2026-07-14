# Task Checklist: Issue #199 Sticky Library Position

## Task 1: Add Library Position State I/O

**Description:** Add serializable state types and atomic load/save helpers for `library_position_state.json`, following the existing `queue_state.json` pattern.

**Acceptance criteria:**
- [x] State stores positions by library ID and view scope (`default`, `power`).
- [x] Path segments store identity/context only, not full item lists.
- [x] Corrupt/missing state files fail harmlessly with no user-facing error.

**Verification:**
- [x] Focused config tests cover save/load, missing file, and parse failure.
- [x] `cargo test config`

**Dependencies:** None

**Files likely touched:**
- `crates/mbv-core/src/config.rs`
- `src/config.rs`

**Estimated scope:** Small: 1-2 files

## Task 2: Add Snapshot and Restore Helpers

**Description:** Add app-level helpers that convert current library state to/from saved library position state without changing navigation behavior yet.

**Acceptance criteria:**
- [x] Snapshot captures drill-down path and focused item ID with cursor fallback.
- [x] Restore clamps fallback indices and derives scroll from focused item visibility.
- [x] Search/detail/album track-selection state is excluded.

**Verification:**
- [x] Focused app tests for snapshot shape and restore fallback helper behavior.
- [ ] `cargo test app::`

**Dependencies:** Task 1

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/actions.rs`

**Estimated scope:** Medium: 2-4 files

## Task 3: Save Default-View Position

**Description:** Persist default library-view position when logical position changes, including drill-down/back, cursor moves, page/home/end, group/pill changes, and mouse selection.

**Acceptance criteria:**
- [x] Default-view navigation writes only the active library's `default` position.
- [x] Holding movement keys does not write on render frames.
- [x] Hidden library state is not deleted.

**Verification:**
- [x] Focused tests for default-view cursor/drill-down snapshot writes.
- [x] Manual review of write call sites confirms no render-frame writes.

**Dependencies:** Task 2

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/input.rs`

**Estimated scope:** Medium: 2-4 files

## Task 4: Restore Default-View Position Lazily

**Description:** Restore a saved default-view library position only when that library tab becomes active, using Emby fetches to rebuild the path before rendering root content.

**Acceptance criteria:**
- [x] Saved default position restores across restart.
- [x] Stale paths restore the deepest valid prefix and nearest sensible fallback.
- [x] Stable stale fallback rewrites the state file after successful restore.

**Verification:**
- [x] Tests for restart restore, stale missing item, stale missing parent, and no root-first flash state.
- [x] Focused app tests around library activation.

**Dependencies:** Tasks 1-3

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/mod.rs`
- `src/app/render/library/mod.rs`

**Estimated scope:** Medium: 3-5 files

## Task 5: Persist Power View Panel Focus

**Description:** Persist Power View queue-side vs library-side focus in `prefs.json`, separate from library position.

**Acceptance criteria:**
- [x] Queue-side/library-side focus survives restart.
- [x] Restoring panel focus does not change saved library position.
- [x] User-facing terminology remains queue side/library side; do not document `PowerFocus::Left` as domain language.

**Verification:**
- [x] Focused prefs tests for both focus values.
- [x] Existing Power View width/tab prefs tests still pass.

**Dependencies:** None

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/input.rs`

**Estimated scope:** Small: 1-2 files

## Task 6: Save and Restore Power View Library Position

**Description:** Persist and lazily restore library-side position in Power View under the `power` scope, independent from the default view.

**Acceptance criteria:**
- [x] Power View library position restores across restart.
- [x] Home/CW tab selection still uses existing `power_left_tab` prefs, but Home/CW internal dashboard position is not persisted.
- [x] Default-view position is not used as bootstrap or fallback for Power View.

**Verification:**
- [x] Tests for Power View restore, Home/CW exclusion, and default/power isolation.
- [x] Existing Power View render/navigation tests still pass.

**Dependencies:** Tasks 1, 2, 5

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/mod.rs`
- `src/app/render/power/mod.rs`

**Estimated scope:** Medium: 3-5 files

## Task 7: Isolate Positions During In-Session View Changes

**Description:** Ensure switching between default library tabs and Power View activates the appropriate view-scoped position immediately, not only after restart.

**Acceptance criteria:**
- [ ] Moving in default view does not move Power View's saved/current position for that library.
- [ ] Moving in Power View does not move default view's saved/current position for that library.
- [ ] Switching views activates each scope's own last position without visible cross-scope jumps.

**Verification:**
- [ ] Focused tests that switch views mid-session and assert independent cursors/paths.
- [ ] Manual inspection of shared `nav_stack` handling, if still shared internally.

**Dependencies:** Tasks 3, 4, 6

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/input.rs`
- `src/app/render/power/mod.rs`

**Estimated scope:** Medium: 3-5 files

## Task 8: Clear Active-View Position on Refresh/Rescan Request

**Description:** Treat manual refresh/rescan as an immediate reset boundary for only the active library/view position.

**Acceptance criteria:**
- [ ] Refresh/rescan clears the active library's active view scope immediately on request.
- [ ] Failed refresh/rescan does not restore the old sticky position.
- [ ] The other view scope for the same library remains intact.

**Verification:**
- [ ] Tests for default refresh clear, Power View refresh clear, failure path, and other-view preservation.
- [ ] Existing refresh tests still pass.

**Dependencies:** Tasks 3, 6

**Files likely touched:**
- `src/app/actions.rs`

**Estimated scope:** Small: 1 file

## Task 9: Acceptance Regression and Documentation Check

**Description:** Prove the end-to-end behavior and update docs only if implementation changes domain vocabulary or records a hard-to-reverse trade-off.

**Acceptance criteria:**
- [ ] Tests cover default/Power isolation, restart restore, lazy no-root-jump behavior, refresh/rescan active-view-only clearing, hidden-library retention, stale fallback rewrite, and panel-focus persistence.
- [ ] `CONTEXT.md` still matches the shipped behavior.
- [ ] No unrelated cleanup or adjacent refactors are included.

**Verification:**
- [ ] `cargo test`
- [ ] GitNexus `detect_changes({scope: "all", repo: "mbv"})`
- [ ] `git diff --check`

**Dependencies:** Tasks 1-8

**Files likely touched:**
- Existing test modules near changed behavior
- `CONTEXT.md` only if vocabulary changes

**Estimated scope:** Medium: 2-4 files
