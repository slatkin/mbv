## Why

Grouped Music presents a settled artist hierarchy as a flat list whose artist labels cannot receive focus, collapse, or act on their albums. A mergeable one-screen integration of `tui-treelistview` will test whether a shallow tree can make that hierarchy directly navigable and filterable while meeting mbv's existing visual, ownership, and input contracts; the dependency will be rejected if its supported customization cannot do so.

## What Changes

- **BREAKING (Grouped Music interaction):** replace the Grouped Music album `MediaList` with a two-level tree whose focusable artist roots expand or collapse and whose album leaves retain existing album activation.
- Add artist-root play, enqueue, shuffle, context, and multi-selection behavior, resolving roots to their visible album descendants in settled display order.
- Replace Grouped Music's flat full-corpus Inline Search presentation with an in-place, 300 ms debounced fuzzy tree filter over the current settled tree. Matching preserves settled order, retains artist ancestors, force-expands matching paths, and restores pre-filter selection and expansion on dismissal.
- Add an artist Hero with artwork and summary metadata plus an artist Workspace containing the in-scope tracks grouped under non-selectable album headings. Album Hero and Workspace behavior remains unchanged.
- Retain stable Emby artist identity in music data, with deterministic fallback identity for albums whose Service payload has no artist identity.
- Integrate the tree through `MusicContent`, the existing Library panel slots, central keyboard policy, and latest-frame mouse geometry. `TreeListViewState` is the sole browser cursor, expansion, scroll, and mark owner; no second router, painter, or shell mirror is added.
- Evaluate visual customization as an acceptance gate: the dependency must reproduce mbv's selected-row bar, hierarchy, semantic theme roles, zebra treatment, metadata, marquee, scrollbar, and representative Wide/non-Wide behavior through its supported extension points. A parallel bespoke tree renderer is out of scope.
- Keep every other destination, the Music track Workspace, and the artist track Workspace on canonical flat `MediaList` controls.

## Capabilities

### New Capabilities

- `grouped-music-tree-browser`: Defines the shallow artist/album tree, stable identities, tree navigation and actions, in-place fuzzy filtering, current-frame hit testing, refresh continuity, and dependency acceptance gate.

### Modified Capabilities

- `canonical-media-lists`: Removes the Grouped Music album browser from canonical flat-list ownership while retaining canonical `MediaList` ownership for Music track Workspaces.
- `stable-music-library-grouping`: Changes settled artist labels into stable, focusable tree roots while preserving atomic publication and refresh continuity.
- `music-library-hero`: Adds artist-focused Hero and grouped-track Workspace content alongside the existing album-focused detail behavior.
- `inline-library-search`: Defines Grouped Music's deliberate in-place current-tree filtering exception while all other destinations retain flat full-library results.
- `media-list-multi-select`: Extends uniform selection outcomes to the Grouped Music tree, including root aggregate state and visible-descendant materialization, without making artist identities effect targets.

## Impact

- Adds the `tui-treelistview` dependency without its optional keymap feature; its Ratatui generation matches mbv's current dependency.
- Affects Emby music DTO projection, settled music grouping, `MusicContent`, Music shell/workspace loading, Library panel list integration, rendering/theme composition, keyboard policy, mouse resolution, image fetching, and focused tests at the owning layers.
- Requires retaining stable artist identity and lazily loading artist artwork and tracks with stale-completion guards. Fallback artist roots remain browsable but do not request artist artwork.
- Supersedes `group-aware-page-navigation` only for Grouped Music: that change remains applicable to canonical Heading-based lists, while the tree uses native visible-node paging.
- Requires a new precise glossary term for the focusable Grouped Music artist root; the existing `Group heading` remains a non-selectable canonical-list row everywhere else.
