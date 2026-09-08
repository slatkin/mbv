## Context

The queue-column mouse resize (#682, `add-mouse-column-resize`) established the pattern: a dedicated shell-mounted boundary component owns the grab zone and gesture, emits semantic resolved-width messages, and the shell applies the width. The Wide hero arrangement already computes its two-pane split in exactly one shared function (`wide_hero_split` in `src/app/render/arrangements/wide_hero.rs`), which every wide surface calls with its own base rect (browser `left_area`, TV `right_panel_lib_area`, Music `wide_music_area`, Home/Feeds/ABS areas). Breakpoint detection calls the same function for `.is_some()` only.

Key constraint: paint-time geometry is recomputed from the passed rect by design — relayed geometry lags one pass (the `player_area` defect fixed in 37cb230a). The split override must ride the existing recompute flow, not a stored pane projection.

## Goals / Non-Goals

**Goals:**
- One central mechanism: one override field, one clamp helper, one boundary component, one dispatch arm — all wide surfaces resize at once.
- Preserve the paint-time-recompute design and the zero-visual-change property of the queue boundary.

**Non-Goals:**
- No persistence layer, no preference/config schema change, no keyboard equivalent.
- No per-surface split memory (one session-wide width, per-surface clamp on apply).
- No consolidation of pane computation into `layout.rs` (see Decisions).

## Decisions

- **Override rides the shared split function, not a stored projection.** `wide_hero_split` gains an override parameter (replacing the `* 2 / 5` default when present); a plain wrapper keeps the current signature so breakpoint `.is_some()` checks stay untouched. Geometry call sites thread the override (~10–12 mechanical touches). Alternative rejected: computing panes once in `layout.rs` and pushing them down — same ctx plumbing for mounted components anyway, but it fights the documented recompute design and reintroduces the stale-relay defect class.

- **State is one `App` field: `Option<u16>` list-pane width, session-only.** `None` = default ratio. Clamp helper mirrors `normalize_queue_column_width` but takes the active content-area width and enforces `WIDE_HERO_MIN_PANE_WIDTH` on both panes (`[40, width − 40 − 2]`); an empty range (content at the 82-column threshold) yields the default. Clamp-on-apply covers terminal resize and cross-surface switching without a resize-event hook.

- **Live-only messaging.** The queue's `ResizeColumnEnd` existed solely to persist; there is nothing to persist here, so the boundary component emits only a live-width message (new `ShellRequest` variant) and DragEnd is a no-op inside the component. One dispatch arm on the shell sets the override.

- **One component, one tab→area match.** `WideHeroBoundaryComponent` mirrors `QueueBoundaryComponent` (private gesture state, `MouseGestureState`, paints the gap with the backdrop it already shows). The shell maps the active `TabSelection` to its wide content area (one match reusing the rects `layout.rs` already publishes) to sync the gap rect and eligibility (`active wide surface && panel_mouse_eligible()`). No per-tab component or per-screen eligibility code.

- **Refresh reset lives in the refresh command handler.** `refresh_current_view` already matches on the active tab; clearing the override at its top reverts every surface in one place.

## Risks / Trade-offs

- [Override threading touches ~10–12 call sites] → each touch is a one-argument forward into the shared function; buffer tests on a representative surface prove all surfaces move together; `ast-grep scan` guards against a call site bypassing the override.
- [Grab zone is the 2-column gap vs the queue's 1-column edge] → slightly less precise arming; unchanged by design because the gap is the only existing boundary chrome. Sentinel buffer test proves the gap's appearance is byte-identical.
- [Split changes reflow hero content (images, wrapped text, episode lists) every drag tick] → same live-redraw cost profile as the queue drag; frames derive entirely from pane rects, so no extra invalidation logic.
- [Boundary competes with pane gestures at gap-adjacent columns] → ownership mirrors the queue: the boundary claims only the gap columns it paints; pane components keep their own geometry; integration test proves pane gestures survive a gap drag.

## Migration Plan

Additive: new component + field + parameter defaulting to current behavior (`None` override = exact current split). No data migration; rollback is reverting the change.

## Open Questions

None.
