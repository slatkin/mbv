## Context

See `proposal.md` for motivation. Grouped Music currently derives an atomically published `GroupedAlbumCatalog`, flattens it to `[Heading, Spacer, Item…]`, and projects the album flow through `MediaListCarrier<String>`. `MusicContent` owns album and track interaction but is already 756 lines, near the 800-line pre-push gate. The shell owns album-track fetching, image effects, and projection into the component.

`tui-treelistview` 0.2.2 matches mbv's Ratatui 0.30 generation. Its useful boundary is the combination of `TreeModel`, `TreeQuery`, `TreeListViewState`, current-render hit testing, label/column renderers, and style configuration. Its optional keymap is incompatible with mbv's single Keyboard Router and will remain disabled. Its nodes use compact copyable IDs while mbv's stable domain targets are owned strings.

The existing contracts conflict with the new surface in deliberate, narrow ways: Grouped Music artist labels are currently non-selectable canonical `Heading` rows; album browsing is required to use `MediaList`; and Inline Search is required to replace grouping with flat, relevance-ordered, full-corpus results. The delta specs make Grouped Music the only exception. Track Workspaces and all other destinations remain canonical media lists.

## Goals / Non-Goals

**Goals:**

- Keep one tree state owner across Wide, Narrow, Mini, and Library Hero overlay composition.
- Use the dependency's supported model/query/state/rendering seams rather than reimplementing its projection.
- Preserve mbv's Panel, Keyboard Router, mouse, semantic-theme, selected-row, and typed-effect boundaries.
- Keep new tree code outside `music_content.rs` so the existing file does not cross the 800-line gate.
- Make dependency rejection cheap and complete if final customization review fails.

**Non-Goals:**

- A generic `TreeMediaList`, recursive browsing, lazy child loading, tree editing, persisted expansion, or migration of another destination.
- Replacing canonical `MediaList` in either album-track or artist-track Workspaces.
- Relevance sorting, a full-library Grouped Music search, preloading every artist's tracks, or a bespoke renderer parallel to `TreeListView`.
- Changing the shared Panel geometry, breakpoints, or Service/Player authority.

## Decisions

### 1. Add one destination-specific tree owner

Add a sibling Grouped Music tree module under `src/app/components/` and keep `MusicContent` as the mounted event and typed-intent boundary. The module owns:

- a `MusicTreeModel` adapted from the current settled catalog;
- one `TreeListViewState` for selection, expansion, viewport, marks, projection cache, and hit map;
- the local filter session and pre-filter anchor;
- stable node interning and node-to-domain translation;
- tree view painting through the crate's widget and supported renderer/style interfaces.

`MusicContent` delegates eligible browser operations to this owner and continues to own active-pane focus, Hero/Workspace composition, query bar placement, and request translation. The shell projects settled catalog facts and accepts typed artist/album intents; it never reads tree cursor or expansion state.

Alternative rejected: extending `MediaList` with tree semantics. That would preserve fewer dependency benefits, spread parent/leaf cases through every canonical-list operation, and make the one-screen evaluation harder to remove. Alternative rejected: wrapping both a hidden `MediaList` and a visible tree. That creates two cursor and selection authorities.

### 2. Intern stable domain keys into monotonic node IDs

Use a destination-local arena:

```text
usize NodeId -> MusicNode::Artist(ArtistKey) | MusicNode::Album(AlbumTarget)
MusicNodeKey -> usize NodeId
```

`ArtistKey` is either a stable Emby artist item ID or a deterministic fallback grouping key. The parser retains the first applicable `ArtistItems` name/ID pair associated with the displayed album artist; the settled catalog carries both display text and this identity. `ArtistItems` IDs from album/item payloads are the sole stable artist ID space used for `ArtistIds` item queries and `/Items/{id}/Images`; IDs obtained from an `/Artists` listing SHALL never be mixed into these keys. If no ID exists, the fallback key derives from the settled grouping identity, not display position. Equal names with different IDs remain separate roots.

IDs are interned by semantic key, survive ordinary catalog replacement, and are not reused for a different key during the owner's lifetime. Removed entries are tombstoned or omitted from the model while preserving their interned mapping; the arena resets only when the retained Music destination changes identity. This favors simple, collision-free continuity over speculative compaction.

Alternative rejected: label IDs, which collide for equal artist names and fallback labels. Alternative rejected: hashing owned targets into `usize`, which introduces collision handling without reducing state.

### 3. Reconcile state from each atomic settled catalog

A model revision changes only when the settled catalog changes. Reconciliation proceeds by stable node key:

