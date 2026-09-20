# Invariant 9 — The Grouped Music tree claims a point only from the latest completed view

**Scope:** `MusicTreeBrowser` (`src/app/components/music_tree.rs`), its
`PanelList` adapter (`src/app/components/library_panel/panel_list.rs`), the
Wide paint order that drives it
(`src/app/components/library_panel/wide.rs`), and the pointer consumers
(`src/app/components/music_interaction.rs`, `music_content.rs`).

## The invariant

1. The tree owner retains row→node hit geometry from its **last completed
   `view()`** call. `paint_complete` is set `true` only at the end of
   `view()`, after the crate's `TreeListView::render` has populated the
   state's hit map.
2. `claims_point`, `hit_test`, `hit_node`, and `selected_row_rect` resolve
   only while `paint_complete` is true. While it is false the owner claims no
   point at all.
3. Every mutator that can change the projection or the row window between
   frames must call `invalidate()` before the next `view()`:
   - `reconcile(entries)` — only when the projection was actually rebuilt
     (a no-op settled push keeps the completed geometry);
   - `expand_root` / `collapse_root` (and `toggle_root` through them);
   - `select_id` / `select_album_target` — the crate's `select_by_id`
     expands ancestors via `expand_to`, which can insert rows the hit map
     does not know about;
   - `clamp_viewport_to` — the panel's viewport-height clamp;
   - the `PanelList` adapter's `set_paint_policy` and `set_geometry`, and the
     test-only `scroll_to` / `expand_all_roots`.
4. The Library panel's per-frame sequence for the slot ends **claimable**:
   `clamp_viewport` → `set_paint_policy` → `set_geometry` → `view`
   (`wide.rs`, `ListSlot::Media` / `ListSlot::Search` arms). Every rendered
   frame therefore completes with fresh geometry, and a subsequent pointer
   event resolves against exactly what the user saw.

## Why it matters

No type enforces this. `TreeListViewState::hit_test` resolves a coordinate
against the `hit_map` captured by the last render call; the projection and
the model can be refreshed independently of that map. If a future mutator
calls `ensure_projection` (or any other projection-refresh path such as
`select_by_id` / `expand_to`) without invalidating, the retained hit map is
interpreted against the new row order: a click at a coordinate silently
selects a row the user never saw, and the shell acts on the wrong album or
artist. The `paint_complete` gate is the only thing standing between the two
views.

Invalidation on *apparently pure* selection changes is also required.
`select_by_id` and `select_album_target` call the crate's `select_by_id`,
which calls `expand_to` to load the target's ancestor path before selecting.
Expanding an artist root inserts its leaf rows into the projection
immediately (and `KeepInView` may then move the window), so the node at a
given coordinate changes even though the entries themselves did not.
Selection movement that does **not** rebuild the projection
(`select_index`, `move_selection`, `select_first_visible`) deliberately does
not invalidate: the crate applies its `KeepInView` scroll during the next
render, so the retained geometry still matches the frame the user saw.

## How the code maintains it today

- **Completion gate.** `view()` sets `paint_complete = true` only after
  `StatefulWidget::render(widget, area, frame.buffer_mut(), state)`;
  `invalidate()` sets it false. `claims_point` / `hit_test` / `hit_node` /
  `selected_row_rect` all early-return `None`/`false` on
  `!self.paint_complete`.
- **Per-mutator invalidation.** `reconcile` (when `rebuilt`),
  `expand_root`, `collapse_root`, `select_id`, `select_album_target`,
  `clamp_viewport_to`, and the test seams call `invalidate()`; the
  `PanelList` adapter does the same in `set_paint_policy` and
  `set_geometry` so the panel's contract holds even though the tree owns its
  own rect. `music_content.rs` additionally invalidates when search takes
  over the frame (`self.browser.invalidate()` at the search hand-off).
- **Order.** `wide.rs` clamps, sets the paint policy, sets the geometry, and
  only then views, so a frame that reaches `view()` always re-completes the
  claimable geometry; the run loop draws after processing events, so a
  pointer event is resolved against the previous completed frame.
- **Tests.** `tree_hit_geometry_is_claimable_only_after_the_latest_view`
  (`src/app/components/music_tree_tests.rs`) pins the gate for an explicit
  `invalidate`, a `reconcile` that rebuilds, and `clamp_viewport_to`.

## Where it currently fails / how it could regress

The invariant is upheld by hand-added `invalidate()` calls at each mutator,
not by a type or a shared choke point. The registry of mutators is the
contract, and nothing checks it:

- A **new mutator** (or a new caller that reaches `ensure_projection` /
  `select_by_id` / `expand_to` directly) that forgets `invalidate`
  regresses the property silently — no compiler error, no failing type, and
  the existing tests only cover the mutators that were remembered.
- A future path that mutates the projection outside `MusicTreeBrowser`
  (for example in `music_content.rs` through a new state accessor) would
  bypass every listed call site.

**Known upgrade path (not done here):** derive staleness structurally
instead of trusting the call sites. Capture the projection's identity stamp
at the end of `view()` — the model's `TreeRevision` (`MusicTreeModel::revision`
only advances on settled content, so it must be paired with the expansion /
selection revision the crate tracks) — and let `hit_test` reject a point
whose current stamp no longer matches the stamp retained from the completed
render. That makes a missed `invalidate` harmless by construction rather
than a silent wrong-row selection.
