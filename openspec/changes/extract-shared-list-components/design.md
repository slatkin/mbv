# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- **Two live implementations, both shipped.** `media_list/` has nine destination
  consumers; `music_tree*.rs` has one. Neither can regress. All behavior
  verification is through the existing unit/buffer/tick-integration suites; the
  repo forbids live or smoke tests.
- **The tree's machinery is not ours.** `tui-treelistview` 0.2.2 is pinned, and
  `music_tree.rs` is explicitly documented as the one boundary to it. The seam
  must sit *above* that boundary — if seam types leak crate types, the flat list
  ends up depending on a tree crate.
- **The crate has no non-selectable node.** `TreeModel` is `roots()` +
  `children()` + `revision()`; every projected node is selectable, expandable
  and hit-testable. The flat list's `Heading`/`Spacer` have no crate counterpart.
  This matters for TV later, but the seam must define structural rows now or the
  tree shape cannot express them at all.
- **mbv already owns tree movement.** The crate keymap is disabled and
  `MusicTreeBrowser` maps its own chords. So cursor policy is already ours on
  both sides — the seam is unifying code we control, not overriding a dependency.
- **File sizes.** `media_list/mod.rs` (~30K), `music_tree.rs`, `music_tree_model.rs`,
  `music_tree_selection.rs` and several test files sit at or over the 800-line
  cap. Splitting is forced by this change, not optional.

## Goals / Non-Goals

**Goals:**

- One implementation of cursor, viewport, retained geometry, point resolution
  and ordered marks, with both shapes as thin consumers.
- Flat and tree substitutable at the destination boundary — a destination should
  differ only where its flow genuinely nests.
- A net deletion. The change is only worth shipping if the duplicated
  arithmetic leaves the tree and flat implementations.

**Non-Goals:**

- No new list capability. Anything neither implementation does today stays
  undone; the seam is shaped by what exists, not by what a future destination
  might want.
- No filtering work. Both filter architectures stay exactly as they are, and the
  seam is not pre-shaped to accommodate their later unification.
- No TV migration, and no second tree destination.
- No change to Grouped Music's observable behavior, including its deliberate
  divergences (its mark aggregation differs from the crate's on purpose, for
  hidden filtered children — that divergence is preserved, not "fixed").

## Decisions

### D1: Composed traits over one row-flow abstraction, not a single list trait

`RowFlow<Target>` is the keystone: ordered rows, some selectable, each selectable
one carrying a stable target. Over it sit `Cursored`, `Viewported`,
`PaintRetained`, `MarkSelection`, and (tree only) `Expandable`.

Rationale: the three middle traits are pure arithmetic over
`(len, selected_row, offset, area)`. Expressed as default methods on top of
`RowFlow`, they require *zero* per-shape code — which is where the deletion
comes from. A single fat `ListComponent` trait would instead force each shape to
implement or stub every method, producing the parallel-abstraction outcome this
change exists to avoid.

*Alternative considered — generic struct with a row-source parameter
(`List<S: RowSource>`) instead of traits.* Rejected: the tree's state lives
inside `TreeListViewState` from the pinned crate and cannot be moved into an
mbv-owned struct field without reimplementing projection. Traits let the tree
keep the crate's state and satisfy the contract by delegation; a generic struct
would demand ownership the tree cannot give.

### D2: The seam is generic over `Target`; internal handles never appear in it

`RowFlow` is generic over the destination's stable target type — already varied
in practice (`String`, `QueueSlotId`, `PodcastEpisodeTarget`). The tree's arena
`usize` and `MusicNodeKey` become private; `MusicTreeBrowser` gains an internal
target↔node map and converts at its boundary.

Rationale: this is what makes the two shapes substitutable, and it aligns the
tree with the rule the repo already applies to messages — stable opaque
identities, never positions. The arena index is the tree's version of a
coordinate.

*Cost, stated plainly:* this is the riskiest part of the change. Roughly 1,200
lines of music tree tests assert against arena indices, and the conversion is
where latent bugs will surface (a target present in the model but absent from
the current projection is currently expressible as a node id and must become an
explicit absent case). Per the repo's standing rule on migrations, those
assertions are **deleted and rewritten against the target surface**, not
translated case by case.

### D3: Structural rows are a seam concept, expressed per shape