1. intern artist roots and album leaves in settled order;
2. replace the model's root/child projection atomically;
3. retain expansion, marks, and selected node that still exist;
4. apply the crate's selection fallback and viewport clamp only for missing nodes;
5. emit an album-selection persistence request only when the resolved selected album changes; artist focus does not overwrite album persistence with an artist ID.

Filter-query and expansion revisions remain separate from model revision. Responsive geometry updates configure the same owner and invalidate old hit geometry before rendering.

Alternative rejected: rebuilding IDs from row position, which makes selection and expansion jump on refresh.

### 4. Map mbv input explicitly; do not enable the crate keymap

The existing Keyboard Router keeps precedence. Once it returns local fall-through, `MusicContent` maps the confirmed chords to tree view actions. `/`, printable filter text, Visual mode, context actions, and track Workspace focus remain destination-local semantic operations. No crate keymap or second routing table receives terminal events.

The crate's latest-render `hit_test` resolves artist/album targets inside the browser rectangle. `MusicContent` keeps gesture recognition and sends already-resolved targets to local operations or typed requests, matching ADR 0024. Geometry is invalidated on content or area changes before the next view.

The active `group-aware-page-navigation` change remains unchanged for canonical Heading-bearing lists. Grouped Music stops using that owner and uses visible-node viewport paging supplied by the tree state.

### 5. Implement tree-local fuzzy filtering, not a second InlineSearch result owner

Reuse `SkimMatcherV2` and the existing 300 ms duration, but keep a small destination-specific filter session rather than generalizing `InlineSearch` prematurely. Each album leaf has precomputed searchable text containing artist, album title, and year. On debounce expiry, the filter records the matching album node IDs; the tree query retains ancestors and force-expands matching paths while leaving persistent expansion untouched. Scores are discarded after match/no-match classification, preserving settled order.

Opening the filter snapshots the selected node. Empty text disables filtering and shows the full tree. Dismissal restores the snapshot if present and persistent expansion. A filter revision intersects active tree multi-selection with visible album leaves so status, painting, and bulk actions never include hidden targets. Query changes perform no shell request.

Alternative rejected: adapting the current `InlineSearch` carrier. Its flat result owner, full-corpus fetch lifecycle, relevance order, and empty-result behavior are the semantics this exception replaces. Alternative rejected: extracting a generic search framework for one new caller.

### 6. Materialize artist operations from leaves at the component boundary

Artist roots never cross the shell as playable targets. For play, enqueue, shuffle, and context requests, the tree owner walks the root's settled child leaves and returns ordered album targets. Expansion does not affect action scope. An active filter restricts the walk to matching visible leaves.

Root multi-selection applies mark operations to the same visible child leaves. Aggregate root state is derived from child marks. Range selection walks visible album leaves and skips roots. The component emits existing ordered album-target intents with a stable tree-origin identity, allowing existing effect materialization and context capability intersection to remain downstream.

Alternative rejected: storing selected artist IDs. Their meaning changes with filtering and would force the shell to re-resolve component-local scope.

### 7. Add artist detail without giving the component Service authority

Before tree implementation, extend the Emby item projection to retain artist name/ID pairs and establish the `mbv-core` Audio-by-artist operation using `ArtistIds=<ArtistItems id>`, `IncludeItemTypes=Audio`, and `Recursive=true`. Verify once against the configured live Emby Service that album payloads include `ArtistItems` alongside the existing requested fields and that the query returns that artist's tracks; automated coverage remains hermetic. Add a shell-owned artist detail cache keyed by Service generation and stable artist ID. On artist focus:

- `MusicContent` immediately projects name, in-scope album count, and year span from the settled tree;
- it emits typed requests for missing artist artwork and tracks;
- the shell fetches artwork through the existing Emby image/cache path by artist item ID;
- the shell fetches the artist's Audio items once by artist ID, caches the result, and projects only tracks whose album IDs are in the root's current in-scope album set;
- the Workspace groups tracks by settled album order, then disc/track order, as canonical `Heading` plus track `Item` rows.

Completion identity includes the Library destination key, Service setup generation, artist ID, and settled catalog revision. A valid result may populate the cache after focus moves, but only a completion matching the current destination, generation, revision, and focused artist is pushed into the visible Hero/Workspace. Query changes re-project the cached artist tracks against the new in-scope album set without refetching. Fallback roots show summary and grouped tracks derivable from their album IDs but make no artist-ID artwork request; where an artist-ID query is unavailable, the shell aggregates existing per-album track fetches rather than matching by artist name.

Alternative rejected: name-based Service lookup, which conflates equal artist names. Alternative rejected: preloading all artist tracks during grouping warm-up, which couples browsing readiness to an unbounded fetch.

