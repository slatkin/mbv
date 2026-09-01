## Context

See `proposal.md` for motivation. PR #606 is open from `feat/migrate-tui-to-tuirealm` and remains blocked by issue #641. This change is the non-implementation architecture and delivery umbrella: it defines the final contract, names the implementation slices, and closes only after every slice has landed on the feature branch. It does not add destination code itself.

The repository already has destination-sized TuiRealm `AppComponent`s, typed `Msg`/`UserEvent` boundaries, central keyboard routing, a shared PillBar painter, Hero-on-left arrangement primitives, and a render-component substrate. The Hero-on-left foundation is not yet universal: Audiobookshelf Podcast (and the related Books surface) is broken. The superseded standalone Audiobookshelf implementation must be reverted; the required Books/Podcasts repairs are absorbed into the canonical Music/Audiobookshelf slice without bespoke exceptions.

Current list state and geometry are split among `BrowserComponent`, `HomeComponent`, `TvWorkspaceComponent`, `MusicWorkspaceComponent`, Audiobookshelf components, `FeedsComponent`, and `QueueComponent`. Generic Emby and TV already share the most reliable existing fixed-row painter path. The new foundation re-homes that working painter behavior rather than rewriting it.

The target boundary is:

```text
shell Model
  -> mounted destination AppComponent
       -> arrangement (outer placement and breakpoint)
            -> embedded plain Component (list state, view, hit regions)
                 -> render component (painting within supplied Rect)
```

The mounted parent owns event subscription and application-level `Event -> Msg`. For mouse input, it also owns `MouseGestureState`; the embedded child owns list-row `HitRegions<Target>` populated by its own view.

## Goals / Non-Goals

**Goals:**

- Define the reusable Wide fixed-row and Narrow selected-row-replacement controls without adding registry identities or another routing mechanism.
- Make one-column Hero-on-left rails, selection visibility, movement, scrolling, scrollbar, truncation, semantic states, and hit geometry shared behavior.
- Preserve one selected stable target and a defined viewport-row offset across responsive presentation changes without rebuilding an `App` mirror.
- Deliver the work as five independently reviewed, independently reversible implementation slices stacked on PR #606's feature branch.
- Preserve provider workspaces, effects, persistence, keyboard precedence, image behavior, and proven output, while requiring stronger evidence where existing fixtures are vacuous.

**Non-Goals:**

- Implementing destination code directly in this umbrella change.
- Replacing the TuiRealm registry, destination `AppComponent`s, central keyboard router, render tree, theme, Hero, or Pill components.
- Restoring all mouse behavior. `restore-mouse-support` owns delivery, gestures, overlays, and precedence; the slice changes own canonical list hit regions and parent delegation.
- Changing Service, Player, Queue authority, provider protocols, daemon boundaries, persistence formats, or content fetching.
- Applying the one-column control to non-hero browsers whose existing contract permits two columns.
- Creating a generic widget framework, renderer callback API, provider trait hierarchy, or independently mounted component per list child or row.

## Decisions

### D1 — Keep one umbrella and five implementation changes

`compose-canonical-media-lists` owns the final capability deltas, architecture decisions, cross-change dependencies, destination inventory, and campaign completion gates. It remains open and unarchived throughout implementation.

Five separate OpenSpec changes and PRs implement the work:

1. **Foundation + Emby/TV:** shared row/viewport types, both controls, generic Emby catalog browsing, Movies, Emby home-video/podcast browsing, narrow TV Series browsing, and the Wide TV right rail.
2. **Home + Feeds:** Home sections and Feeds grouping on the shared controls.
3. **Music + Audiobookshelf:** grouped Music albums plus Audiobookshelf Podcast shows and Books.
4. **Queue:** fixed-row-only adoption, including Queue row hit ownership and bounded progress presentation.
5. **Cleanup + reconciliation:** cross-family obsolete-loop and geometry deletion, exact inventory verification, docs/spec synchronization, and final campaign gates.

Each implementation PR targets `feat/migrate-tui-to-tuirealm`. PR #606 merges only after all five are merged into that branch and this umbrella's gates pass. A squash may combine commits inside one slice but never combines slices; each PR remains a review and rollback boundary.

