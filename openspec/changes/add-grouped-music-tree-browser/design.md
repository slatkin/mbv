## Context

See `proposal.md` for motivation. Grouped Music currently derives an atomically published `GroupedAlbumCatalog`, flattens it to `[Heading, Spacer, Item…]`, and projects the album flow through `MediaListCarrier<String>`. `MusicContent` owns album and track interaction but is already 756 lines, so adding the tree there would worsen an already large orchestration module. The shell owns album-track fetching, image effects, and projection into the component.

`tui-treelistview` 0.2.2 matches mbv's Ratatui 0.30 generation. Its useful boundary is the combination of `TreeModel`, `TreeQuery`, `TreeListViewState`, current-render hit testing, label/column renderers, and style configuration. Its optional keymap is incompatible with mbv's single Keyboard Router and will remain disabled. Its nodes use compact copyable IDs while mbv's stable domain targets are owned strings.

The existing contracts conflict with the new surface in deliberate, narrow ways: Grouped Music artist labels are currently non-selectable canonical `Heading` rows; album browsing is required to use `MediaList`; and Inline Search is required to replace grouping with flat, relevance-ordered, full-corpus results. The delta specs make Grouped Music the only exception. Track Workspaces and all other destinations remain canonical media lists.

## Goals / Non-Goals

**Goals:**

- Keep one tree state owner across Wide, Narrow, Mini, and Library Hero overlay composition.
- Use the dependency's supported model/query/state/rendering seams rather than reimplementing its projection.
- Preserve mbv's Panel, Keyboard Router, mouse, semantic-theme, selected-row, and typed-effect boundaries.
- Keep new tree code outside `music_content.rs` so the existing orchestration module does not grow further.
- Make dependency rejection cheap and complete if final customization review fails.

**Non-Goals:**

- A generic `TreeMediaList`, recursive browsing, lazy child loading, tree editing, on-disk persistence of expansion, or migration of another destination.
- Replacing canonical `MediaList` in either album-track or artist-track Workspaces.
- Relevance sorting, a full-library Grouped Music search, preloading every artist's tracks, or a bespoke renderer parallel to `TreeListView`.
- Changing the shared Panel geometry, breakpoints, or Service/Player authority.

## Decisions

### D1. Add one destination-specific tree owner

Add a sibling Grouped Music tree module under `src/app/components/` and keep `MusicContent` as the mounted event and typed-intent boundary. The module owns:

- a `MusicTreeModel` adapted from the current settled catalog;
- one `TreeListViewState` for selection, expansion, viewport, marks, projection cache, and hit map;
- the local filter session and pre-filter anchor;
- stable node interning and node-to-domain translation;
- tree view painting through the crate's widget and supported renderer/style interfaces.

`MusicContent` delegates eligible browser operations to this owner and continues to own active-pane focus, Hero/Workspace composition, query bar placement, and request translation. The shell projects settled catalog facts and accepts typed artist/album intents; it never reads tree cursor or expansion state. In non-Wide geometry the tree opens selected artist or album detail in the existing Library Hero overlay and removes Grouped Music's album-only inline-row Hero path; retaining that path would create a second, album-specific detail composition that cannot represent the new artist Workspace.

Alternative rejected: extending `MediaList` with tree semantics. That would preserve fewer dependency benefits, spread parent/leaf cases through every canonical-list operation, and make the one-screen evaluation harder to remove. Alternative rejected: wrapping both a hidden `MediaList` and a visible tree. That creates two cursor and selection authorities.

### D2. Intern stable domain keys into monotonic node IDs

Use a destination-local arena:

```text
usize NodeId -> MusicNode::Artist(ArtistKey) | MusicNode::Album(AlbumTarget)
MusicNodeKey -> usize NodeId
```

`ArtistKey` is either a stable Emby artist item ID or a deterministic fallback grouping key. The integration treats `ArtistItems` as a fallback-safe Emby payload assumption to confirm at terminal acceptance: the public `getItems` reference does not enumerate either `ArtistItems` or mbv's already-used `AlbumArtist`/`Artists` fields, so it is not authoritative for this DTO shape ([Emby ItemsService](https://dev.emby.media/reference/RestAPI/ItemsService/getItems.html)). Relevant Music album/item requests explicitly include `ArtistItems` in their `Fields`, and the parser retains name/ID pairs only when returned. A pair is applicable only when its trimmed name case-insensitively equals the settled displayed album artist; no arbitrary first pair is chosen for multi-artist or `Various Artists` albums. If the field is ignored, absent, or has no matching pair, the album uses fallback identity. The settled catalog carries both display text and the resolved identity.

