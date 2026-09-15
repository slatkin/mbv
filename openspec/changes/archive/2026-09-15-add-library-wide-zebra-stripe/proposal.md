## Why

Queue zebra striping (change `add-media-list-zebra-stripe`, #719, merged at `8a9132c2`, released in
v0.19.3) proved the pattern: alternating row backgrounds carry the eye across a wide panel. The
library's Wide lists have the same dense one-column rows and the same readability problem, in both the
Browser pane and the provider Workspace.

Meanwhile #719 shipped the gutter-accent selected row — bold focus-accent title, no selected-row
background — behind a per-list opt-in flag whose only user is the Queue. A caller-selected variant arm
whose only user is one screen is a defect to remove, not extend (`openspec/specs/ui-design-system/spec.md:85`,
and the `mbv-frontend` decision table says the same). The follow-up should stripe the library and delete
the option in the same motion.

## What Changes

- Enable zebra striping on both library Wide lists, each with the pair its own list box does **not**
  paint with, resolved through the surface table like the Queue site does:
  - Browser-pane list (box = `LibraryPanel`) stripes with the `MainContentBox` pair — focused `#48584e`,
    unfocused `#2d353b`;
  - provider Workspace list (box = `MainContentBox`) stripes with the `LibraryPanel` pair — focused
    `#3c4841`, unfocused `#333c43`.
- Make the gutter-accent selected row the unconditional Wide treatment: delete `with_selected_gutter`,
  simplify the Queue call site to zebra-only, and adopt no flag on the library side.
- Multi-selected Wide rows take the same accent as the selected row (Queue parity), replacing the
  library's current selected-row background fill.
- Parity and the stripe/selection interaction inherit #719 **as merged**: 1st/3rd/5th visible items are
  striped, and a selected row keeps its stripe under the accent. No new behaviour decision here.
- Out of scope: the Narrow/Inline presentation and its selected-row background; Inline Search results
  and the other legacy `list_rows.rs` painters, which stay as they are until #720 converts them.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `canonical-media-lists`:
  - ADDED — the two library Wide lists stripe, each with its contrasting pair, and a stripe never equals
    its own panel body.
  - MODIFIED — *WideMediaList owns fixed-row mechanics*: the gutter accent goes from "a policy MAY
    select" to the only Wide selected-row treatment, and the owning-surface identity's remaining
    scrollbar-backing role is stated.
  - ADDED — *Wide presentation zebra striping*, and REMOVED — *Wide presentation supports optional zebra
    striping*: the parity, counting, containment, and geometry claims carry over unchanged, but the
    selected-row-background override and its scenario become unreachable once every Wide list uses the
    accent. Both are rewritten as one block, because a MODIFIED block cannot drop a scenario the main
    spec still carries (`openspec validate --strict` refuses it).

## Impact

- `src/app/components/library_panel/panel_list.rs` — both Wide policy arms gain their zebra pair through
  `palette::surface_colors` (single seam, two pairs).
- `src/app/components/media_list/mod.rs` — `WideMediaListPaintPolicy` loses `with_selected_gutter`,
  `selected_gutter`, and its accessor.
- `src/app/render/components/media_list/wide.rs` — the Wide adapter passes the accent unconditionally;
  `media_list_row`'s parameter is renamed `gutter_accent`; comment at the `selected_bg` argument noting
  the Wide path consumes it for the scrollbar backing only.
- `src/app/render/components/media_list/row.rs` — stale doc comment about a panel-edge marker corrected.
- `src/app/render/components/queue.rs` — drops `.with_selected_gutter()`, keeps its zebra pair.
- Tests: `panel_list.rs` policy test re-pinned to the accent and given a surviving owner proof for the
  Browser/Workspace distinction; `render/components/media_list.rs` Wide selection and multi-selection
  tests converted to the component adapter; new library render regressions for both arms' stripes and
  the accent; `tests_surface_conformance.rs` coverage-table citations corrected.
- `CONTEXT.md` — new Presentation terms **Zebra stripe** and **Gutter accent**.
- No palette, `Cargo.toml`, or dependency changes.