**Alternatives considered:**

- **One mega-change with task groups.** Rejected because review, rollback, and baseline evidence would span too many destinations.
- **Merge #606 after the foundation slice.** Rejected because issue #641 names the missing reusable composition across all primary destinations as blocking.
- **Merge #606 first and follow on main.** Rejected because it contradicts the maintainer's blocking decision.

### D2 — Embed plain Components; keep destinations as the application boundary

Each destination `AppComponent` owns an embedded `WideMediaList<Target>` or `InlineMediaBrowser<Target>` plain TuiRealm `Component`. The embedded control receives no `ComponentId`, mount call, subscription, or independent focus-stack entry. The destination remains the sole application-level `Event -> Msg` boundary and delegates list-local actions after central keyboard policy resolves precedence.

**Alternatives considered:**

- **Mount every list child.** Rejected because it multiplies identities, subscriptions, focus targets, and keep-mounted reconciliation without creating an independent user surface.
- **Use plain Rust helpers only.** Rejected because issue #641 corrects the missing reusable TuiRealm component layer, not only duplicated painters.
- **Replace destinations with one universal AppComponent.** Rejected because provider workspaces and event translation are legitimately destination-specific.

### D3 — Use a small generic row vocabulary with bounded Queue progress

The shared model is conceptually:

```text
MediaListRow<Target>
  Item {
    target: Target,
    primary: text,
    trailing: optional text,
    semantic_state:
      ordinary
      | played
      | active { progress_percent: optional integer 0..=100 }
      | disabled
  }
  Heading { text }
  Spacer
```

`Target` is a stable, cloneable parent-defined identity. Heading and Spacer are full-width display rows absent from the selectable-target index. Queue's percentage is prepared, bounded presentation data; Queue and the shell retain Player authority, runtime ticks, activation, reorder, and scope behavior.

The model carries no `App`, Service client, source URL/header, renderer callback, raw style, breakpoint, or provider effect.

**Alternatives considered:**

- **Payload-free active state.** Rejected because it cannot represent Queue's existing percentage.
- **Raw position/runtime ticks in the shared control.** Rejected because the parent can prepare the bounded percentage without moving playback authority.
- **A central provider target enum or callback trait.** Rejected because it couples every provider and its effects to the shared layer.

### D4 — Wide and Inline are separate controls with explicit scope

`WideMediaList` always paints fixed-height, one-column rows. It accepts neither a column-count option nor an Inline-detail plan. It applies to Hero-on-left right rails. Queue may compose the same fixed-row mechanics outside the hero presentation contract. Existing non-hero two-column catalogs remain outside its scope.

`InlineMediaBrowser` always paints one browser column and may replace its selected item row with one variable-height Inline hero. The term is distinct from Inline Search. It owns height admission, display-row expansion, visibility clamp, selected-parent geometry, and ordinary-row fallback. Structured child lists remain in the selection modal.

The two controls may share a private selectable-row/index/viewport implementation. That private code is not a third public component.

### D5 — Define the position handoff precisely and characterize existing behavior first

Only the active Wide or Inline variant owns live cursor, scroll, viewport, and geometry. A responsive transition hands off:

```text
ViewportAnchor<Target> {
  selected_target,
  selected_row_offset,
}
```

`selected_row_offset` is the zero-based screen-row offset from the top of the list viewport to the top of the selected item's ordinary row before Inline replacement. The receiving control preserves that offset when possible and clamps it when its viewport cannot.

Ordinary content pushes preserve selection by stable target and otherwise clamp locally; they do not carry cursor or scroll. Persisted resting position remains shell-owned and is written only by the existing navigation event path.

Before a slice replaces TV or Music handoff logic, it adds focused characterization of the current cursor, scroll, and selected-row screen position across Wide -> Narrow -> Wide. The replacement must match that evidence unless a separately approved behavior correction says otherwise.

**Alternatives considered:**

