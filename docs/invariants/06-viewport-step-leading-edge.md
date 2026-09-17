# Invariant 6 — A scroll gesture carries the selection forward, never back onto its own edge

**Scope:** every scrollable list or viewport that owns a selection — the canonical media-list owner
(`src/app/components/media_list/`, its carrier and its fixed-row presentation
`src/app/render/components/media_list/wide.rs`), the destinations that compose it, the shell
restore/re-anchor boundaries that seed a window, and the overlays that still scroll their own offset
(Global Search sidebar, Settings, Help, Sessions, Playlists). Any new scrollable surface is in scope from
the day it is written.

## The invariant

1. **Window and selection have separate verbs.** A scroll gesture (wheel, `Ctrl+e`/`Ctrl+y`, `PgUp`/`PgDn`)
   moves the visible window. A cursor gesture (`↑`/`↓`, `j`/`k`, `Home`/`End`) moves the selection. Neither
   verb may silently become the other.
2. **When a scroll gesture must move the selection, the selection lands on the leading edge of the
   gesture.** A step toward the preceding display row drags the selection to the *first* selectable row of
   the new window; a step toward the following row drags it to the *last* selectable row of the new window.
   The selection travels with the gesture and may never be parked on the edge it is leaving (the trailing
   edge) and left to ride it.
3. **A scroll gesture moves the selection only when it would otherwise leave the window.** A step whose
   window still contains the selection moves only the window, and reports no selection move.
4. **The window may never be pinned to the selection's edge.** A cursor move moves the window the minimum
   distance that shows the selection, and may move one row further so the labelling `Heading` directly
   above the selection stays painted. No rule may make a display row (in particular display row 0 and a
   group's `Heading`) unreachable by any input.
5. **The window is owner state with exactly one writer: input.** Painting reads the window, clamps it for
   display only, and never stores a resolved offset back; no parent keeps a second copy and no path
   recomputes the window from the selection outside an input operation or an explicit restore boundary.

## Why it matters

The window and the selection are two coordinates, and every failure of this invariant collapses them into
one: whichever is on the "outer" edge becomes a slave of the other, and the list stops responding to the
gesture that is supposed to move it.

- Pinning the *selection* to the trailing edge (violating 2) makes a scroll verb useless for travel: the
  cursor stays on the same screen line while the content slides past it. Holding the verb never advances
  the selection relative to the screen, so a keyboard user — for whom this is the only bound verb — has to
  switch to cursor keys to move the highlight at all. It is the reported "scrolling back up pins the cursor
  to the bottom row".
- Pinning the *window* to the selection's edge (violating 3/4) makes part of the list unreachable: the
  window can never clear the selection's row, so the row above it, and on a grouped list the first group's
  `Heading`, can never be painted by any input (issue #731).
- Letting a paint write the window (violating 5) creates a second writer whose value depends on the last
  frame's height, so the window's position becomes a function of paint order — every generation of painter
  re-derives it differently.

## Where this has recurred

Three times in two months, in three different implementations, which is why the rule is written down here
rather than left to review:

1. **`510fe59d` (`src/app/render/power/list.rs`, "Keep letter group headers visible")** — the legacy
   power-view painter was patched to keep the labelling `Heading` painted above the selection, i.e. the
   window rule of (4) had to be taught to one painter.
2. **The canonical `MediaList` owner (September)** — written without that patch: the window rule raised the
   window onto the selection, so display row 0 and the first group's `Heading` became unreachable again
   (#731). Fixed by `media-list-viewport-scroll` (archived 2026-09-17): the cursor path's leading-context
   rule (`MediaList::follow_cursor`), the read-only paint, and the single writer.
3. **The same change's own step landing** — its D3 specified "the nearest selectable row the window
   shows", which mechanically resolves to the *trailing* edge: stepping up from the window's last row
   parked the selection on the new window's last row (the reported symptom). Superseded by
   `viewport-step-leading-edge`, which resolves the drag by step direction.

## How the code maintains it today

- One owner decides: `MediaList::scroll_viewport` / `scroll_viewport_page` move the window and call
  `drag_selection_into_window`, which resolves the drag against the step direction (leading edge) and never
  selects a `Heading`/`Spacer` row.
- The cursor path is separate and keeps the window only as close as needed:
  `MediaList::follow_cursor`, with its one-row `Heading` exception (rule 4).
- The paint is read-only: `render/components/media_list/wide.rs` reads the window, clamps for display, and
  stores nothing; the carrier takes the step's height from the retained frame
  (`WideMediaList::current_content_rect`) and treats a missing frame as a no-op.
- Restores seed the window once at the boundary (`BrowseLevel::from_position_level`); no paint corrects it
  afterwards.
- Pinned by `src/app/components/media_list/tests.rs`
  (`viewport_step_at_the_window_edge_drags_the_selection_both_ways`,
  `viewport_step_inside_the_window_moves_only_the_window`,
  `cursor_move_to_the_first_selectable_row_keeps_its_heading_in_the_window`, and the named leading-edge
  guard), by the per-surface tick evidence (`tests_tick_integration_*`), and by
  `paint_does_not_change_the_window_and_a_shorter_paint_does_not_raise_it`.
- The surface-by-surface proof lives in `docs/architecture/interactive-surface-ledger.md`.

## How to apply it to new work

1. Decide which coordinate a new gesture moves, and say so in the spec before writing code — a verb that
   moves both is the bug this invariant names.
2. Route the gesture through the shared owner if the surface composes a canonical list; do not give one
   surface its own landing rule.
3. When the gesture must move the selection, land it on the leading edge and prove it in a test that fails
   if the selection ends on the edge it left.
4. Never let a paint, a projection, or a parent store the window; if a value is needed at input time, read
   it from the retained frame that hit resolution already uses.
5. Record the proof in the ledger row for the surface.
