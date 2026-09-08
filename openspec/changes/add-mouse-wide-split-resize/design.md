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

- **Override rides the shared split function, not a stored projection.** `wide_hero_split` gains an override parameter (replacing the `* 2 / 5` default when present); breakpoint checks move to a dedicated breakpoint predicate so geometry exits only through the override-taking function, and an `ast-grep scan` rule confines `wide_hero_split` callers to the arrangement file as a structural ratchet. The override reaches paint paths two ways: shell-direct paths (`shell_browser`, `shell_music_workspace`, TV wide) read the `App` field directly; mounted destination components receive it through their existing sync/push render-ctx fields (`LibraryListRenderCtx`, `MusicWideRenderCtx`, the Home/Feeds/ABS render-fn parameters) and forward it into the render functions. ~15–20 mechanical forwarding touches across ~10 files; no screen carries resize logic. Alternative rejected: computing panes once in `layout.rs` and pushing them down — same ctx plumbing for mounted components anyway, but it fights the documented recompute design and reintroduces the stale-relay defect class.

- **State is one `App` field: `Option<u16>` list-pane width, session-only.** `None` = default ratio. Clamp helper mirrors `normalize_queue_column_width` but takes the active content-area width and enforces `WIDE_HERO_MIN_PANE_WIDTH` on both panes (`[40, width − 40 − 2]`); an empty range (content at the 82-column threshold) yields the default. The helper normalizes on every application against the active surface's content width, so terminal resize and cross-surface switching need no dedicated event hook.

- **Live-only messaging.** The queue's `ResizeColumnEnd` existed solely to persist; there is nothing to persist here, so the boundary component emits only a live-width message (new `ShellRequest` variant) and DragEnd is a no-op inside the component. One dispatch arm on the shell sets the override. Eligibility loss mid-drag (overlay mount, mode change) resets the component's gesture state before delivery, mirroring the queue's arbitration cancellation, so no stale width can be emitted after eligibility ends.

- **One component, one tab→area match with painted-split eligibility.** `WideHeroBoundaryComponent` mirrors `QueueBoundaryComponent` (private gesture state, `MouseGestureState`, paints the gap with the backdrop it already shows). The shell maps the active `TabSelection` to its wide content area (one match reusing the rects `layout.rs` already publishes, re-synced every tick — the landed queue precedent for one-frame transition staleness). Eligibility is `panel_mouse_eligible() && <the active surface paints the two-pane split now>`: breakpoint fit alone is not enough, because empty/loading/no-selection states return before painting the hero pane (verified in `render/components/feeds.rs`), which would expose a grab zone over an unsplit frame. Each match arm carries its surface's existing wide-paint gate (Feeds: subscriptions exist and entries are non-empty; Home/ABS/library arms: their paint-time wide branches); the match stays central — no per-tab component and no per-screen gesture code.

- **Resolution formula pinned.** The list-pane width resolves as the pointer column minus the pane origin. The gap column nearest the browser pane is the exact edge, so grabbing it without motion emits nothing (no jump on grab); the outer gap column resolves one column wider and self-aligns to exact-edge tracking on the next pointer event; once a drag has changed the width, tracking continues while the pointer moves outside the gap (queue precedent). Clamping applies after resolution, so the edge rests at the nearest valid bound.

- **Refresh reset lives in the library-side refresh arm.** `refresh_current_view` dispatches on panel focus first (queue focus runs `refresh_queue`); clearing the override at the top of the function would revert the library split when the user refreshes the queue. The reset therefore lives inside the `PanelFocus::Library` arm: refreshing the active library view reverts the split; refreshing while the queue holds focus leaves it untouched.

## Risks / Trade-offs

- [Override threading touches ~15–20 call sites across ~10 files] → each touch is a one-argument forward into the shared function; the dedicated breakpoint predicate plus the `ast-grep` confinement rule make a bypassing call site unrepresentable and scannable; buffer tests on a representative surface prove all surfaces move together.
- [Grab zone is the 2-column gap vs the queue's 1-column edge] → the pinned resolution formula keeps grabs jump-free from the near column and within one column from the outer column. Sentinel buffer tests prove the gap's appearance is byte-identical, including an empty/loading state where no split is painted and the boundary must not arm.
- [Split changes reflow hero content (images, wrapped text, episode lists) every drag tick] → same live-redraw cost profile as the queue drag; frames derive entirely from pane rects, so no extra invalidation logic.
- [Boundary competes with pane gestures at gap-adjacent columns] → ownership mirrors the queue: the boundary claims only the gap columns it paints; pane components keep their own geometry; integration test proves pane gestures survive a gap drag.

## Migration Plan

Additive: new component + field + parameter defaulting to current behavior (`None` override = exact current split). No data migration; rollback is reverting the change.

## Open Questions

None.