The seam defines rows as selectable-with-target or structural-without. The flat
shape already has this (`Heading`/`Spacer`). The tree shape must synthesize it,
because the crate cannot: a structural row is a node the tree marks
non-selectable in its own projection, and cursor movement — already ours —
skips it.

Rationale: defining it in the seam now costs nothing (the flat shape needs it
regardless) and is the only thing that makes TV's alphabet headings expressible
later without reopening the seam. Defining it *only* in the flat shape would
guarantee reopening it.

This is the one place the seam is shaped by a known near-term need rather than
purely by existing code. It is included because the flat shape independently
requires it, not on TV's behalf alone.

### D4: Paging is an explicit policy hook, not a shared default

Flat lists do `Heading`-based group jumps; `grouped-music-tree-browser`
explicitly forbids the tree inheriting them and pages the visible-node viewport
instead. Both are correct for their shape.

Rationale: a shared default would silently regress one of them. Naming it as a
policy keeps the divergence deliberate and visible. This is the only behavioral
hook in the seam — everything else is genuinely common.

### D5: Mark aggregation belongs to `Expandable`, not `MarkSelection`

`MarkSelection` owns ordered membership, which both shapes have.
Parent-rolls-up-children `Partial` state only exists where there are children.

Rationale: keeps `MarkSelection` implementable by a flat list with no stub, and
keeps music's deliberate divergence from the crate's aggregation (for hidden
filtered children) local to the shape that has the concept.

### D6: Paint invalidation unifies on the explicit call, not the flag

The two implementations spell this differently: `WideMediaList::invalidate_paint()`
versus the tree's `paint_complete` bool cleared at three separate sites.
`PaintRetained` takes the explicit form.

Rationale: three clear sites is how the tree's version drifts. One call is
auditable. This is a small change that happens to be the most concrete evidence
in the change that the duplication was already going wrong.

## Risks / Trade-offs

- **The change has no user-visible payoff until TV adopts it, so "done" is easy
  to fake.** → The acceptance gate is the net deletion in task group 4, not the
  existence of the new traits. If removing the duplicated arithmetic from both
  implementations does not come out substantially net-negative, the seam is a
  parallel abstraction and should be reverted rather than shipped.
- **The arena→target conversion is where real bugs are.** → Convert the tree
  *after* the flat shape is already green against the seam, so a failure is
  unambiguously attributable to the conversion. Keep the music tick-integration
  suites as the behavioral backstop, since they exercise composition rather than
  direct method calls.
- **Deleting ~1,200 lines of index-based tree tests could silently drop
  coverage.** → Gate on coverage of the new target surface (`cargo llvm-cov`
  over the tree module) rather than on case-for-case correspondence with the
  deleted tests. Coverage not meeting the pre-change level for that module is a
  blocker.
- **Forced file splits inflate the diff and obscure review.** → Split in its own
  task group, mechanically, before behavior changes land, so the substantive
  diffs read cleanly against already-split files.
- **`canonical-media-lists` is being partially superseded while the rest of it
  is known-stale.** → This change removes only the three requirements the seam
  demonstrably takes over and modifies one. Auditing the remainder is explicitly
  left out; it is a larger question than this refactor should absorb.

## Migration Plan

Sequenced so each step is independently verifiable and a failure is attributable:

1. **Mechanical file splits.** No behavior change; existing suites green.
2. **Seam introduced; flat shape implements it.** `media_list/` satisfies the
   traits. Externally observable behavior unchanged — the canonical suites and
   every destination's tests are the proof.
3. **Tree shape implements it; arena id goes private.** `MusicTreeBrowser`
   converts to targets; its index-based tests are deleted and rewritten. Music
   buffer/characterization and tick-integration suites are the proof.
4. **Delete the duplicated arithmetic from both.** The gate. Net-negative diff
   required.

Rollback: steps are independently revertable, and the seam is additive until
step 4. Reverting step 4 alone leaves a working but redundant system; reverting
steps 2–4 restores the current architecture without touching destinations.

## Open Questions

- Whether `MarkSelection` needs a visual-mode representation in the seam or
  stays flat-only. The flat list has visual mode; the tree has no equivalent
  today. Deferrable because it changes no spec requirement and no task boundary
  — if the tree never grows visual mode, it stays a flat-shape concern, and if
  it does, it widens the trait then.
