---
status: accepted
---

# Geometry-Scoped Surface Appearance Is A Named Identity

## Problem

The render tree resolves every painted fill through one closed surface table
(`src/app/render/theme/surface_table.rs`). A row names a *structural position* — the library content
box, the queue panel, a pill row — not a screen or a geometry, and the one resolver takes the call
site's own focus bit: `surface_colors(surface, focused)`.

That shape is what keeps surfaces consistent: the same position resolves the same appearance
everywhere it appears. It also makes one class of request awkward. A request scoped to a *single
geometry* — "in the narrow panel the list body should be this tone, but leave Wide alone" — cannot be
satisfied by editing a row's value, because the row is shared by every geometry that paints that
position. The failure is silent in review and loud on screen: the palette moves on surfaces nobody
asked about.

That is not hypothetical. The first attempt at the non-Wide library palette edited
`Surface::MainContentBox`'s focused value, which the Wide Browser pane, the Workspace box, the Queue
panel and both library lists all resolve. The tone leaked into every one of them (`0b0023c0`) and was
reverted the same day (`e127177b`). The second attempt, months of habit away, was the same mistake in
a different row.

## Decision

A geometry-scoped surface appearance is expressed in one of two ways, and never by changing a shared
row's value:

1. **A purpose-named surface row** for a position that exists only in that geometry (or resolves a
   genuinely different appearance there), placed in the table with its level, focus source and
   resting value, and resolved at that geometry's own call site. Its resting value is a declared
   deviation when it differs from its level's default, so drift fails a test instead of hiding.
2. **A geometry's own paint policy** where the difference is which *content* the geometry paints
   rather than which position it occupies — for example an optional body fill carried by a list's
   paint policy, set by that geometry's arm and unset everywhere else.

The call site resolves the identity with the focus input it already computes. No geometry-specific
value may be reached from another geometry's paint path, and no shared row's pinned pair may move to
satisfy a geometry-scoped request.

## Rationale

- **Blast radius is the whole argument.** A named identity changes exactly the surfaces that name it,
  and every other row keeps its pinned value, so "Wide is unchanged" is a property of the code's
  reachability rather than a claim about testing.
- **The table stays the single vocabulary.** Adding a row is reviewed like any other vocabulary
  addition, with a reason and a resting deviation; editing a value is a one-line change that no test
  failure distinguishes from an intended global palette move.
- **Focus stays a call-site input.** The resolver's contract already requires the caller's own focus
  bit. A geometry-scoped identity keeps that contract instead of introducing a second, geometry-aware
  focus source inside the theme.

## Consequences

- Geometries with genuinely different palettes will accumulate purpose-named rows. That is the
  intended cost: each row is a reviewed, pinned, documented decision rather than an invisible drift.
- Where the difference is *which content* a geometry paints, the geometry's own paint policy carries
  it (the list body fill), keeping the table free of rows that would only ever be used by one caller.
- A geometry-scoped difference must be provably unreachable from the geometries it does not target;
  reviewers should ask for that reachability argument, not just a green suite.
- The first attempt at the non-Wide library palette (`0b0023c0`, reverted by `e127177b`) is the
  worked counter-example this ADR cites; the accepted implementation was change
  `add-non-wide-library-palette` (`Surface::NarrowLibraryBody` plus `PanelListPaintPolicy::Narrow`).
  That identity was later retired by `unify-narrow-library-with-wide-browser-pane` once the non-Wide
  library became the Wide browser pane without a Hero, so no non-Wide palette identity remains. The
  Decision above still stands and governs any future geometry-scoped appearance.

## Considered options

- **Edit the shared row's value and accept the global move** — rejected: it changes surfaces the
  request did not name, and was reverted twice in practice.
- **A runtime geometry argument on the resolver** (`surface_colors(surface, focused, geometry)`) —
  rejected: it multiplies the table's state space, makes every row's appearance geometry-dependent,
  and loses the property that a row is one position with one declared appearance.
- **A per-geometry palette table duplicated from the shared one** — rejected: two tables drift, and
  the duplicated rows would restate values that already exist once.
- **Bespoke painters per geometry** — rejected: the geometry difference here is a fill, which the
  surface vocabulary can already express; a bespoke painter would also take ownership away from the
  panel that composes the surface.

## References

- `openspec/changes/add-non-wide-library-palette/` (proposal, design, delta specs)
- `openspec/changes/unify-narrow-library-with-wide-browser-pane/` (the change that retired
  `NarrowLibraryBody` and `PanelListPaintPolicy::Narrow`)
- `openspec/specs/ui-design-system/spec.md` (the render-tree boundary and role vocabulary)
- `docs/architecture/interactive-surface-ledger.md` (surface ownership)