The `ArtistIds=<id>&IncludeItemTypes=Audio&Recursive=true` query is likewise an assumption to confirm, not a prerequisite for tree correctness. Mock tests prove mbv's request and response handling, while terminal acceptance proves server support; unsupported or rejected queries use per-album aggregation. `ArtistItems` IDs from album/item payloads are the sole artist ID space used for that query and `/Items/{id}/Images`; IDs obtained from an `/Artists` listing SHALL never be mixed into these keys. The fallback key derives from the settled grouping identity, not display position. Equal names with different IDs remain separate roots.

IDs are interned by semantic key, survive ordinary catalog replacement, and are not reused for a different key during the owner's lifetime. Removed entries are tombstoned or omitted from the model while preserving their interned mapping; the arena resets only when the retained Music destination changes identity. This favors simple, collision-free continuity over speculative compaction.

Alternative rejected: label IDs, which collide for equal artist names and fallback labels. Alternative rejected: hashing owned targets into `usize`, which introduces collision handling without reducing state.

### D3. Reconcile state from each atomic settled catalog

A model revision changes only when the settled catalog changes. Reconciliation proceeds by stable node key:

1. intern artist roots and album leaves in settled order;
2. replace the model's root/child projection atomically;
3. retain expansion, marks, and selected node that still exist;
4. if the selected node survives, preserve its prior viewport row when bounds permit, otherwise apply the minimum scroll needed to keep it visible and clamp at projection bounds; if the node is missing, apply the crate's selection fallback and then the same visibility rule;
5. emit an album-selection persistence request only when the resolved selected album changes; artist focus does not overwrite album persistence with an artist ID.

Filter-query and expansion revisions remain separate from model revision. Responsive geometry updates configure the same owner and invalidate old hit geometry before rendering.

Alternative rejected: rebuilding IDs from row position, which makes selection and expansion jump on refresh.

### D4. Map mbv input explicitly; do not enable the crate keymap

The existing Keyboard Router keeps precedence. Once it returns local fall-through, `MusicContent` maps the confirmed chords to tree view actions. `/`, printable filter text, Visual mode, context actions, and track Workspace focus remain destination-local semantic operations. No crate keymap or second routing table receives terminal events.

The crate's latest-render `hit_test` resolves artist/album targets inside the browser rectangle. `MusicContent` keeps gesture recognition and sends already-resolved targets to local operations or typed requests, matching ADR 0024. Geometry is invalidated on content or area changes before the next view.

Neighbour artwork prefetch follows the same boundary: after a completed paint, the tree owner resolves the selected album leaf's ordered ±1-behind/±3-ahead album targets from that exact visible projection and emits those stable targets in a typed shell request. The shell applies the existing idle gate and performs fetches; it receives neither a tree cursor nor enough tree state to re-resolve the window.

The active `group-aware-page-navigation` change remains unchanged for canonical Heading-bearing lists. Grouped Music stops using that owner and uses visible-node viewport paging supplied by the tree state.

### D5. Implement tree-local fuzzy filtering, not a second InlineSearch result owner

Reuse `SkimMatcherV2` and the existing 300 ms duration without creating a second text-input owner. `MusicContent.inline_search` remains the sole query editor, debounce clock, open/close state, and source for the Library panel's one-row search-bar projection. Its `InlineSearchHost` integration drains a debounced Grouped Music query into a small destination-specific tree filter session, which owns only the applied matching node IDs, forced expansion, and pre-filter anchor. Grouped Music removes the `ListSlot::Search` flat-carrier swap in `MusicContent::panel_content`; the ordinary tree remains the `ListSlot::List` browser while the panel paints the existing bar above it. No `InlineSearch` result `MediaListCarrier` is constructed or queried for Grouped Music.

Each album leaf has precomputed searchable text containing artist, album title, and year. On debounce expiry, the filter records the matching album node IDs; the tree query retains ancestors and force-expands matching paths while leaving persistent expansion untouched. Scores are discarded after match/no-match classification, preserving settled order.

Opening the filter snapshots the selected node. Empty text disables filtering and shows the full tree. Dismissal restores the snapshot if present and persistent expansion. Filtering masks stored marks outside the visible album projection rather than deleting them: status, aggregate root state, painting, ranges, and bulk actions use only visible marks while the filter is active; dismissal reveals surviving pre-filter marks without adding newly hidden albums. Query changes perform no shell request.

Alternative rejected: adapting the current `InlineSearch` carrier. Its flat result owner, full-corpus fetch lifecycle, relevance order, and empty-result behavior are the semantics this exception replaces. Alternative rejected: extracting a generic search framework for one new caller.

