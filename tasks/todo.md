# Task Checklist: Issue #183 Recursive Grouped Music Album Search

## Task 1: Add Album-Path Index State I/O

**Description:** Add versioned, serializable album-path index types plus load/save helpers for a JSON cache file. The cache stores recursive album search metadata for grouped music libraries without storing tracks.

**Acceptance criteria:**
- [ ] State is versioned and defaults safely when the file is missing, corrupt, or from an unsupported version.
- [ ] Entries are scoped by server/user identity, library ID, and relevant `music_levels`.
- [ ] Entries store album ID, album display title, and group path context, but no track lists.
- [ ] Save uses the existing atomic-write pattern used by other JSON state files.

**Verification:**
- [ ] Focused config tests cover save/load, missing file, parse failure, unsupported version, changed server/user, changed library, and changed `music_levels`.
- [ ] `cargo test -p mbv-core config`

**Dependencies:** None

**Files likely touched:**
- `crates/mbv-core/src/config.rs`
- `src/config.rs`

**Estimated scope:** Small: 1-2 files

## Task 2: Build Recursive Grouped Music Album Index

**Description:** Add a builder that records every album path for a selected grouped music library. First prove whether one recursive `MusicAlbum` query returns enough parent/path context; if not, walk the configured group/album hierarchy. It should fetch groups and albums only, not tracks.

**Acceptance criteria:**
- [ ] Builder gates recursive album indexing to grouped music layouts, initially `["group", "album"]`.
- [ ] Album-only, empty, non-music, and unsupported deeper `music_levels` keep existing search behavior.
- [ ] Builder records albums across all groups, including albums outside the currently visible group.
- [ ] Builder avoids track fetches and preserves enough path context to navigate later.

**Verification:**
- [ ] Focused tests cover `["group", "album"]`, album-only/non-grouped no-op behavior, unsupported levels, empty groups, and same-named albums in different groups.
- [ ] `cargo test app::`

**Dependencies:** Task 1

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/mod.rs`
- `crates/mbv-core/src/api.rs` if a narrow fetch helper is needed

**Estimated scope:** Medium: 3-5 files

## Task 3: Add Typed Search Results and Stale Resolver

**Description:** Replace the implicit “search results are indices into visible items” assumption with a typed library search result model that can represent existing visible-item results and recursive album-index results. Add a resolver that classifies recursive album results as resolved, stale, or broadly failed before activation.

**Acceptance criteria:**
- [ ] Recursive album results cannot be consumed by generic `current_lib_item()`, enqueue, context-menu, watched-toggle, or play paths as ordinary visible-level items.
- [ ] Resolver validates album ID and cached group path; path mismatch is stale and suppressed until refresh replaces it.
- [ ] Resolver returns a distinct broader failure for genuine indexing/fetch failures, separate from stale cached entries.

**Verification:**
- [ ] Tests cover typed visible-item results, typed recursive album results, stale album ID, stale group path, and broad resolver failure.
- [ ] Tests assert non-Enter actions such as enqueue/context/watched toggles are ignored or intentionally handled while recursive album search is open.
- [ ] `cargo test app::input`

**Dependencies:** Tasks 1-2

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/input.rs`
- `src/app/actions.rs`

**Estimated scope:** Medium: 3-5 files

## Task 4: Load Saved Index and Refresh in Background

**Description:** Load the saved album-path index during app initialization and trigger a background refresh for grouped music libraries without blocking startup.

**Acceptance criteria:**
- [ ] App construction can use the saved index immediately.
- [ ] Background refresh is spawned after launch or first grouped music search without blocking startup.
- [ ] Refresh failures leave the last saved index usable and produce no visible search error unless there is a broader indexing failure.
- [ ] Refresh state includes library ID and generation so stale refresh completions can be ignored.

**Verification:**
- [ ] Tests prove startup uses saved index without requiring a refresh to complete.
- [ ] Tests prove refresh failure does not clear usable cached results.
- [ ] Tests prove an old refresh generation does not overwrite newer index state.
- [ ] `cargo test app::`

