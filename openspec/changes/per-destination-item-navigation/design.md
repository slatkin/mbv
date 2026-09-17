# per-destination-item-navigation — Design

## Context

`spawn_navigate_to_item` (`src/app/library_browse_actions.rs`) is the single
entry point for both cross-surface callers: the queue context menu's
"Go to Library" (`ContextAction::GoToLibrary(id, item_type)`, resolved in
`src/app/context_menu_actions.rs`) and Search sidebar activation
(`input_search_sidebar_keys.rs`). It walks `get_ancestors`, drops the two
structural ancestors, and rebuilds the remaining chain as `BrowseLevel`s whose
cursors rest on the next level's target. Delivery is `LibEvent::NavigateTo`,
handled in `lib_event_actions.rs`: replace `nav_stack`, then `set_library_tab`.
Since 34dbbd55 a completed navigation also replaces the saved Library position,
and the Model drain (`shell_inline_search.rs::handle_inline_search_lib_event`)
re-anchors retained TV/Music owners.

The Emby destination surfaces are not uniform browse chains:

- Movies / generic video: a plain drill-in chain — the generic rebuild is
  already correct.
- TV (`TvContent`): depth-1 series list + Workspace (Wide) or Library Hero
  overlay (Narrow). Series activation goes through `activate_searched_series`
  (root level only: letter pill, cursor, `fetch_series_detail`) plus the shell
  hand-off in `activate_inline_search_item` (`activate_selected_series_item` /
  `open_library_hero_overlay`, `reanchor_tv_owner_selection`,
  `push_tv_workspace_content`). A Season level in the nav stack renders no
  existing screen.
- Music (`MusicContent`): group/album list + track Workspace; recursive album
  activation (`activate_recursive_album`, `RecursiveAlbumActivated`) replaces
  the nav stack for an album, and the group view owns the root level.

## Goals / Non-Goals

**Goals**

- One per-kind landing semantic shared by both callers.
- Reuse the existing activation flows; no new surface machinery.
- Preserve 34dbbd55's invariants (navigated state survives the tab switch;
  retained owners re-anchor).

**Non-Goals**

- Changing the queue menu entry, Inline Search activation, or Library Hero
  overlay requirements (they stay as specified).
- Audiobookshelf/Feeds navigation (no queue items of those kinds get the
  Emby menu).
- Changing letter-pill, sort, or Library-position restore semantics beyond the
  navigation-completes case.

## Decisions

### D1: Route by resolving a "reveal item" before building any state

The navigation resolves a single **reveal item** first, then per kind builds
the landing:

| item_type | reveal item | landing |
|---|---|---|
| Movie / generic | the item | root level, cursor on it (existing chain path) |
| Series | the item | `activate_searched_series` flow |
| Episode / Season | owning Series | `activate_searched_series` flow |
| Audio / MusicAlbum | owning album | Music album activation |
| MusicArtist | itself | plain chain (`[library, artist]` levels - the artist's album list as the top level; an artist has no single owning album, so the original "resolve to its album" rule was unsatisfiable and was renamed to this rule during U2 review) |

Resolution uses the item's own `series_id` for Episode/Season (one field, no
extra round trip when present; `get_ancestors` remains the fallback), and
`album_id` for Audio tracks. Alternatives considered: keeping the ancestor
chain and truncating it for TV (rejected — it still fabricates a Season level
and duplicates the activation flow the codebase already owns), and special-
casing only the queue menu (rejected — Search sidebar shares the entry point
and would keep producing phantom states).

### D2: The landing event carries the reveal item, not a pre-built stack

`LibEvent::NavigateTo` gains the resolved reveal item (or the per-kind payload
the App needs); the App applies the per-kind landing at drain time. The
Movie/generic arm keeps the existing nav-stack payload. This keeps the
async/Drain boundary: the worker resolves the reveal item (it already calls
`get_ancestors` there), the App applies state synchronously, the shell does its
presentations at the existing event seam. Alternative considered: having the
App thread call activation flows directly — rejected, those flows are
synchronous App state and the event already exists.

### D3: Shell hand-off mirrors Inline Search's series/album activation exactly

The Model drain on a navigated show/album performs the same sequence as
`activate_inline_search_item`: TV — `reanchor_tv_owner_selection`,
`push_tv_workspace_content`, then `activate_selected_series_item` (Wide) or
`open_library_hero_overlay` (Narrow), and re-push. Music — set
`music_workspace_reanchor` and push; the owner adopts the album's track list as
workspace content. No new hand-off kind is invented. The re-anchor added in
34dbbd55 is subsumed by this sequence (kept for the Movie/generic arm).

### D4: Landing replaces the saved Library position (generalized)

The 34dbbd55 `save_default_library_position` call stays, applied to whatever
state the per-kind landing produced, before `switch_tab` activation. The
restore fence (requested-position check in
`handle_restored_library_position`) then discards any pre-navigation restore.

### D5: Ensure-then-land for a Series whose target library cannot satisfy the landing

A miss against a root corpus that can still grow is not a failure (the
pre-change Chain arm landed regardless of library load state, and the spec
requires the landing to select the show). Instead the landing arms a pending
state and grows the corpus: `ensure_lib_loaded_for` for an unloaded library,
`spawn_all_items_prefetch` when the root is loaded but `all_items` is absent
(`ensure_lib_loaded_for`'s saved-position restore path never emits `Loaded`, so
the restored-position drain is a third retry trigger; its prefetch only fires
for this user-initiated pending, keeping startup restore's no-prefetch regime).
Retries fire only on drains whose `parent_id` is the library root's parent.
The miss rule - flash, tab unchanged - is reserved for a genuinely absent item
against a complete corpus. Lifecycle: the pending is consumed only by its own
library's drains, cleared on `LibEvent::Error`, and cleared on any manual tab
change; the deferred album tab switch (`pending_navigate_tab_switch`) and the
series hand-off (`pending_series_handoff`) follow the same pattern. Loss
semantics: an armed pending that is discarded by a manual tab change or an
unscoped `LibEvent::Error` never surfaces an error - it simply leaves the
current view untouched.

## Risks / Trade-offs

- [Music group view has two root shapes (grouped vs plain)] → the album reveal
  rides `activate_recursive_album`'s existing replace-on-arrival semantics, so
  the landed stack is whatever that flow already produces; a grouped root keeps
  its group level. Evidence tests cover one grouped and one flat library shape.
- [Reveal-item resolution can fail server-side (deleted item, offline server)]
  → resolve failure flashes the existing library-error path and leaves the
  current tab unchanged, matching today's `LibEvent::Error` handling.
- [Search sidebar results that are plain folders (collections/playlists) still
  go through the generic chain] → unchanged behavior for kinds outside the
  table; the chain is correct wherever the surface is a chain.

## Migration Plan

Single-repo change; no persistence migration (Library position snapshots of the
new shapes are already serializable browse levels). Rollback = revert commit.

## Open Questions

None — the three scope questions from exploration were answered by the user's
direction (land the show, workspace opens, fix shared path, music included).