### D6. Materialize artist operations from leaves at the component boundary

Artist roots never cross the shell as playable targets. For play, enqueue, shuffle, and context requests, the tree owner walks the root's settled child leaves and returns ordered album targets. Expansion does not affect action scope. An active filter restricts the walk to matching visible leaves.

Root multi-selection applies mark operations to the same visible child leaves. Aggregate root state is derived from child marks. Range selection walks visible album leaves and skips roots. The component emits existing ordered album-target intents with a stable tree-origin identity, allowing existing effect materialization and context capability intersection to remain downstream.

Alternative rejected: storing selected artist IDs. Their meaning changes with filtering and would force the shell to re-resolve component-local scope.

### D7. Add artist detail without giving the component Service authority

Before tree implementation, request and retain artist name/ID pairs in the Emby item projection and establish the `mbv-core` Audio-by-artist operation using `ArtistIds=<ArtistItems id>`, `IncludeItemTypes=Audio`, and `Recursive=true`. Automated coverage proves request-field construction, parsing, query construction, and unsupported/error propagation hermetically. A terminal manual check records whether the configured live Emby Service returns `ArtistItems` and supports that query, but implementation proceeds fallback-safe either way. Add a shell-owned artist detail cache keyed by Service generation and stable artist ID. On artist focus:

- `MusicContent` immediately projects name, in-scope album count, and year span from the settled tree;
- it emits typed requests for missing artist artwork and tracks;
- the shell fetches artwork through the existing Emby image/cache path by artist item ID;
- the shell fetches the artist's Audio items once by artist ID, caches the result, and projects only tracks whose album IDs are in the root's current in-scope album set;
- the Workspace groups tracks by settled album order, then disc/track order, as canonical `Heading` plus track `Item` rows.

Every new artist-artwork and artist-track request and completion is a typed boundary variant with an exhaustive shell dispatch arm; unsupported fallback roots are handled explicitly rather than hidden by wildcard matching. Completion identity includes the Library destination key, Service setup generation, artist ID, and settled catalog revision. A valid result may populate the cache after focus moves, but only a completion matching the current destination, generation, revision, and focused artist is pushed into the visible Hero/Workspace. Query changes re-project the cached artist tracks against the new in-scope album set without refetching. Fallback roots show summary and grouped tracks derivable from their album IDs but make no artist-ID artwork request; where an artist-ID query is unavailable, the shell aggregates existing per-album track fetches rather than matching by artist name.

Alternative rejected: name-based Service lookup, which conflates equal artist names. Alternative rejected: preloading all artist tracks during grouping warm-up, which couples browsing readiness to an unbounded fetch.

### D8. Use the stock tree rendering pipeline as the dependency gate

Use `TreeListView` with `TreeLabelRenderer`, `TreeColumnSet`, and `TreeListViewStyle` from the exact locked 0.2.2 API to express hierarchy glyphs, artist/album labels, year metadata, selected-row treatment, marks, scrollbar, and horizontal bounds. The crate's published metadata declares Ratatui 0.30.2 compatibility, and its API documentation exposes these seams plus latest-render hit testing ([docs.rs](https://docs.rs/tui-treelistview/0.2.2/tui_treelistview/), [lib.rs](https://lib.rs/crates/tui-treelistview)). Task 1.1 is a hard go/no-go gate: before the tree owner or artist features are built, a one-frame adapter spike SHALL paint the existing Wide and smallest supported non-Wide fixtures and prove the full-row bar, group-relative zebra, scrollbar, focused marquee, and clipping through supported seams. If it does not compile, any required seam is absent, or that buffer evidence fails, stop and reject the dependency.

Style values are resolved from existing semantic theme policies inside the owning render layer; the destination does not pass raw colours. Album leaves use `MediaSemanticState::from_emby`, whose music collapse keeps them `Ordinary`; the label renderer SHALL NOT re-derive played or resume decoration from raw `EmbyItem` fields. Artist roots are likewise ordinary grouping rows.

Tree album years follow the canonical single metadata-gutter contract introduced by `unify-row-metadata-gutter`: one right-aligned fixed six-column gutter in `STATUS_AVAILABLE`, no inline or second year column, and no reserved gutter on artist roots or leaves without a year. Because the tree has its own label/column renderer, task 3.1 SHALL wait until that change is archived or its delta is applied to the main canonical spec, then reproduce the settled contract without depending on `MediaListTrailing` or its painter.

The tree remains in the Library panel's existing browser slot. The panel owns placement and fill; the tree painter owns paint-local row geometry. No screen module calls Ratatui or splits layout, and no base frame paints underneath. Marquee timing reuses the existing title-marquee primitive if the label-renderer seam permits it.

