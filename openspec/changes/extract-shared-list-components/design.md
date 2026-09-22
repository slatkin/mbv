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
  `children()` + `revision()`; every projected node is selectable and
  hit-testable. The seam supports structural rows because the flat list already
  requires them, but this change does not synthesize unsupported structural
  nodes in the current tree. A future tree that needs them must first establish
  a viable dependency boundary.
- **mbv already owns tree movement.** The crate keymap is disabled and
  `MusicTreeBrowser` maps its own chords. So cursor policy is already ours on
  both sides — the seam is unifying code we control, not overriding a dependency.
- **File sizes.** `media_list/mod.rs` is currently the only governed production
  file over the 800-line cap. It is split mechanically before behavior changes;
  tree files are split only if this implementation would push one over the cap.

## Goals / Non-Goals

**Goals:**

- One implementation of cursor and viewport arithmetic, retained geometry,
  point resolution, and ordered-mark storage, with each shape supplying only
  the primitive access needed to adapt its existing state owner.
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

The traits separate shared algorithms from shape storage. `Cursored` requires
primitive selected-target read/write operations; `Viewported` requires offset
read/write operations; their default methods own movement, clamping, and
keep-visible arithmetic over `RowFlow`. The flat adapter mutates its existing
fields, while the tree adapter delegates those primitives to
`TreeListViewState`. `PaintRetained` and `MarkSelection` use shared state carriers
held by each owner, replacing `WidePaintResult`/`paint_complete` and each shape's
ordered-membership vector. These small adapters are required per shape; the
arithmetic and retained-state implementations are not duplicated.

Rationale: traits preserve the tree crate's state ownership without pretending
Rust traits can own fields. A single fat `ListComponent` trait would instead
force each shape to implement or stub every method, producing the
parallel-abstraction outcome this change exists to avoid.

*Alternative considered — generic struct with a row-source parameter
(`List<S: RowSource>`) instead of traits.* Rejected: the tree's state lives
inside `TreeListViewState` from the pinned crate and cannot be moved into an
mbv-owned struct field without reimplementing projection. Traits let the tree
keep the crate's state and satisfy the contract by delegation; a generic struct
would demand ownership the tree cannot give.

### D2: The seam is generic over `Target`; internal handles never appear in it

`RowFlow` is generic over the destination's stable target type — already varied
in practice (`String`, `QueueSlotId`, `PodcastEpisodeTarget`). The tree exposes
one stable `MusicTreeTarget` with `Artist(ArtistKey)`, `Album(String)`, and
`Track { album: String, track: String }` arms. Its arena `usize` and
`MusicNodeKey` become private; `MusicTreeBrowser` gains an internal
`MusicTreeTarget`↔node map and converts at its boundary.

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

### D3: Structural rows are a row-flow concept, used only where supported

The seam defines rows as selectable-with-target or structural-without because
the flat shape already has `Heading`/`Spacer`. The current tree projects only
selectable stable targets: `tui-treelistview` 0.2.2 has no non-selectable-node
API, and this change neither emulates one nor changes tree behavior for a future
destination.

Rationale: every shape can describe its existing flow without speculative
machinery. Supporting structural rows in a future tree is a separate dependency
and behavior decision, not hidden scope in this extraction.

### D4: Paging is an explicit policy hook, not a shared default

Flat lists keep their existing fixed selectable-row page distance. The tree's
paging was a fixed 5-row stride (`TREE_PAGE_ROWS`) when this change began, not
the visible-viewport paging named here — the `PagingPolicy::VisibleViewport`
variant shipped with no production adopter. This change aligns the tree to the
named visible-viewport policy rather than leaving the policy named but unwired
and this decision's premise false. That alignment is the one intentional,
user-approved behaviour change in this extraction: Grouped Music's PageUp/PageDown
stride changes from a fixed 5 rows to the visible viewport. Flat lists must
remain behavior-neutral; the tree does not.

Rationale: a shared default would silently regress one of them. Naming it as a
policy keeps the divergence deliberate and visible. This is the only behavioral
hook in the seam — everything else is genuinely common.

### D5: Mark aggregation belongs to `Expandable`, not `MarkSelection`

`MarkSelection` owns ordered membership, which both shapes have.
Parent-rolls-up-children `Partial` state only exists where there are children.

Rationale: keeps `MarkSelection` implementable by a flat list with no stub, and
keeps music's deliberate divergence from the crate's aggregation (for hidden
filtered children) local to the shape that has the concept.

### D6: Paint invalidation uses one shared retained-state implementation

The two implementations currently retain completion differently:
`WideMediaList` stores an optional `WidePaintResult`, while the tree combines the
crate's hit facts with a `paint_complete` flag. `PaintRetained` provides one
shared state carrier and explicit begin/finish/invalidate operations; shape
adapters populate it from the geometry each painter actually produced.

Rationale: one state transition model makes stale-frame behavior auditable while
leaving each painter responsible for producing its own geometry.

## Risks / Trade-offs

- **The change has no user-visible payoff until TV adopts it, so "done" is easy
  to fake.** → Acceptance requires direct evidence that each duplicated mechanic
  was removed from both shapes. Production line count is recorded as a
  diagnostic; a non-negative result triggers review and explanation for possible
  parallel abstraction, but no numeric reduction threshold decides correctness.
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

1. **Mechanical file split.** Split the over-cap `media_list/mod.rs`; split a
   tree file only if this implementation would otherwise push it over the cap.
   No behavior change; existing suites green.
2. **Seam introduced; flat shape implements it.** `media_list/` satisfies the
   traits. Externally observable behavior unchanged — the canonical suites and
   every destination's tests are the proof.
3. **Tree shape implements it; arena id goes private.** `MusicTreeBrowser`
   converts to targets; its index-based tests are deleted and rewritten. Music
   buffer/characterization and tick-integration suites are the proof.
4. **Delete the duplicated mechanics from both.** Confirm each shared mechanic
   has one production implementation; record line-count movement as a diagnostic.

Rollback: steps are independently revertable, and the seam is additive until
step 4. Reverting step 4 alone leaves a working but redundant system; reverting
steps 2–4 restores the current architecture without touching destinations.

## Open Questions

None. Visual-mode range/anchor state stays flat-only because the tree has no
such behavior today; `MarkSelection` shares only ordered stable-target
membership.