- **An undefined “meaningful viewport anchor.”** Rejected because it cannot protect scroll position from subtle breakpoint drift.
- **Keep two live variants synchronized every frame.** Rejected as a cursor mirror with two owners.
- **Move cursor/scroll back to `App`.** Rejected because the reusable control owns the state its geometry resolves.

### D6 — Parent recognizes events; child resolves list geometry

The parent maps destination-local keyboard input to the embedded control's update API. Global chords remain owned by `router.rs`/`key_policy.rs`.

For mouse input:

1. the mounted parent receives `Event::Mouse` through its existing TuiRealm subscription;
2. the parent's `MouseGestureState` recognizes click, double click, context click, or scroll;
3. the embedded control owns and queries `HitRegions<Target>` populated by its latest view;
4. the child returns a stable target or list-local scroll result;
5. the parent emits the destination-specific typed request when work crosses authority.

Parent-owned pills, workspace children, Queue scope buttons, and overlays keep their own regions. An embedded control never subscribes or owns a second gesture recognizer. `restore-mouse-support` is revised to remove Queue/list row-hit ownership that would collide with slices; it retains delivery, primitives, overlays, and precedence.

### D7 — Arrangements place; child geometry stays local

The shared Hero-on-left decision and arrangement primitives produce left workspace, pills, and list rectangles. Slice 3 owns the Audiobookshelf Books/Podcasts arrangement and geometry repairs required for canonical composition, without a standalone prerequisite or bespoke exception.

The parent passes only the list rectangle to the active child. The child view delegates pixels to render-component functions and stores visible target rectangles, selected ordinary/replacement parent geometry, viewport facts, and optional explicit Inline child targets. The parent does not recompute row coordinates.

`LayoutMain` list fields are deleted only in slice 5 after every consumer has moved. Unrelated shell placement geometry remains.

### D8 — Re-home the working generic painter before generalizing

Slice 1 uses the existing generic Emby/TV fixed-row path, especially the body and output of `render_plain_rows`, as the source implementation for the new Wide render component. It moves and parameterizes that proven behavior; it does not start with a greenfield painter rewrite.

Characterization for the generic Emby and TV path must remain unchanged through the re-home except for an explicitly named contract correction. Heading/Spacer support and other new vocabulary are added around that baseline in focused commits.

### D9 — Image and effect work stays outside painting

Parents prepare Inline/Wide hero presentation and preserve each destination's deferred image-paint handoff. Embedded controls may hold prepared image paint requests but never inspect `App` caches, fetch images, or run effects from `view()`. Existing stale-key and images-disabled behavior remains shell-owned.

### D10 — Independent bug fixes establish prerequisites, not folded work

Before implementation slices begin:

- `restore-feed-group-inline-expansion` is narrowed to the #634/#637 Narrow Feed defects and lands independently. It removes its conflicting Wide expansion. Slice 2 later replaces that now-green Narrow implementation without taking ownership of the bug-fix change's acceptance criteria.
- `restore-feeds-service-wide-list` independently corrects issue #623 in the Feeds Service/tab Wide panel (one column, rail framing, and selected-row geometry) before slice 2. It does not touch the Emby homevideos feed view fixed by #634/#637.
- Slice 3 owns Audiobookshelf Books and Podcasts together, including non-list arrangement/geometry defects required for canonical composition, and repairs them without bespoke exceptions; no standalone prerequisite is sequenced.
- `restore-mouse-support` records D6's parent-gesture/child-hit contract and removes overlapping canonical list row-hit tasks before slice 1 begins.

### D11 — Verification combines focused automation and explicit manual evidence

Issue #641 demonstrated that some passing characterization was vacuous because fixtures lacked metadata or relevant state. Existing tests are reused only when their fixtures exercise the migrated path.

Each slice must provide:

- focused automated composition evidence for its exact destinations;
- a representative fixture for metadata, grouping, progress, breakpoint, focus, or image behavior touched by the slice;
- current-state characterization before replacing TV/Music handoff behavior;
- manual Wide/Narrow end-to-end evidence for destination selection, movement, focus, and prepared images; and
- one-painter evidence before deleting an old loop.