The implementation is not allowed to bypass a missing extension point with a second projection or bespoke tree widget. After focused buffer/component/integration checks and full gates pass, live review covers Wide, Narrow, Mini, and Library Hero overlay states, including the smallest supported non-Wide Library-panel width. Failure of hierarchy readability, selected-row treatment, zebra behavior, metadata, marquee, scrollbar, or narrow-width behavior rejects and removes the dependency.

### D9. Preserve test ownership and use the smallest durable evidence

Tests are added only where they protect realistic failures:

- pure model/filter cases: stable-ID reconciliation, ancestor retention, stable ordering, filter restoration, and visible-descendant materialization;
- Interactive Component cases: expansion/navigation, typed album intents, multi-selection aggregate state, viewport continuity, and latest-frame hit resolution;
- Render Component/buffer cases: semantic row differences, full-row selection, zebra reset, metadata/marquee bounds, scrollbar, and narrow clipping;
- mounted tick/mouse cases: one painter, routing/focus, responsive owner reuse, latest-frame delivery, and stale artist completion rejection.

Existing tests are adapted or replaced rather than duplicated. No snapshot suite, live Service fixture, real filesystem/config, sleep-based debounce, or smoke test is added. Debounce is tested by injecting the clock instant directly.

## Risks / Trade-offs

- **[Crate rendering seams cannot reproduce mbv's surface]** → Keep customization inside supported crate interfaces, run focused visual evidence before final acceptance, and remove the dependency/change if live review fails; do not add a parallel renderer.
- **[The locked crate API differs from the evaluated surface]** → Pin 0.2.2 exactly, make the minimal adapter a hard stop, and reject the dependency rather than guessing equivalent APIs.
- **[The dependency has limited adoption and a short release history]** → Current registry metadata shows one owner, low download volume, and two yanked earlier versions ([lib.rs](https://lib.rs/crates/tui-treelistview), [crates.io API](https://crates.io/api/v1/crates/tui-treelistview)); keep the integration destination-local, pin exactly, and retain dependency rejection as the acceptance outcome.
- **[Artist identity is absent, ambiguous, or omitted by a Service]** → Request `ArtistItems` explicitly, accept only a name-matching pair, retain equal-name groups separately, and use a deterministic fallback key when no match exists; never perform effect lookup by name. Unsupported artist-ID queries propagate to the shell's per-album aggregation fallback.
- **[Artist completions paint beneath a new selection]** → Key requests and visible application by destination, Service generation, settled revision, and artist ID; cache valid data separately from presentation.
- **[Filtering and marks expose hidden actions]** → Intersect tree multi-selection with the visible album projection whenever a debounced filter revision applies, and materialize every root action from that same projection.
- **[Node arena grows during a long retained session]** → Reset it when destination identity changes; accept monotonic growth within one retained destination rather than risk ID reuse. Revisit compaction only if measured catalog churn makes it material.
- **[`music_content.rs` becomes harder to maintain]** → Put model, filter, identity arena, and rendering adapter in destination-specific sibling modules; keep `MusicContent` to orchestration and typed translation.
- **[Concurrent page-navigation change claims Grouped Music]** → Its canonical Heading behavior can land in either order; this change removes the Music album flow from `MediaList`, so no destination branch or ordering dependency is needed.
- **[Concurrent metadata-gutter change defines year placement]** → Do not invent a tree-specific year column while `unify-row-metadata-gutter` is unsettled. Sequence task 3.1 after that change is archived or its delta is applied, then match its fixed six-column green gutter contract in the tree renderer.

## Migration Plan

1. Add the locked dependency and destination-specific adapter with no optional keymap.
2. Request and retain `ArtistItems` identity in parsed music data and add the fallback-safe Audio-by-artist client operation; defer the advisory live Emby mapping check to terminal acceptance.
3. Settle the verified identity into stable tree keys.
4. Replace only the Grouped Music album carrier with the tree owner; keep both track Workspaces canonical.
5. Add local filtering, root action materialization, multi-selection, and current-frame mouse handling.
6. Add shell-owned lazy artist detail/artwork/track projection with stale guards.
7. Complete focused automated evidence, repository checks, and live user review in Wide, Narrow, Mini, and Library Hero overlay Panel states.
8. On acceptance, update `CONTEXT.md` with the new artist-root term and sync the delta specs. On rejection, remove the dependency and all tree-specific production changes; do not retain an alternate implementation from this change.

Rollback is a normal revert: no persisted tree state, protocol, queue schema, or config migration is introduced.
