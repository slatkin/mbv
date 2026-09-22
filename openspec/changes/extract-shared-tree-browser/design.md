# Design

## Context

See `proposal.md` — Why. PR #759 introduced shared row-flow traits under `src/app/components/list/`, but the complete nesting control remains spread across `music_tree.rs`, `music_tree_model.rs`, `music_tree_selection.rs`, `music_tree_view.rs`, and `music_tree_label.rs`. `MusicTreeBrowser` still owns the `tui-treelistview` model/query/state, filter session, interaction operations, rendering, and Music-specific arena.

`tui-treelistview` 0.2.2 requires its node identifier to be `Copy + Eq + Hash`, while mbv's stable destination targets commonly contain `String` and cannot serve directly as that identifier. The current monotonic `usize` arena is therefore still useful, but it belongs inside the shared component rather than inside Music. The existing `RowFlow`, `Cursored`, `Viewported`, `PaintRetained`, `MarkSelection`, and `Expandable` contracts remain the shared mechanics below the complete component.

The Library panel already erases list shape through `PanelList`, but it has a concrete `impl PanelList for MusicTreeBrowser`. That concrete implementation is direct evidence that the tree is not reusable yet.

## Goals / Non-Goals

**Goals:**

- One generic embedded `TreeBrowser<Target>` implements TuiRealm `Component` and is the complete nesting counterpart to the flat `WideMediaList<Target>` component, owning all reusable tree state, operations, filtering, and presentation state while delegating pixels to one shared Render Component.
- A destination adopts it by projecting plain typed node data and translating stable-target intents.
- Grouped Music becomes the first consumer with no Music-specific browser, model, renderer, or tree state owner remaining.
- The Library panel implements `PanelList` once for the shared tree type.
- The generic boundary is proven without adding a second production destination.

**Non-Goals:**

- No TV adoption or TV data design.
- No Hero, Workspace, shell-effect, playback, or routing changes.
- No changes to the flat `MediaList` shape.
- No new tree behavior or visual variant.
- No `tui-treelistview` upgrade or new dependency.

## Decisions

### D1: `TreeBrowser<Target>` is the complete owner, not another trait seam

Create the shared Interactive Component under `src/app/components/list/tree_browser/`. `TreeBrowser<Target>` owns the internal arena, target-to-node interning map, roots and children, model revision, `TreeQuery`, `TreeListViewState<usize>`, filter session and anchor, shared mark carrier, marquee clock, focus, configured geometry, and retained paint state.

`TreeBrowser<Target>` implements `tuirealm::Component`. It remains embedded and is never mounted, focused independently, subscribed, or assigned a `ComponentId`. Its `Component::view` is the only interactive view entry point and delegates to the destination-neutral TreeBrowser Render Component under `src/app/render/components/tree_browser/`; no inherent or destination-owned alternative view entry point exists, and no destination calls the Render Component directly.

The existing list traits remain implementation tools used by `TreeBrowser`; they are not the destination-facing deliverable. A destination holds one `TreeBrowser<DestinationTarget>` field and cannot supply or replace its state, query, renderer, painter, or behavior.

Alternative considered: add more traits for Music's model, renderer, and interaction code. Rejected because it would reproduce PR #759's failure: shared interfaces with a destination-owned browser implementation.

### D2: Destinations reconcile plain node projections

The destination-facing content type is a plain ordered node projection, conceptually:

```rust
TreeNode<Target> {
    target: Target,
    parent: Option<Target>,
    title: String,
    search_text: String,
    trailing: Option<TreeTrailing>,
    semantic_state: MediaSemanticState,
    mark_policy: TreeMarkPolicy,
}
```

`Target: Clone + Eq + Hash` is the stable identity. Targets unique only within a parent include that parent identity in their own type, as `MusicTreeTarget::Track { album, track }` already does. Root and sibling order come from projection order. The shared component validates that every parent is present, rejects duplicate targets deterministically, interns targets to private `usize` node ids, and atomically rebuilds roots/children when the projection changes.