This deliberately avoids a new exhaustive screenshot matrix while acknowledging that automated coverage alone is not a complete safety net.

### D12 — Plan file splits inside the owning slice

The 800-line source cap is checked before wiring, not only at campaign end:

- slice 1 plans ownership-preserving splits for `src/app/components/tv_workspace.rs` and `src/app/components/browser.rs` before or with embedded-control wiring;
- slice 3 plans a split for `src/app/components/audiobookshelf_podcast.rs` before or with its wiring; and
- every slice runs the file-size gate before its PR is approved.

A split is not permission to move state across the destination/embedded-control boundary; it follows existing ownership.

### D13 — Establish vocabulary before code

Before slice 1 implementation, `CONTEXT.md` defines:

- **Wide media list / `WideMediaList`:** the canonical fixed-height single-column control for Hero-on-left rails, also reusable by Queue for fixed-row mechanics; not the policy for non-hero two-column catalogs.
- **Inline media browser / `InlineMediaBrowser`:** the canonical single-column selected-row-replacement control; distinct from Inline Search and from the Inline hero content block it contains.

The frontend skill and ledger use the same terms. No slice introduces alternate names.

### D14 — Reconcile completion only after all slices land

The umbrella remains open while slices are implemented. Slice 5 removes obsolete cross-family loops and geometry, updates ADR 0022 and the interactive-surface ledger, syncs the umbrella deltas into main specs, and runs final gates. The umbrella is archived only after all slice PRs, independent fixes, manual evidence, and PR #606 branch checks are complete.

No bespoke exception is planned. Any discovered exception requires an umbrella design/spec update before its slice proceeds.

## Risks / Trade-offs

- **[PR #606 grows while blocked]** -> Review five stacked slice PRs independently; never combine family slices into one squash or review unit.
- **[Existing characterization is vacuous]** -> Inspect fixture content, add the smallest representative case, and record manual evidence per slice.
- **[Selection jumps across breakpoints]** -> Use the defined target + selected-row offset handoff and characterize TV/Music before replacement.
- **[Scroll persistence regresses]** -> Preserve resolved-value requests and event-time persistence; never write paint-derived offsets back every frame.
- **[Images disappear after composition]** -> Require per-slice image-enabled and images-disabled evidence for affected destinations.
- **[Structural headings become selectable]** -> Keep display rows separate from selectable targets and cover the mapping with one focused table.
- **[Queue expands the shared API]** -> Admit only a bounded prepared progress percentage; keep Queue scope, reorder, playback, and title behavior parent-owned.
- **[Mouse work collides with Queue/list migration]** -> Record D6 in both umbrella and `restore-mouse-support`; make canonical list row hits slice-owned.
- **[The abstraction becomes a callback framework]** -> Hold the model to prepared data plus opaque targets and require an umbrella update for callbacks.
- **[A slice crosses the file cap]** -> Split named near-limit files before or with wiring and run the gate in every slice.
- **[Superseded standalone Audiobookshelf implementation remains present]** -> Revert that implementation and absorb the required Audiobookshelf Books/Podcasts repairs in slice 3; require user live visual approval before changing or adding regression tests.

## Migration Plan

1. Land the revised umbrella planning artifacts; keep PR #606 blocked.
2. Narrow and land #634/#637 independently, reconcile `restore-mouse-support` with D6, and land `restore-feeds-service-wide-list` before the Home/Feeds slice. Audiobookshelf Books/Podcasts repairs belong to the canonical Music/Audiobookshelf slice rather than a standalone prerequisite.
3. Create and approve all five slice OpenSpec changes, each naming its branch dependency, exact destination inventory, file splits, automated evidence, and manual checks.
4. Land slice 1 through slice 4 as separate PRs against `feat/migrate-tui-to-tuirealm`; do not cross-squash slices.
5. Land slice 5 cleanup/reconciliation after all destination slices are green.
6. Run umbrella strict validation and final branch gates, sync umbrella deltas, archive the umbrella, and then merge PR #606.

Rollback is one slice PR at a time. Reverting a slice must not restore global legacy interaction infrastructure or activate two painters as a fallback.
