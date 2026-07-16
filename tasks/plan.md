# Implementation Plan: Selectable Artist Headers in Custom Music Library View

## Overview

Issue #210 should make only the artist headers in the custom Power View music library selectable. This is the combined music view enabled when the configured music library uses the custom grouped view through `config.toml`; it is not a generic grouped-list feature and does not apply to queue headers or other library headers. A selected artist header is an internal UI target: Enter does nothing, but context/bulk actions can play, enqueue, or shuffle the visible albums/tracks under that artist.

## Architecture Decisions

- Represent the focused row in the custom music-group view as either an artist header or an album row, rather than overloading the existing album cursor alone. The existing cursor indexes `BrowseLevel.items`, so it cannot identify a header without an additional selection marker.
- Reuse and extend the existing grouped-album display plan in `src/app/render/power/album.rs` (`build_grouped_album_display_plan`). Do not create a parallel grouping model in `music.rs`; the same plan must drive render, paging, keyboard movement, mouse hit testing, and artist-member resolution.
- Gate selectable headers with an explicit custom-view/selectable mode passed only from `render_power_music_group_view` / `is_music_group_view(lib_idx)`. Generic `render_power_list` grouped album headers must remain non-selectable.
- Replace or supplement `LayoutPower.left_row_map: Vec<Option<usize>>` with a typed target such as `PowerLeftRowTarget::{Album(usize), ArtistHeader { first_album_idx, first_album_id, artist }}`. `None` remains reserved for filler/non-target rows.
- Store artist-header selection as a revalidated identity, not a raw label alone. Recommended identity: `{ first_album_id, artist_label }`, clearing the selection if the rebuilt display plan cannot match it exactly after loading, cache updates, search, group switch, or navigation changes.
- Define "visible" precisely: the target set is the album rows present under the selected artist header in the current custom music-group display plan after current loading/filtering. It is not a whole-library artist query, and it is not limited to the rows currently inside the viewport.
- Bulk artist-header actions need a single multi-album execution helper that resolves all playable media for the selected header, preserves display album order then each album's existing track order, and performs one play/enqueue/shuffle mutation. Do not loop through `do_enqueue_folder` because that would repeatedly flash status, sync queues, and partially mutate on failures.
- If any recursive album fetch fails during header bulk resolution, abort the whole bulk action before mutating the queue/playback state and show one error. This keeps the operation atomic from the UI's perspective.

## Task List

### Phase 1: Model the Selectable Row

- [ ] Task 1: Add custom music artist-header selection state
- [ ] Task 2: Extend the existing grouped-album display plan

### Checkpoint: Foundation

- [ ] Focused artist header can be represented without changing the stored album cursor semantics.
- [ ] Existing selected album behavior still works when an album row is selected.
- [ ] Unit tests cover display-plan construction and selected-row mapping for both selectable and non-selectable modes.

### Phase 2: Render and Navigate Headers

- [ ] Task 3: Render selectable artist headers with focused styling
- [ ] Task 4: Route keyboard movement through display rows in the custom music-group view
- [ ] Task 5: Update typed mouse hit testing so clicking an artist header selects it

### Checkpoint: Selection Behavior

- [ ] Up/Down can land on artist headers and album rows.
- [ ] Clicks on artist headers focus/select them.
- [ ] Enter on an artist header is consumed as a no-op.
- [ ] Changing music group, leaving the view, or changing album selection clears invalid header selection.
- [ ] Generic grouped album headers remain non-selectable.

### Phase 3: Bulk Actions

- [ ] Task 6: Resolve the selected artist header to its visible display-plan album members
- [ ] Task 7: Wire context-menu and bulk play/enqueue/shuffle actions for artist headers

### Checkpoint: Bulk Behavior

- [ ] Context menu on an artist header shows Play All, Shuffle, Add to Queue.
- [ ] Bulk actions affect only display-plan albums under that artist header in the current custom music-group view.
- [ ] Album-row context menu and direct row actions behave as before.

### Phase 4: Verification and PR

- [ ] Task 8: Add focused tests and run targeted suites
- [ ] Task 9: Run GitNexus change detection, full relevant tests, commit, push, and open PR

## Detailed Tasks

## Required Impact Analysis Targets

Before editing code, run GitNexus impact analysis for the symbols that will be touched. At minimum, analyze:

- `build_grouped_album_display_plan`
- `render_power_grouped_album_rows`
- `page_power_grouped_album_cursor`
- `move_lib_cursor`
- `jump_lib_cursor`
- `handle_mouse_panels`
- `open_context_menu`
- `handle_lib_key`
- `execute_context_action`
- `enqueue_selected`
- `current_lib_item`
- `select`
- `shuffle_play`
- `play_folder` if it is reused or bypassed by new header bulk helpers
- `select_music_group`
- `switch_music_group`
- any new/modified helper that maps the Power-left selected artist header to a bulk action target
- every consumer of `LayoutPower.left_row_map`