### 8. Use the stock tree rendering pipeline as the dependency gate

Use `TreeListView` with `TreeLabelRenderer`, `TreeColumnSet`, and `TreeListViewStyle` (or the exact equivalent in the locked 0.2.2 API) to express hierarchy glyphs, artist/album labels, year metadata, selected-row treatment, marks, scrollbar, and horizontal bounds. Style values are resolved from existing semantic theme policies inside the owning render layer; the destination does not pass raw colours.

The tree remains in the Library panel's existing browser slot. The panel owns placement and fill; the tree painter owns paint-local row geometry. No screen module calls Ratatui or splits layout, and no base frame paints underneath. Marquee timing reuses the existing title-marquee primitive if the label-renderer seam permits it.

The implementation is not allowed to bypass a missing extension point with a second projection or bespoke tree widget. After focused buffer/component/integration checks and full gates pass, live review covers representative Wide, Narrow, Mini, and Library Hero overlay states. Failure of hierarchy readability, selected-row treatment, zebra behavior, metadata, marquee, scrollbar, or narrow-width behavior rejects and removes the dependency.

### 9. Preserve test ownership and use the smallest durable evidence

Tests are added only where they protect realistic failures:

- pure model/filter cases: stable-ID reconciliation, ancestor retention, stable ordering, filter restoration, and visible-descendant materialization;
- Interactive Component cases: expansion/navigation, typed album intents, multi-selection aggregate state, viewport continuity, and latest-frame hit resolution;
- Render Component/buffer cases: semantic row differences, full-row selection, zebra reset, metadata/marquee bounds, scrollbar, and narrow clipping;
- mounted tick/mouse cases: one painter, routing/focus, responsive owner reuse, latest-frame delivery, and stale artist completion rejection.

Existing tests are adapted or replaced rather than duplicated. No snapshot suite, live Service fixture, real filesystem/config, sleep-based debounce, or smoke test is added. Debounce is tested by injecting the clock instant directly.

## Risks / Trade-offs

- **[Crate rendering seams cannot reproduce mbv's surface]** → Keep customization inside supported crate interfaces, run focused visual evidence before final acceptance, and remove the dependency/change if live review fails; do not add a parallel renderer.
- **[The locked crate API differs from the evaluated surface]** → Pin the exact compatible release, compile a minimal adapter first, and map names to the locked API without changing ownership decisions.
- **[The dependency is immature and release history is volatile]** → Version 0.2.2 is a single-maintainer crate with low adoption, no GitHub releases, and two recent yanked releases; keep the integration destination-local, pin exactly, and retain dependency rejection as the acceptance outcome.
- **[Artist identity is absent or ambiguous]** → Prefer `ArtistItems` stable IDs, retain equal-name groups separately, and use a deterministic fallback key only when identity is absent; never perform effect lookup by name.
- **[Artist completions paint beneath a new selection]** → Key requests and visible application by destination, Service generation, settled revision, and artist ID; cache valid data separately from presentation.
- **[Filtering and marks expose hidden actions]** → Intersect tree multi-selection with the visible album projection whenever a debounced filter revision applies, and materialize every root action from that same projection.
- **[Node arena grows during a long retained session]** → Reset it when destination identity changes; accept monotonic growth within one retained destination rather than risk ID reuse. Revisit compaction only if measured catalog churn makes it material.
- **[`music_content.rs` exceeds the repository gate]** → Put model, filter, identity arena, and rendering adapter in destination-specific sibling modules; keep `MusicContent` to orchestration and typed translation.
- **[Concurrent page-navigation change claims Grouped Music]** → Its canonical Heading behavior can land in either order; this change removes the Music album flow from `MediaList`, so no destination branch or ordering dependency is needed.

## Migration Plan

1. Add the locked dependency and destination-specific adapter with no optional keymap.
2. Retain `ArtistItems` identity in parsed music data, add the Audio-by-artist client operation, and complete the one-time live Emby mapping check before tree implementation.
3. Settle the verified identity into stable tree keys.
4. Replace only the Grouped Music album carrier with the tree owner; keep both track Workspaces canonical.
5. Add local filtering, root action materialization, multi-selection, and current-frame mouse handling.
6. Add shell-owned lazy artist detail/artwork/track projection with stale guards.
7. Complete focused automated evidence, repository gates, and live user review at representative Panel modes.
8. On acceptance, update `CONTEXT.md` with the new artist-root term and sync the delta specs. On rejection, remove the dependency and all tree-specific production changes; do not retain an alternate implementation from this change.

Rollback is a normal revert: no persisted tree state, protocol, queue schema, or config migration is introduced.
