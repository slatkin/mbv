## Context

See `proposal.md` — Why. Two facts from the current tree shape the approach:

1. `render_wide_skeleton` (`components/library_panel/wide.rs`) already paints the Wide browser pane
   in one contiguous block: `wide_hero_browser_pane` → Selector row → optional List controls row →
   `fill_surface(list_panel, LibraryPanel, focused)` → list view. The Hero pane is painted after it,
   from a different rect. Nothing about that block depends on the Hero being present.

2. The Wide browser pane's `PANE_PAD_X` is **not a paint inset**. `full_width_claim(list_panel,
   list_area)` restores the claim's `x`/`width` for painting, and `claims_current_point` checks `x`
   against the claim and `y` against the content rect. The content rect's `x`/`width` therefore only
   move saved geometry; the painted row flow is `pane.x` in both geometries, and every row carries
   its own two-column indent. So the non-Wide rows a user sees today already match the Wide browser
   rows' left edge relative to their box.

Constraints:

- `panel.rs:701` gates the non-Wide double-click-to-overlay path on `narrow_geometry.is_some()`. The
  Library Hero overlay must keep opening in non-Wide geometry (proposal: preserved behaviour).
- ADR 0028 governs *new* geometry-scoped identities. This change retires one, so it does not
  contravene the ADR; the ADR's Consequences note that `NarrowLibraryBody` was the accepted
  non-Wide palette route and should record that it was retired.

## Goals / Non-Goals

**Goals:**

- One browser-pane painter serves the Wide list pane and the non-Wide library panel.
- One fixed backdrop for the library column body, in both geometries and both focus states.
- Delete the world that existed only for the non-Wide palette: `Surface::NarrowLibraryBody`,
  `PanelListPaintPolicy::Narrow`, `WideMediaListPaintPolicy::{body, scrollbar}` and their painter
  seams.
- Zero change to Wide geometry/appearance and zero change to the Library Hero overlay's pixels,
  focus, opening or dismissal.

**Non-Goals:**

- Reworking the Wide split, the Hero pane, or the overlay's own composition.
- Changing the non-Wide list's painted rows: they already match (Context fact 2).
- Any Queue, Service, Player, persistence or protocol change.

## Decisions

### D1 — Extract the browser pane; the non-Wide panel is the whole pane

Move the pane composition (Selector row, controls row, list box fill, list view, hit registration)
out of `render_wide_skeleton` into one `paint_browser_pane(f, pane: WideHeroBrowserPane, content,
list_focused, hovered_selector, hits, windows)` returning the painted role rects. Both geometries
call it:

- Wide: `wide_hero_browser_pane(panes.browser_panel, panes.browser_area)`, then paint the Hero pane.
- Non-Wide: the pane is `pill_bar_areas(area)` — the panel *is* the browser pane, full width.

The only narrow/wide branch left is "is there a Hero pane to paint". *Alternative rejected*: keep two
skeletons and only fix the fills — leaves the second paint path (and its drift) in place.

### D2 — The non-Wide column body is the Wide column's fixed backdrop

`library_body_fill` loses its non-Wide focus branch and returns the `LibraryColumn` row's fixed
`#2d353b` for every geometry, so the panel placement and the status band's padding rows stay on the
backdrop. The Selector row's spacer row uses the same `PillRowGap` surface the Wide pane uses
(already `#2d353b`, fixed). `Surface::NarrowLibraryBody` is deleted from `surface`, `surface_table`,
`surface_resolve`'s pinned fills and the surface-conformance residual table. *Alternative*: keep the
identity and pin it to the backdrop — two identities with one value is the duplication the surface
table exists to avoid.

### D3 — The non-Wide list uses the `Wide` policy verbatim

The non-Wide list is configured with `PanelListPaintPolicy::Wide { focused }`, so the skeleton's
`fill_surface` paints the box and the list carries no body. `PanelListPaintPolicy::Narrow`,
`WideMediaListPaintPolicy::{body, scrollbar}` with `with_body`/`with_scrollbar`, `body_bg`/
`scrollbar_bg`, and the painter's `body_bg`/`scrollbar_bg` parameters are deleted; the scrollbar
column resolves its selected-row surface exactly as the Wide browser list does. *Alternative*: keep
the policy arm and set it to the same values as `Wide` — a second arm that must be kept in sync.

### D4 — Keep both geometry slots; they carry the overlay gate

`NarrowSkeletonGeometry` is deleted; `narrow_geometry` becomes `Option<WideSkeletonGeometry>` and
`render_narrow_skeleton` returns one (its `hero`/`workspace`/`hero_image`/`overview_*` fields stay
empty). `narrow_geometry.is_some()` therefore keeps its exact meaning — "a non-Wide frame painted" —
and the double-click-to-overlay gate, `menu_geometry`, `list_rect` and `test_painted_layout` keep
working with a type change only. *Alternative*: collapse to one slot plus a `non_wide: bool` — more
churn in the overlay path for no gain.

### D5 — The one visible delta is the surrounding tone

Painted rows, row indent, list-box fill, stripe pair, controls and selector rows do not move; the
non-Wide list's claim/row-flow saved rects become the Wide pane's, and the surrounding body goes
`#3c4841` (focused) → `#2d353b`. This is the acceptance evidence to look for on a real terminal.

## Risks / Trade-offs

- *Tests pinned to `NarrowLibraryBody`* → update `tests_surface_conformance.rs` (coverage list and
  residual entry), `pinned_fills` in `surface_resolve`, and `shell_library_panel`'s doc comment in
  the same change; `narrow_tests.rs`'s `list_area.y == controls.bottom() + PANE_PAD_Y` assertion still
  holds under the Wide pane's geometry.
- *Non-Wide hit geometry shifts to the Wide pane's rects* (the outer two columns of the list box, and
  the context-menu anchor rect) → intended: "identical to Wide" includes the saved rects. Cover with a
  tick-integration or component test that clicks the list edge and opens the context menu in
  non-Wide geometry before/after.
- *The overlay gate is easy to break while deleting `NarrowSkeletonGeometry`* → D4 keeps the second
  slot; a test must assert the double-click-in-non-Wide path still opens the overlay, and that it does
  not open in Wide.
- *Mini shares the non-Wide arm* → it takes the same fixed backdrop with no Mini-specific bit; assert
  Mini and Narrow paint the same body.