# Implementation Plan: Issue #183 Recursive Grouped Music Album Search

## Overview
Implement recursive album-only fuzzy search for grouped music libraries in both the standard library view and Power View. The feature should use a versioned, persistent album-path index so startup can show cached results immediately while a background refresh rebuilds the index. Activating a result must navigate the relevant library UI to the album's real grouped path, and stale cached entries must be suppressed instead of producing visible activation errors.

## Architecture Decisions
- Add a dedicated album-path index rather than stretching `LibSearch.items/results` alone. Current library search stores `results` as indices into the visible level's item list, but recursive grouped search needs synthetic results with album ID, path labels, and group path.
- Add a typed search result model before activation work. Recursive album results must not flow through `current_lib_item()`, enqueue, context-menu, watched-toggle, or play paths as if they were visible-level `MediaItem` indices.
- Persist the index as versioned JSON under the existing cache/config state helpers. Store only server/user identity, library ID, relevant `music_levels`, album ID, album title, group path segments, and enough fetch context to resolve the path; do not persist tracks.
- Keep index refresh asynchronous. Load the last saved matching index during app construction, then spawn a refresh for grouped music libraries after launch or first use; never block startup on a full recursive walk.
- Use a dedicated album-index refresh event with library ID and generation checks. Do not reuse `SearchItemsLoaded`, whose semantics are tied to the current visible browse level.
- Reuse existing library navigation primitives where possible. Activation should rebuild or select the real group path in `LibraryTab.nav_stack` instead of creating a special search-only browse mode.
- Treat standard library view and Power View as two activation contexts over the same index. Search data can be shared, but activation must use a scoped path that does not switch Power View activation through the standard library tab.
- Add a resolver before activation that returns resolved, stale, or broader failure. If a cached album ID or group path no longer matches current server data, suppress the result and let refresh replace it instead of opportunistically navigating to a different path.
- Gate the feature narrowly to grouped music layouts with a group-like level followed by an album terminal level, initially `["group", "album"]`. Album-only or non-grouped music libraries keep existing search behavior.
- Update `CONTEXT.md` if implementation introduces new domain terms for the album-path index or recursive album search behavior. Add an ADR only if the worker makes a hard-to-reverse storage or navigation decision beyond this plan.

## Task List

### Phase 1: Index Foundation
- [ ] Task 1: Add versioned album-path index state and config I/O.
- [ ] Task 2: Add recursive grouped music album index builder.
- [ ] Task 3: Add typed recursive album search results and stale resolver.

### Checkpoint: Foundation
- [ ] Focused config/index tests pass.
- [ ] No user-facing search behavior changes yet.

### Phase 2: Refresh Lifecycle
- [ ] Task 4: Load saved index at startup and refresh it in the background.
- [ ] Task 5: Deliver index refresh events to open searches and persist fresh results.

### Checkpoint: Refresh
- [ ] Startup does not synchronously fetch the full grouped library.
- [ ] Open search results update after a refresh event.

### Phase 3: Search UI and Activation
- [ ] Task 6: Extend library search rendering to use album-index results for grouped music libraries.
- [ ] Task 7: Navigate standard library activation to the indexed album path.
- [ ] Task 8: Navigate Power View activation to the indexed album path.

### Checkpoint: Core Behavior
- [ ] Standard library search finds albums outside the current group and opens the selected album.
- [ ] Power View search finds albums outside the current group and opens the selected album.
- [ ] Normal grouped browsing remains unchanged when search is closed.

### Phase 4: Stale Handling and Regression
- [ ] Task 9: Add acceptance coverage and documentation updates.

### Checkpoint: Complete
- [ ] All issue #183 acceptance criteria are covered.
- [ ] `cargo test` passes, or any remaining failure is documented with the exact failing command/output.
- [ ] GitNexus `detect_changes({scope: "all", repo: "mbv"})` shows only expected affected symbols/flows before commit.
- [ ] `git diff --check` passes.

## Risks and Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| `LibSearch`, search key handling, and activation paths are shared by standard view and Power View. | High | Keep the existing index-based search mode intact for non-grouped libraries; add an explicit recursive album result mode only for grouped music libraries. |
| `current_lib_item()` and generic selection paths are high-blast-radius seams. | High | Introduce typed recursive search results before activation; explicitly test non-Enter actions while recursive search is open. |
| Recursive grouped index building may require many Emby requests. | High | Start from the configured `music_levels`, fetch only group/album levels, persist the last good index, and refresh asynchronously. Avoid track fetches entirely. |
| Activation may accidentally mutate normal browsing state when search closes. | High | Route activation through existing navigation helpers and add tests that closing search without activation leaves current grouped browse state unchanged. |
| Cached entries can become stale between startup and refresh. | Medium | Validate album/group path before navigation, remove stale entries from the current result set, and refresh the persisted index after successful rebuild. |
| Power View and default view have isolated saved positions from issue #199. | Medium | Do not bootstrap one view from the other. Activation should update only the currently active view context and must not call a path that switches to the standard library tab. |
| Background refresh events can race with query/library changes. | Medium | Include library ID and generation in refresh state; recompute only still-open recursive searches for the same library/generation. |
| JSON index size or write time may grow for large libraries. | Low | Use readable JSON first as requested; only optimize format if tests or measurement show a real bottleneck. |

## Open Questions
- None for the initial implementation. The issue explicitly scopes results to albums only, permits cached stale results if suppressed, and prefers JSON unless measurement proves it inadequate.

## Implementation Notes
- Before editing existing functions/classes/methods, run GitNexus impact analysis per `AGENTS.md` and report direct callers, affected processes, and risk. Warn before editing any HIGH or CRITICAL risk symbol.
- Required impact-analysis targets before editing include `current_lib_item`, `select`, `LibSearch`, `handle_lib_search_key`, `update_lib_search`, `render_power_list`, `LibEvent::NavigateTo`, and any library-position scope helpers touched by activation.
- Likely seams to inspect first: `LibSearch` and `LibraryTab` in `src/app/mod.rs`, `/` search opening in `src/app/input.rs`, `spawn_search_items_load` and `LibEvent` handling in `src/app/actions.rs`, Power View library rendering/activation in `src/app/render/power/`, and config persistence in `crates/mbv-core/src/config.rs`.
- Prefer behavior tests at existing app seams over tests that assert private helper internals, but add focused serialization tests for the versioned index state.
