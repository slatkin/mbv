## Context

`BrowserComponent` (`src/app/components/browser.rs`) paints the generic/
Movies/HomeVideos Emby browser and already routes every claimed key through a
typed `ShellRequest` — this is a D14 stage-1-and-most-of-stage-2 surface, not
a raw-forwarding one. The prior design cited
`openspec/handoffs/scout-remove-browser-cursor-scroll-mirror.md`, but that
artifact is absent. Reconciliation established the bounded contract directly:
this change owns only removal of the Browser component's shell-projected wide
input; the shared `movies_wide_right_area` producer/readers and any
legacy-underpaint cleanup are cross-surface geometry work owned by
`remove-migrated-surface-underpaint` (#613).

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
- Finish the Browser-local portion of D18 step 2: its wide-Movies/HomeVideos
  input is derived from `BrowserKey` kind plus painted geometry at the existing
  breakpoint, rather than projected from `App::layout`.

**Non-Goals:**
- Deleting `BrowseLevel.cursor`/`.scroll` as fields, or re-homing any of the
  ~37 unrelated `nav_stack` readers (pagination, other surfaces, letter
  pills, search, music grouping, context menu/shuffle for non-Browser
  surfaces).
- Removing the shared `self.app.render(f)` legacy-underpaint call in
  `shell_run.rs` — that is issue #613 (`remove-migrated-surface-underpaint`),
  out of scope here; see "Ordering resolution" below.
- Deleting `movies_wide_right_area`, `is_wide_movies_active()`, their shared
  cross-surface readers, or a legacy wide renderer. The named Emby-specific
  renderer is already absent; the remaining geometry field cleanup belongs to
  #613's paint-free geometry pass.
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

### D3 — Ordering resolution: Browser input isolation before #613 geometry cleanup

D17's "detach component geometry/content from legacy underpaint" has two
layers. The Browser-local layer is a prerequisite to #613, while the shared
geometry and paint layer is #613 itself:

1. **This change's scope**: derive the Browser's wide mode from its own
   `BrowserKey` kind and painted geometry at the existing breakpoint, and stop
   projecting `App::layout.main.is_wide_movies_active()` into the component.
   This is a bounded, surface-local input-ownership change that preserves the
   visible wide/narrow result.
2. **Issue #613's scope**: retain or replace shared geometry facts such as
   `movies_wide_right_area` while extracting a paint-free geometry pass, then
   delete the shared facts/readers and a legacy body renderer only after all
   of their readers are re-homed. It exclusively owns the shared
   `self.app.render(f)` underpaint removal.

The earlier task's supposed Emby-specific renderer deletion is obsolete: that
renderer is already absent. Keeping its remaining field/readers in #613 avoids
turning a Browser-only migration into a cross-surface geometry rewrite. #613's
"after #611" ordering is satisfied once this input isolation lands.

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
- [D18 step 2 derivation drifting from the existing wide/narrow result]
  → it lands only after D1/D2 and reuses the existing component breakpoint;
  focused wide/narrow navigation tests prove parity.
- [Shared geometry cleanup accidentally absorbed into this change] → #613 owns
  `movies_wide_right_area`, its cross-surface readers, and legacy-body
  deletion; this change's unit has a zero-touch boundary for those symbols.

## Migration Plan

No data migration; `crate::config::LibraryPosition`'s shape is unchanged.
Rollout is the three writer units below, each independently
compile-complete and mergeable:

1. Cursor effect re-homing (D1).
2. Scroll ownership at navigation choke points (D2) — depends on 1 (reuses
   the same choke-point pattern and request shape).
3. Browser-wide input isolation (D18 step 2 / D3) — depends on 1 and 2
   landing first. It derives the component's wide mode locally and leaves
   shared geometry/underpaint cleanup to #613.

No rollback beyond a normal revert; no feature flag (interaction-state
ownership is not user-configurable).
