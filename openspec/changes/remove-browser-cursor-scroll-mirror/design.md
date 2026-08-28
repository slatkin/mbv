## Context

`BrowserComponent` (`src/app/components/browser.rs`) paints the generic/
Movies/HomeVideos Emby browser and already routes every claimed key through a
typed `ShellRequest` — this is a D14 stage-1-and-most-of-stage-2 surface, not
a raw-forwarding one. What remains is the last-mile interaction-state pin
D17 exists to close: see the scout handoff at
`openspec/handoffs/scout-remove-browser-cursor-scroll-mirror.md` for the full
symbol-level inventory (inputs to the mirror, component-local vs. shell-owned
state, choke points, the ~37 `nav_stack` readers, and this document's ordering
resolution, which that handoff also records). This design summarizes the
decisions the handoff's discovery supports; it does not repeat the inventory.

`App::move_lib_cursor_rows` / `move_lib_cursor` / `jump_lib_cursor`
(`src/app/lib_cursor_actions.rs`) are effectful, not setters — pagination
(`maybe_fetch_next_page`), position persistence
(`save_default_library_position`), and navigation-idle image fetch
(`mark_library_navigation`/`last_nav_at`) all key off them. `BrowseLevel`
(`src/app/types_browse.rs`) is read by roughly 37 non-test production files
through `nav_stack`, most for concerns unrelated to the Browser's own
interaction (pagination, other still-legacy surfaces' own `nav_stack` shape,
position persistence, letter pills, search scoping). Deleting a `BrowseLevel`
field is not on this change's table (D17: "the smallest compile-complete
implementation units," not global field deletion); re-homing every unrelated
reader is a separate, much larger effort than this issue scopes.

## Goals / Non-Goals

**Goals:**
- Remove the per-frame `browser.scroll()` → `level.scroll` write-back inside
  the draw closure.
- Remove the cursor round trip: stop the shell from independently
  recomputing a movement the component already resolved, and stop
  `push_emby_browser_content` from silently overwriting the component's
  local cursor after every effect.
- Preserve pagination, position persistence, and navigation-marking side
  effects byte-for-byte (parity is the active production path, per D17 — not
  an opportunity to change behavior).
- Finish D18 step 2 for this surface: the wide-Movies layout signal moves
  from the legacy renderer's `movies_wide_right_area` output to a
  component-owned derivation, and the Emby-specific legacy wide-renderer
  functions this component was the last reader of are deleted.

**Non-Goals:**
- Deleting `BrowseLevel.cursor`/`.scroll` as fields, or re-homing any of the
  ~37 unrelated `nav_stack` readers (pagination, other surfaces, letter
  pills, search, music grouping, context menu/shuffle for non-Browser
  surfaces).
- Removing the shared `self.app.render(f)` legacy-underpaint call in
  `shell_run.rs` — that is issue #613 (`resolve-migrated-surface-correctness`)
  item 3, out of scope here; see "Ordering resolution" below.
- Any change to mouse routing (accepted-broken for the alpha, D16) or to the
  four typed selected-item effects, context menu, shuffle, refresh/rescan,
  back navigation, or letter-pill cycling, none of which carry a cursor
  mirror today.
- Any change to `crate::config::LibraryPosition`'s on-disk or shared-document
  shape.

## Decisions

### D1 — Cursor: component resolves, shell applies; no shell-side recompute

Today: component predicts its own cursor (`move_rows`/`move_cursor_delta`/
`jump_cursor` in `browser_navigation.rs`), emits a delta
(`BrowserMoveRows{rows}` etc.), the shell independently recomputes the same
movement against `BrowseLevel.cursor` via `App::move_lib_cursor_rows`, then
`push_emby_browser_content` re-reads the App's result back into the
component. Two independent implementations of the same arithmetic must stay
byte-identical, and every one of the shell's callback sites re-syncs the
component's cursor after the fact.

Decision: the request carries the component's resolved *index*, not a delta.
The shell gets a new `App` method (e.g. `apply_lib_cursor_index`) that writes
`BrowseLevel.cursor = index` directly and then runs the same
`save_default_library_position` / `mark_library_navigation` /
`maybe_fetch_next_page` / `last_nav_at` tail `move_lib_cursor_inner` already
runs — it does not recompute the index from a delta. This is exactly the
"typed intent carrying the component-resolved index, with parity proven"
shape the issue calls for. Existing `App::move_lib_cursor_rows`/
`move_lib_cursor`/`jump_lib_cursor` stay as-is for every other caller of
those methods (other still-legacy surfaces reachable through the same
`BrowseLevel` shape) — only the Browser's `ShellRequest` arm changes which
`App` method it calls.

**Alternative considered**: keep the delta-based request and instead stop
`push_emby_browser_content` from overwriting `self.cursor` after a Browser
effect. Rejected — it leaves the duplicate arithmetic in place (both sides
still compute the same movement independently), so a future divergence
between the component's `browser_navigation.rs` and the App's
`lib_cursor_actions.rs` letter-grouped/flat/column logic would silently
desync the two without either side erroring, only intermittently
resurfacing as a visible cursor jump on the next re-sync. Passing the
resolved index removes the second computation entirely.

