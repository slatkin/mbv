## Context

The library panel resolves through one closed surface table (`src/app/render/theme/surface_table.rs`)
whose rows are identities, not screens: each name is a structural position and each row carries a
nesting level, a focus source, a `soft` flag and a resting value. The resolver takes the call site's
own focus bit (`surface_colors(surface, focused)`).

That design makes a geometry-scoped palette ask awkward in exactly one way. Widening or recolouring a
*row* reaches every geometry sharing that identity. The first attempt at this change edited
`MainContentBox`'s focused fill from the soft content-body value to the resting value; because the
Wide Browser pane, the Workspace box, the Queue panel and both library lists all resolve that row,
the edit leaked the library's tone everywhere and was reverted the same day (`0b0023c0` →
`e127177b`). Two further facts already existed in the tree and constrain the answer:

- The non-Wide skeleton (`components/library_panel/narrow.rs`) paints no body fill at all: the shell
  fills the library placement with the library column's fixed backdrop. The panel's Selector spacer
  row, the status band's padding rows and the list's scrollbar column each painted their own
  identity over parts of that region.
- The list painter already had a two-tone seam: a paint policy carrying a focused/unfocused zebra
  pair, and a claim rect distinct from its row-flow rect.

The user wanted, for a focused non-Wide library, a body tone distinct from the inset list's, an inset
list that is one surface including its own spacer rows and scrollbar column, and a visible zebra
alternation in both focus states. Wide had to be untouched.

## Decisions

**D1 — Geometry-scoped appearance is a named identity, never a table-value edit.** A geometry-scoped
appearance is expressed by adding a purpose-named surface row (here `NarrowLibraryBody`) or a
geometry's own paint policy, and resolving it at that geometry's call site. Shared rows keep their
values. This is the property that makes the change safe: no existing row's value moves, so no other
surface can regress. `NarrowLibraryBody` is a `ColumnPane` row with `FocusSource::LibraryColumn`, the
level's focused fill while focused, and the app backdrop while resting (declared in
`RESTING_DEVIATIONS`) — i.e. a focused non-Wide library body is the same tone the Wide panels use, and
a resting one is byte-identical to what the column painted before.

**D2 — One authority, two guards.** `Model::library_body_fill` is the single place the non-Wide body
is decided. It is guarded by the shared breakpoint predicate (`wide_hero_fits`) and by
`PanelFocus::Library`. Four call sites consume it instead of inventing a fill: the shell's placement
fill, the status band's padding rows, the Selector spacer row (threaded through
`paint_selector_row`/`paint_pill_row_gap`, which now take the owning panel's identity and focus bit),
and the list's own body. Because the panel body fill is resolved from the same predicate the panel
itself uses for its skeleton, the shell and the panel cannot disagree about which geometry is being
painted.

**D3 — The list owns the surface under it.** `WideMediaListPaintPolicy` gained an optional body fill
(`with_body`). When set, the painter fills its *claim* rect with it before painting rows, and the
scrollbar column resolves that same fill rather than the selected-row punch-through. The non-Wide
policy (`PanelListPaintPolicy::Narrow`) sets it; `Wide` and `WideWorkspace` leave it unset, so Wide
and the Queue keep the surface their callers already painted.

**D4 — The claim is the panel; the row flow is inset.** The list claims the whole list panel and
takes its rows from a flow inset by `PANE_PAD_Y` (one row) above and below. The spacer rows therefore
belong to the inset's surface instead of showing the panel body through. Retained geometry, hit
resolution, menu placement and row arithmetic all read the inset — the rect the rows actually
occupy — so ADR 0024's "geometry resolved is geometry painted" still holds.

**D5 — The non-Wide stripe is the library-panel pair.** The stripe resolves
`Surface::LibraryPanel` — focused `#3c4841`, resting `#333c43` — while the list body is the
`MainContentBox` pair (`#48584e` focused, `#2d353b` resting). The existing invariant that a stripe
never equals its own list-box fill holds in both focus states, and the stripe identity is the same
one the Wide Workspace list already uses, so the library's stripe vocabulary stays one pair.

**D6 — Mini follows the non-Wide presentation**, focused fills included, because in Mini the narrow
view is the focused one. There is no Mini-specific paint bit. This supersedes the earlier
same-session assumption that Mini should stay on resting colours.

## Alternatives considered

- **Edit the shared table value** (`#48584e` → `#333c43` on `MainContentBox`) — rejected: leaks to
  every geometry sharing the identity; tried and reverted (`0b0023c0`, `e127177b`).
- **A Mini-specific `focused=false` paint bit** — rejected: it asks the panel and the list, which
  each compute their own focus input, to agree on a geometry override that neither owns. D6 removes
  the need.
- **Guard only in the shell** — rejected: the panel's spacer and the list's body would resolve
  different inputs than the placement fill, so a focused narrow panel would show one tone in its
  margins and another in its list.
- **Make the spacer rows a chrome band of their own** — rejected: the spacer is the panel showing
  through; giving it its own identity is what produced three fills for one region in the first place.
- **Dismiss or special-case the list in the status band** — rejected: the band's padding rows are
  library column, not status bar. The status row itself remains the status bar's own surface.

## Risks and mitigations

- *Wide regressions* — the Wide paths name different identities (`LibraryColumn`, `PillRowGap`,
  `Wide`/`WideWorkspace`) and there is no path from Wide geometry to `NarrowLibraryBody`; the
  surface-conformance and per-geometry characterization tests pin the Wide values.
- *A new surface row that is never painted* — `NarrowLibraryBody` is listed on the surface
  conformance residual list with its reason (it is painted by the shell's placement fill, not by a
  component view) and its pair is pinned by `pinned_fills` in `surface_resolve`.
- *Resting appearance drifting* — the resting tone is a declared deviation, so
  `resting_values_are_default_or_declared` fails if it drifts rather than hiding.
- *Manual verification remaining* — live Wide/Narrow inspection on a real terminal is still the
  acceptance evidence for the palette; the automated suites prove resolution, geometry and routing,
  not appearance.

## Evidence

- Palette authority: `src/app/shell_library_panel.rs` (`Model::library_body_fill`),
  `src/app/shell_chrome_panels.rs` (status band padding).
- Identity and pinning: `src/app/render/theme/surface_table.rs` (`Surface::NarrowLibraryBody` row and
  its deviation), `src/app/render/theme/surface_resolve.rs` (`pinned_fills`).
- List body and stripe: `src/app/components/library_panel/panel_list.rs` (`PanelListPaintPolicy::Narrow`
  arm), `src/app/components/media_list/mod.rs` (`with_body`, `body_bg`),
  `src/app/render/components/media_list/wide.rs` (claim fill, scrollbar column).
- Inset and spacers: `src/app/components/library_panel/narrow.rs`.
- Landed commits on `feat/replace-inline-hero-with-library-hero-overlay`: `2cc73469` (authority +
  guards), `545baa7d` (stripe pair), `1f086fc4` (spacers inside the inset), `e235ffa2` (test
  contracts). Superseded first attempts: `92feb78f`, `4535af6c`.
