# per-destination-item-navigation

## Why

The queue context menu's "Go to Library" rebuilds the item's Emby ancestor chain
as browse levels for every item kind. That is correct only for destinations
whose surface IS a browse chain (Movies/generic video). TV browses as a depth-1
series list plus a Workspace (there is no screen that lists a show's episodes as
a browse level), and Music browses groups/albums plus a track Workspace. For a
queued episode the navigation therefore lands on a browse state no screen
renders: the user sees a phantom seasons/episodes list instead of the show.
The same shared mechanism serves Search sidebar activation, so the fix must be
per destination, not per caller.

## What Changes

- Route item navigation per destination kind instead of rebuilding a generic
  ancestor-level chain:
  - Movie / generic video: unchanged — the root browse level with the cursor on
    the item (the existing chain behavior is already correct there).
  - Series: the existing series-activation flow (letter-range pill applied,
    cursor on the series, series detail fetched), workspace or Library Hero
    overlay opening through the same shell hand-off Inline Search uses.
  - Episode / Season: resolve the owning Series (via `series_id`/ancestors) and
    navigate to the show — never to a Season or Episode browse level.
  - Music item (track / album / artist): resolve to its album and select that
    album in the Music surface, with the track list as the workspace content —
    no Artist browse level.
- A completed navigation replaces the saved Library position for the target
  library (the 34dbbd55 behavior is kept and generalized).
- Retained TV and Music destination owners re-anchor to the navigated selection
  (34dbbd55 seam, extended to carry the workspace-open hand-off).
- The Search sidebar path shares the per-kind routing (same entry point,
  `spawn_navigate_to_item`); both callers get identical landing semantics.

## Capabilities

### New Capabilities
- `item-library-navigation`: navigating an Emby library to the surface state
  representing a chosen item (queue "Go to Library", Search sidebar): which
  browse levels may exist per destination kind, where the cursor rests, and
  what opens alongside.

### Modified Capabilities
<!-- none: the queue menu entry, Inline Search series activation, and Library
     Hero overlay behavior are already specified; this change adds a capability
     and reuses those flows rather than changing their requirements -->

## Impact

- `src/app/library_browse_actions.rs` (`spawn_navigate_to_item`) — the generic
  ancestor-chain walk is replaced or re-scoped to the Movie/generic case.
- `src/app/lib_event_actions.rs` (`LibEvent::NavigateTo` handling) — per-kind
  landing event shape (may need the landing item, not just a nav stack).
- `src/app/shell_inline_search.rs` (Model drain) — the TV/Music re-anchor seam
  gains the workspace-open / hero-overlay hand-off for navigated shows/albums.
- `src/app/context_menu_actions.rs` — "Go to Library" passes kind through
  unchanged (already carries `item_type`).
- Reuses existing pieces only: `activate_searched_series`, `fetch_series_detail`,
  `activate_recursive_album`, Library Hero overlay, `music_workspace_reanchor`.
- Tests: library-position activation family, inline-search activation family,
  plus new per-kind navigation evidence; openspec validate must stay green.