`TreeMarkPolicy` is a small closed per-node semantic value: directly markable, aggregate over markable descendants, or excluded. It lets Artist roots aggregate Album marks while Track rows remain excluded without embedding Music concepts or callbacks in the browser. `TreeTrailing` carries optional right-aligned metadata text; the component owns its width, spacing, clipping, and paint. A destination supplies only `TreeNode<Target>` values and translates emitted stable-target intents; it supplies no trait implementation, callback, closure, model/query/state object, renderer, painter, navigation rule, filter matcher, aggregation algorithm, or action-order policy.

Reconciliation accepts only an acyclic forest. Duplicate targets, missing parents, self-parenting, and cycles return a typed reconciliation error and leave content, selection, expansion, marks, viewport, and retained geometry unchanged.

Alternative considered: make destinations implement `tui_treelistview::TreeModel` or a new model trait. Rejected because the destination would still own model behavior and tree-library types would cross the boundary. Alternative considered: use stable targets as the crate node id. Rejected because the pinned crate requires `Copy`, which ordinary destination targets do not satisfy.

### D3: The shared component exposes operations and transitions, not raw state

`TreeBrowser` accepts a closed set of semantic operations for movement, paging, first/last, parent/child traversal, expansion toggle, activation, context intent, marking, pointer-resolved selection, and filter editing. One operation returns one transition containing independent facts: consumed disposition, selected-target change, mark-summary change, and optional provider-neutral external intent carrying stable targets.

Except for construction, atomic reconciliation, Panel configuration, and read-only stable-target queries, every destination-triggered tree mutation enters through `apply(TreeOperation<Target>) -> TreeTransition<Target>`. No destination-visible movement, expansion, filter, mark, pointer, activation, context, or alternate view mutator may bypass that operation surface.

The shared component decides tree-local behavior: movement, parent/child traversal, branch expansion/collapse, point resolution, filter lifecycle, selection and mark mutation. The destination translates only external intents into its typed `Msg` and resolves targets against its retained domain snapshot. It may not sequence private tree mutators to recreate an operation.

Music-specific persistence and prefetch notifications remain destination translations of selected stable targets. Album, Artist, and Track activation effects remain Music-owned; the browser reports the resolved target and whether it is a branch/leaf through stable semantic output rather than exposing its model.

Alternative considered: preserve the current wide method surface and merely rename `MusicTreeBrowser` to `TreeBrowser`. Rejected because methods such as `selected_artist_album_targets`, `track_identity_of`, and `model_is_track` encode Music and would leave the supposedly shared component destination-specific.

### D4: Filtering is shared component state over destination search text

The component retains the current fuzzy filter session, query text, anchor, forced expansion, dismissal restoration, and hidden-mark behavior. Filtering matches each node's supplied `search_text`; destinations do not supply matcher callbacks or a second result list. `PanelList::search_bar` reads the shared filter session.

This preserves Grouped Music behavior while making filtering available to any later adopter without copying Music code. It does not unify flat Inline Search; that remains a separate control.

### D5: One fixed tree presentation is shared

Move the current tree painter and label renderer into one destination-neutral Render Component under `src/app/render/components/tree_browser/`. `TreeBrowser` owns all presentation state and is the sole interactive view entry point; its `Component::view` delegates widget composition and painting to that Render Component. The presentation remains fixed:

- plain-space hierarchy with no state glyphs;
- depth-derived semantic title roles;
- canonical focused selected-row bar;
- aggregate-mark roles;
- group-relative zebra stripes;
- focused selected-title marquee;
- optional per-row trailing metadata gutter;
- parent-owned text insets and full-width selection claim;
- shared focus-gated scrollbar;
- retained latest-frame target geometry.

Destinations provide semantic row data, never colours, `Style`, glyphs, gutter widths, rendering callbacks, or direct Render Component calls. The Library panel supplies only geometry and focus through a generic `impl<Target> PanelList for TreeBrowser<Target>`, which invokes `TreeBrowser` through `Component::view`.