Warn before proceeding if any impact result is HIGH or CRITICAL.

## Task 1: Add custom music artist-header selection state

**Description:** Add a small internal state field to the per-library state that can distinguish "album row selected" from "artist header selected" while in the custom music-group view.

**Acceptance criteria:**
- [ ] State can represent an artist header by revalidated identity, preferably `{ first_album_id, artist_label }`.
- [ ] State is scoped to the library and cleared when changing music group, changing selected album, entering track-focus mode, searching, breadcrumb truncating, or leaving the applicable nav level.
- [ ] Rebuilding the grouped-album display plan clears the header selection if the identity no longer matches exactly, including after album-artist cache regrouping.
- [ ] Existing album cursor remains the source of truth for album-row selection.

**Verification:**
- [ ] Unit test proves switching groups clears header selection.
- [ ] Unit test proves cache regrouping clears or revalidates header selection safely.
- [ ] Existing track-focus tests still pass.

**Dependencies:** None.

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/actions.rs`
- `src/app/input.rs`

**Estimated scope:** Medium.

## Task 2: Extend the existing grouped-album display plan

**Description:** Extend `build_grouped_album_display_plan` in `src/app/render/power/album.rs` so the existing grouped album display sequence can represent selectable artist headers only when the caller opts in from the custom music-group view.

**Acceptance criteria:**
- [ ] The display plan returns enough data to map display rows to album indices or artist-header identities.
- [ ] The display plan can resolve the selected display row from current library state.
- [ ] The display plan is the single source for render, paging, keyboard movement, mouse hit testing, and selected artist member resolution.
- [ ] Non-custom callers keep artist headers non-selectable.

**Verification:**
- [ ] Unit tests cover two artists with multiple albums and unknown artist fallback.
- [ ] Tests cover selected display-row position for both album and artist header selection.
- [ ] Test proves generic grouped album headers still map to non-target rows.

**Dependencies:** Task 1.

**Files likely touched:**
- `src/app/render/power/music.rs`
- `src/app/render/power/album.rs`
- possibly `src/app/render/power/mod.rs`

**Estimated scope:** Medium.

## Task 3: Render selectable artist headers with focused styling

**Description:** Update `render_power_music_group_view` / grouped album rendering so selected artist headers visibly receive the same focus affordance category as selectable rows while still reading as headers.

**Acceptance criteria:**
- [ ] Header row renders as selected when focused and selected.
- [ ] Album rows render unchanged when selected.
- [ ] Generic letter headers and queue headers do not change.
- [ ] The shared grouped renderer takes an explicit selectable-header mode so only `render_power_music_group_view` enables selection.

**Verification:**
- [ ] Ratatui snapshot/string tests assert selected artist header styling/marker.
- [ ] Existing power list render tests pass.

**Dependencies:** Task 2.

**Files likely touched:**
- `src/app/render/power/music.rs`
- `src/app/render/power/list.rs` if the current grouped renderer must be parameterized

**Estimated scope:** Medium.

## Task 4: Route keyboard movement through display rows

**Description:** In Power View left-panel input handling, when `is_music_group_view(lib_idx)` is true, make Up/Down move through the custom music-group display rows rather than raw album indices.

**Acceptance criteria:**
- [ ] Up/Down can land on artist headers.
- [ ] Moving from a header to an album updates the existing album cursor.
- [ ] Header-selected Enter is checked before the existing album-track focus block, consumed, and does not mutate `album_track_focus`.
- [ ] Existing `[` / `]` music group switching remains unchanged.

**Verification:**
- [ ] Unit tests for Up/Down across header/album/header boundaries.
- [ ] Unit test for Enter on header no-op.

**Dependencies:** Tasks 1-2.

**Files likely touched:**
- `src/app/input.rs`
- `src/app/actions.rs`

**Estimated scope:** Medium.

## Task 5: Update typed mouse hit testing

**Description:** Extend Power View left-panel row mapping with a typed target so custom music artist headers can be click targets without overloading album indices or making generic headers selectable.

**Acceptance criteria:**
- [ ] `LayoutPower` exposes typed row targets for power-left rows, with album rows and artist headers represented distinctly.
- [ ] Clicking an artist header selects/focuses that header.
- [ ] Clicking an album row still selects that album and clears header selection.
- [ ] Clicking non-row filler space remains harmless.
- [ ] Existing `left_row_map` callers are updated or bridged without changing their semantics.

**Verification:**
- [ ] Unit test for mouse click on a header row.
- [ ] Existing mouse click tests pass.

**Dependencies:** Tasks 1-3.

**Files likely touched:**
- `src/app/layout.rs`
- `src/app/input.rs`
- `src/app/render/power/music.rs`

**Estimated scope:** Medium.

## Task 6: Resolve selected artist header members

**Description:** Add an action helper that returns the display-plan album items under the selected artist header in the current custom music-group view.

**Acceptance criteria:**
- [ ] Only returns members for the selected artist header in `is_music_group_view`.
- [ ] Returns current display-plan albums under that header, respecting current loaded/filter state.
- [ ] Returns `None`/empty for ordinary album selection or non-custom views.
- [ ] Partially loaded libraries only target currently loaded display-plan albums.

**Verification:**
- [ ] Unit tests for artist with one album and multiple albums.
- [ ] Unit test proves another artist's albums are excluded.

**Dependencies:** Task 2.

**Files likely touched:**
- `src/app/actions.rs`
- `src/app/render/power/music.rs` or helper module chosen in Task 2

**Estimated scope:** Small.

## Task 7: Wire play/enqueue/shuffle/context-menu actions

**Description:** Connect selected artist header targets into the existing context-sensitive action paths.

**Acceptance criteria:**
- [ ] Context menu on a selected artist header offers Play All, Shuffle, Add to Queue.
- [ ] Add explicit header-aware `ContextAction` variants or action payloads; do not fake the header as a `MediaItem`.
- [ ] `open_context_menu` detects selected custom artist headers before deriving `current_lib_item()` so it does not target the stale album cursor.
- [ ] The direct enqueue shortcut enqueues all playable media from the display-plan albums under the header.
- [ ] The direct Play All shortcut targets selected artist-header members before falling back to `current_lib_item()` / album-folder behavior.
- [ ] The direct Shuffle shortcut targets selected artist-header members before falling back to `shuffle_play()` / album-folder behavior.
- [ ] Shuffle and play use the same resolved header member set, with one status message and one queue/playback mutation.
- [ ] Recursive fetch failure aborts before mutation and reports one error.
- [ ] Album-row actions are unchanged.

**Verification:**
- [ ] Unit tests assert context-menu entries/actions for selected artist header.
- [ ] Unit tests assert album-row context menu still uses album folder actions.
- [ ] Unit test asserts right-click/context menu on a header does not expose stale album actions.

**Dependencies:** Task 6.

**Files likely touched:**
- `src/app/mod.rs`
- `src/app/input.rs`
- `src/app/actions.rs`

**Estimated scope:** Medium.

## Task 8: Add focused tests and run targeted suites

**Description:** Add tests close to the modified seams and run targeted Rust tests regularly.

**Acceptance criteria:**
- [ ] Tests cover render selection, keyboard movement, mouse selection, and context action exposure.
- [ ] Tests are scoped to issue #210 behavior and do not encode unrelated UI details.
- [ ] Regression tests cover non-custom grouped headers, queue headers, Enter no-op, stale cursor prevention, header invalidation, and partial loaded-member resolution.

**Verification:**
- [ ] `cargo test power_music_group`
- [ ] `cargo test selectable_artist`
- [ ] `cargo test input::tests::`

**Dependencies:** Tasks 1-7.

**Files likely touched:**
- `src/app/render/power/mod.rs`
- `src/app/render/power/music.rs`
- `src/app/input.rs`
- `src/app/actions.rs`

**Estimated scope:** Medium.

## Task 9: Change detection, commit, push, PR

**Description:** Run required repository verification, then commit and open the PR from the isolated worktree branch.

**Acceptance criteria:**
- [ ] GitNexus `detect_changes({scope: "compare", base_ref: "main", worktree: "/tmp/mbv-issue-210"})` reports expected scope.
- [ ] Full relevant test suite passes or any failure is documented with cause.
- [ ] Branch is pushed and PR references issue #210.

**Verification:**
- [ ] `cargo test`
- [ ] `git status --short`
- [ ] `gh pr create --repo slatkin/mbv --fill`

**Dependencies:** Tasks 1-8.

**Files likely touched:** None beyond prior tasks.

**Estimated scope:** Small.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Header selection fights the existing album cursor | High | Keep album cursor unchanged and add a narrowly scoped header-selection marker. |
| Bulk actions accidentally target all albums by artist across the library | High | Resolve only contiguous visible display rows under the selected header. |
| Generic grouped headers become selectable by accident | Medium | Gate every new path on `is_music_group_view(lib_idx)`. |
| Artist resolution triggers async cache updates during input/action handling | Medium | Reuse the same artist labels already resolved during render where possible, or isolate helper behavior and test cache-miss fallback. |
| Context menu model cannot represent multi-folder artist actions cleanly | Medium | Add specific context actions for artist-header album members; do not coerce headers into `MediaItem`. |
| Looping existing folder helpers causes repeated queue mutations | High | Implement one multi-album bulk helper that resolves first, then mutates once. |

## Open Questions

- None. Current decisions: preserve display album order then existing per-album track ordering; swallow Enter on selected artist headers as a no-op; target current display-plan albums only.