### D2 — Scroll: persist only at navigation choke points, not every frame

Decision: delete the write-back in `render_emby_browser_component`. Instead,
persist `BrowseLevel.scroll` from the component's current scroll at the two
places the visible level actually changes: `select_item`'s folder push
(`actions_navigation.rs`) and `go_back`'s pop (`actions_navigation.rs`) — the
same functions that already own `save_default_library_position` calls for
cursor. A folder push starts a fresh level at `scroll: 0` (already true;
nothing to persist there beyond the outgoing level, which the component's
last-known scroll captures at pop time via the same mechanism `go_back`
already reads `nav_stack` through). Tab-switch-away and quit-time flush
(`flush_library_position_now`) already exist as choke points in
`library_position_state.rs`; if either needs the component's live scroll
mid-session, the shell reads it once at the choke point (`BrowserComponent`
already exposes `scroll()`), not every draw.

**Alternative considered**: give the component per-`BrowserKey` position
memory (its own map from library id to last cursor/scroll) instead of
persisting through `BrowseLevel`. Rejected — `BrowseLevel.cursor`/`.scroll`
already are the cross-session persistence authority
(`library_position_state.rs`, `crate::config::LibraryPosition`), read by
`activate_library_position` on tab activation and by save/restore-on-relaunch.
Duplicating that into component-local memory would create a second source of
truth for the same fact (App's `library_position_state` document vs. a new
component map) with no reconciliation rule, exactly the kind of pin D17
exists to remove, not introduce. Keeping `BrowseLevel` as the one persisted
copy and changing *when* it is written (choke points, not every frame) is the
smaller, single-authority change.

### D3 — Ordering resolution: this change vs. issue #613

The issue frames D17 stage 5 ("detach component geometry/content from legacy
underpaint, then delete that surface's legacy renderer") against issue #613's
stated sequencing ("after the relevant ownership slices of #611") as
apparently circular. It is not, once "underpaint" is split into two
independent layers (full reasoning in the scout handoff §6):

1. **This change's scope**: the Browser's own dependency on
   `movies_wide_right_area`, populated by the Emby-specific legacy
   wide-renderer functions. D18 step 2 already commits this change to derive
   "wide" from the component's own `BrowserKey` kind + geometry width and
   delete those Emby-specific functions once they have no remaining reader.
   That is a bounded, surface-local deletion this change finishes.
2. **Issue #613's scope**: the single shared `self.app.render(f)` call in
   `shell_run.rs` that paints the legacy surface beneath *every* migrated
   component. It stays, because TV/Music/album-detail branches still depend
   on it. Removing that call is #613 item 3, and #613 is correctly sequenced
   after #611's slices (including this one) precisely because it cannot run
   until no migrated surface — including the Browser — still depends on
   anything the legacy base frame populates.

This change performs step 1 and explicitly does not touch step 2. #613's
"after #611" sequencing is satisfied, not contradicted, once this change
lands.

## Risks / Trade-offs

- [Duplicate-arithmetic parity risk during the transition] → D1's resolved-
  index handoff removes the risk by construction once landed; until then the
  existing behavior (recompute + re-sync) stays in place unchanged, so there
  is no window where cursor movement is unverified.
- [Choke-point scroll persistence could miss a session-only scroll change if
  the user never leaves the level and quits] → `flush_library_position_now`
  already runs at teardown; extend it (or the tab-switch-away choke point) to
  read the component's live `scroll()` once before the app exits, matching
  the existing "final burst never lost" contract `library_position_state.rs`
  documents for cursor.
- [D18 step 2 derivation drifting from the still-active legacy wide renderer
  during development] → D18 already forbids computing the derivation before
  this change's own underpaint-detach unit lands last (after D1/D2 are
  proven stable); this design keeps that ordering.
- [Unrelated `nav_stack` readers accidentally treated as in-scope during
  implementation] → the scout handoff §5 enumerates them by concern
  (pagination, other surfaces, persistence, letter pills, music grouping,
  search) precisely so a writer unit can recognize "not this change" quickly
  instead of re-discovering it mid-implementation.

## Migration Plan

No data migration; `crate::config::LibraryPosition`'s shape is unchanged.
Rollout is the three writer units below, each independently
compile-complete and mergeable:

1. Cursor effect re-homing (D1).
2. Scroll ownership at navigation choke points (D2) — depends on 1 (reuses
   the same choke-point pattern and request shape).
3. Underpaint detach (D18 step 2 / D3) — depends on 1 and 2 landing first,
   per D18's explicit prohibition on computing the wide-layout derivation
   ahead of stable ground.

No rollback beyond a normal revert; no feature flag (interaction-state
ownership is not user-configurable).
