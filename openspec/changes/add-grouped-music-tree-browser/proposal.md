## Why

Grouped Music presents a settled artist hierarchy as a flat list whose artist labels cannot receive focus, collapse, or act on their albums. A mergeable one-screen integration of `tui-treelistview` will test whether a shallow tree can make that hierarchy directly navigable while meeting mbv's ownership, input, theme-role, and one-painter contracts. The tree is its own presentation rather than a copy of the canonical flat lists — its selection extent, zebra rhythm, and scrollbar focus behavior are tree-specific — so the dependency is rejected only if the tree's core behavior or ownership proves impossible through its supported interfaces, never because its visuals differ from canonical lists.

## What Changes

- **BREAKING (Grouped Music interaction):** replace the Grouped Music album `MediaList` with a two-level tree whose focusable artist roots expand or collapse and whose album leaves retain existing album activation.
- Add artist-root play, enqueue, shuffle, and context behavior, resolving roots to their visible album descendants in settled display order. (No multi-selection UI ships in this PoC — user decision 2026-09-20; modified-click toggles, Visual mode, and range selection are descoped and revisited after PoC acceptance.)
- Replace Grouped Music's flat full-corpus Inline Search presentation with an in-place, 300 ms debounced fuzzy tree filter over the current settled tree. Matching preserves settled order, retains artist ancestors, force-expands matching paths, and restores pre-filter selection and expansion on dismissal. (Deferred from this PoC — user decision 2026-09-20; the pre-change Inline Search behavior for Grouped Music remains until this is revisited after PoC acceptance.)
- Add an artist Hero with artwork and summary metadata plus an artist Workspace containing the in-scope tracks grouped under non-selectable album headings. Album Hero and Workspace behavior remains unchanged. In non-Wide geometry, selected artist or album detail uses the existing Library Hero overlay rather than retaining Grouped Music's album-only inline-row Hero, because a focusable artist root needs the same complete Workspace-bearing detail surface as its Wide presentation.
- Retain stable Emby artist identity in music data, with deterministic fallback identity for albums whose Service payload has no artist identity.
- Integrate the tree through `MusicContent`, the existing Library panel slots, central keyboard policy, and latest-frame mouse geometry. `TreeListViewState` is the sole browser cursor, expansion, scroll, and mark owner; no second router, painter, or shell mirror is added.
- Hold the tree to a coherent tree-specific presentation instead of parity with the canonical flat lists: semantic theme roles, readable artist/album hierarchy with visible expansion state, a clear focused-node treatment inside the browser rectangle the panel supplies, metadata when space permits, clipping or marquee within bounds, ordinary Music row semantics, and one painter, verified at the existing Wide and smallest supported non-Wide Library-panel fixture widths. Differences from canonical lists in selection extent, zebra rhythm, and scrollbar focus gating are intentional and permitted. Rough visual edges are expected to be worked out during the end-of-implementation manual PoC evaluation rather than blocking earlier rows. A parallel bespoke tree renderer is out of scope.
- Keep every other destination, the Music track Workspace, and the artist track Workspace on canonical flat `MediaList` controls.

## Capabilities

### New Capabilities

- `grouped-music-tree-browser`: Defines the shallow artist/album tree, stable identities, tree navigation and actions, current-frame hit testing, refresh continuity, and dependency acceptance gate. (Its in-place fuzzy filtering requirements are deferred from the PoC per user decision 2026-09-20.)

### Modified Capabilities

- `canonical-media-lists`: Removes the Grouped Music album browser from canonical flat-list ownership while retaining canonical `MediaList` ownership for Music track Workspaces.
- `stable-music-library-grouping`: Changes settled artist labels into stable, focusable tree roots while preserving atomic publication and refresh continuity.
- `music-library-hero`: Adds artist-focused Hero and grouped-track Workspace content alongside the existing album-focused detail behavior and removes Grouped Music's album-only narrow inline-row Hero.
- `library-hero-overlay`: Extends non-Wide overlay entry from canonical browser rows to Grouped Music artist roots and album leaves.

## Impact

- Adds the `tui-treelistview` dependency without its optional keymap feature; its Ratatui generation matches mbv's current dependency.
- Affects Emby music DTO projection, settled music grouping, `MusicContent`, Music shell/workspace loading, Library panel list integration, rendering/theme composition, keyboard policy, mouse resolution, image fetching, and focused tests at the owning layers.
- Requires retaining stable artist identity and lazily loading artist artwork and tracks with stale-completion guards. Fallback artist roots remain browsable but do not request artist artwork.
- Supersedes `group-aware-page-navigation` only for Grouped Music: that change remains applicable to canonical Heading-based lists, while the tree uses native visible-node paging.
- Requires a new precise glossary term for the focusable Grouped Music artist root; the existing `Group heading` remains a non-selectable canonical-list row everywhere else.