Alternative considered: a style/config object so Music can preserve its appearance. Rejected because it would make the existing presentation a caller-selected variant with one user. The current Music style becomes the shared tree style, which is also the required style for later adopters.

### D6: Music becomes projection and translation only

`MusicTreeTarget` remains Music's closed stable-target type. `MusicContent` projects Artist, Album, and cached Track nodes into `TreeNode<MusicTreeTarget>` values and reconciles its `TreeBrowser<MusicTreeTarget>`. It retains full `EmbyItem` snapshots needed to resolve playback, context menus, persistence, artwork, and prefetch.

Delete `music_tree.rs`, `music_tree_model.rs`, `music_tree_selection.rs`, `music_tree_view.rs`, and `music_tree_label.rs`. Relocate only `MusicTreeTarget` and plain projection/target-resolution helpers. `MusicContent` contains exactly one tree-control field, of concrete type `TreeBrowser<MusicTreeTarget>`. Music production modules contain no tree model/query/state, expansion/filter/viewport/mark/geometry/marquee carrier, tree painter, renderer, or forwarding browser API, regardless of its name.

The extraction is incomplete if any Music-owned wrapper or equivalent responsibility survives, or if a production or test module outside `src/app/components/list/tree_browser/` and `src/app/render/components/tree_browser/` imports, names, constructs, or asserts against a `tui_treelistview` type.

### D7: Reuse is proven by a generic fixture and deletion evidence

A focused shared-component test instantiates `TreeBrowser` with a small non-Music target enum and a three-level hierarchy through `Component::view`. It proves movement, paging, first/last and parent/child traversal; persistent versus filter-forced expansion; ordered marks, aggregation, exclusions, and display-order intents; atomic reconciliation and stable-identity preservation; one operation returning all independent transition facts; completed-frame point resolution and invalidation; and indentation, depth roles, zebra, selected/aggregate bars, metadata gutter, marquee, and scrollbar. This fixture is test data, not a second production destination.

Existing Music component, buffer, and mounted tick tests remain the behavior backstop and are updated only where they name deleted implementation types. Tests that inspect private Music arena details are deleted or replaced by assertions through the shared component's stable-target surface. No duplicate test is added when an existing Music test already proves the preserved behavior.

Acceptance also includes structural deletion evidence: `MusicContent` has exactly one tree-control field of concrete type `TreeBrowser<MusicTreeTarget>`; the five Music tree implementation files are deleted; Music production modules retain no tree-control responsibility under any name; and no production or test module outside the two shared TreeBrowser implementation modules references `tui_treelistview`.

## Risks / Trade-offs

- **A nominal generic type could still hide Music policy.** → The non-Music three-level fixture and the prohibition on Music types/imports in the shared module prove the boundary directly.
- **Moving the painter can accidentally alter pixels or hit geometry.** → Preserve the existing focused Music buffer tests and latest-frame pointer tests; make extraction commits behavior-neutral.
- **A large all-at-once move would obscure whether ownership actually changed.** → Introduce the shared data/owner surface first, move behavior and painting into it, switch Music, then delete the old implementation before acceptance.
- **Generic node projection duplicates full destination objects.** → Project only stable targets and row presentation data; destinations retain domain objects for effects.
- **The current pinned crate constrains internal ids to `Copy`.** → Keep the private monotonic arena; never expose it or make destinations adapt to it.

## Migration Plan

1. Introduce the shared node projection and `TreeBrowser<Target>` owner with a non-Music fixture.
2. Move tree operations, filtering, rendering, and `PanelList` integration into the shared module without changing Music behavior.
3. Convert Music to project nodes and translate shared transitions.
4. Delete the Music-specific browser/model/filter/renderer/state implementation and simplify tests to the shared stable-target surface.
5. Run focused shared-tree and Music tests, then the repository's full verification gate.

Rollback is a normal commit revert: this change alters no persisted data, protocol, Service API, or user-visible behavior.