**Dependencies:** Tasks 1-3

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/actions.rs`

**Estimated scope:** Medium: 2-4 files

## Task 5: Deliver Refresh Events to Open Searches

**Description:** Add a dedicated library event for completed/failed album-index refreshes, persist the fresh index, and update any open grouped music search results live. Do not reuse `SearchItemsLoaded`, which belongs to current-level search loading.

**Acceptance criteria:**
- [ ] Fresh index data is saved after a successful refresh.
- [ ] If a matching grouped music recursive search is still open, its results are recomputed against the fresh index and current query.
- [ ] Closed searches, changed-library searches, changed-query generations, and non-recursive searches ignore stale refresh completions.
- [ ] Non-grouped library searches keep their existing full-level search behavior.

**Verification:**
- [ ] Tests cover open search result recomputation after refresh.
- [ ] Tests cover refresh races after closing search, switching libraries, and changing query.
- [ ] Tests cover non-grouped search unaffected by refresh events.
- [ ] `cargo test app::`

**Dependencies:** Task 4

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/mod.rs`

**Estimated scope:** Medium: 2-4 files

## Task 6: Use Recursive Album Results in Grouped Music Search

**Description:** Extend library search rendering so grouped music libraries can search album-index entries and render path-aware album results, while preserving the current item-index search mode for other libraries.

**Acceptance criteria:**
- [ ] Standard view and Power View grouped music searches return albums only.
- [ ] Result labels include group path context such as `Artist / Album`.
- [ ] Existing visible-level fuzzy search remains unchanged for non-grouped libraries.

**Verification:**
- [ ] Tests cover recursive album matches outside the current group, same album names in different groups, and non-grouped fallback behavior.
- [ ] Rendering/search tests assert path context is visible.
- [ ] `cargo test app::input`

**Dependencies:** Tasks 1-5

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/input.rs`
- `src/app/render/library/`
- `src/app/render/power/`

**Estimated scope:** Medium: 3-5 files

## Task 7: Activate Standard Library Recursive Album Results

**Description:** When a recursive album result is selected in standard library search, resolve its group path and navigate the standard library browse stack to the real album.

**Acceptance criteria:**
- [ ] Activation navigates to the selected album under its real group path.
- [ ] Closing search without activation leaves normal grouped browsing unchanged.
- [ ] Stale cached entries that cannot resolve are removed from visible results without a user-facing activation error.

**Verification:**
- [ ] Tests cover successful navigation, no-op close behavior, and stale-result suppression.
- [ ] `cargo test app::input`

**Dependencies:** Tasks 3, 6

**Files likely touched:**
- `src/app/input.rs`
- `src/app/actions.rs`
- `src/app/mod.rs`

**Estimated scope:** Medium: 3-5 files

## Task 8: Activate Power View Recursive Album Results

**Description:** Apply recursive album activation behavior to Power View's right-hand music library panel through a Power-scoped activation path. It must not call a generic path that switches `tab_idx` to the standard library tab.

**Acceptance criteria:**
- [ ] Power View activation navigates the right-hand music panel/library context to the selected album.
- [ ] Default view and Power View library positions remain isolated.
- [ ] Default view on group A, Power View on group B, and Power activation to group C leaves default nav/position at A while Power moves to C.
- [ ] Power View non-music and non-grouped music search behavior remains unchanged.

**Verification:**
- [ ] Tests cover Power View navigation, hard default/power activation isolation, and non-grouped fallback.
- [ ] Existing Power View render/navigation tests still pass.
- [ ] `cargo test app::render::power`

**Dependencies:** Tasks 3, 6, 7

**Files likely touched:**
- `src/app/input.rs`
- `src/app/actions.rs`
- `src/app/render/power/mod.rs`
- `src/app/render/power/music.rs`

**Estimated scope:** Medium: 3-5 files

## Task 9: Acceptance Regression and Documentation

**Description:** Add end-to-end regression coverage for issue #183 and update domain documentation if new recursive search/index terminology becomes part of the implementation.

**Acceptance criteria:**
- [ ] Tests cover standard view recursive search, Power View recursive search, path-context labels in both renderers, activation, background refresh live update, stale suppression, and unchanged normal grouped browsing.
- [ ] Tests cover non-Enter behavior while recursive album search is open so synthetic results do not leak into unrelated actions.
- [ ] `CONTEXT.md` is updated if the album-path index or recursive grouped album search needs domain vocabulary.
- [ ] No unrelated cleanup or adjacent refactors are included.

**Verification:**
- [ ] `cargo test`
- [ ] GitNexus `detect_changes({scope: "all", repo: "mbv"})`
- [ ] `git diff --check`

**Dependencies:** Tasks 1-8

**Files likely touched:**
- Existing test modules near changed behavior
- `CONTEXT.md` if terminology changes

**Estimated scope:** Medium: 2-4 files
