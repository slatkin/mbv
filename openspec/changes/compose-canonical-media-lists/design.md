## Context

See `proposal.md` for motivation. PR #606 is open from `feat/migrate-tui-to-tuirealm` and remains blocked by issue #641. This change is the non-implementation architecture and delivery umbrella: it defines the final contract, names the implementation slices, and closes only after every slice has landed on the feature branch. It does not add destination code itself.

The repository already has destination-sized TuiRealm `AppComponent`s, typed `Msg`/`UserEvent` boundaries, central keyboard routing, a shared PillBar painter, Hero-on-left arrangement primitives, and a render-component substrate. The Hero-on-left foundation is not yet universal: Audiobookshelf Podcast (and the related Books surface) is broken. `restore-audiobookshelf-podcast-wide-layout` (#640) is superseded and archived without execution; only the separately landed #640 Home podcast sub-view hero-placement fix stays. The Audiobookshelf Podcast library Wide repair and the Books repair are absorbed into the canonical Music/Audiobookshelf slice without bespoke exceptions.

Current list state and geometry are split among `BrowserComponent`, `HomeComponent`, `TvWorkspaceComponent`, `MusicWorkspaceComponent`, Audiobookshelf components, `FeedsComponent`, and `QueueComponent`. Generic Emby and TV already share the most reliable existing fixed-row painter path. The new foundation re-homes that working painter behavior rather than rewriting it.

The target boundary is:

```text
shell Model
  -> mounted destination AppComponent
       -> arrangement (outer placement and breakpoint)
            -> embedded plain Component (list state, view, paint geometry)
                 -> render component (painting within supplied Rect)
```

The mounted parent owns event subscription and application-level `Event -> Msg`. Mouse input is out of scope for this campaign and owned by `restore-mouse-support` (#638), which lands last; no canonical slice touches mouse, and the existing bespoke `*HitRegion` paths stay wired and untouched.

## Goals / Non-Goals

**Goals:**

- Define the reusable Wide fixed-row and Narrow selected-row-replacement controls without adding registry identities or another routing mechanism.
- Make one-column Hero-on-left rails, selection visibility, movement, scrolling, scrollbar, truncation, and semantic states shared behavior.
- Preserve one selected stable target and a defined viewport-row offset across responsive presentation changes without rebuilding an `App` mirror.
- Deliver the work as five independently reviewed, independently reversible implementation slices stacked on PR #606's feature branch.
- Preserve provider workspaces, effects, persistence, keyboard precedence, image behavior, and proven output, while requiring stronger evidence where existing fixtures are vacuous.

**Non-Goals:**

- Implementing destination code directly in this umbrella change.
- Replacing the TuiRealm registry, destination `AppComponent`s, central keyboard router, render tree, theme, Hero, or Pill components.
- Any mouse behavior. `restore-mouse-support` (#638) owns all of it (delivery, gestures, overlays, precedence, `HitRegions<Target>` on the canonical controls, and per-surface row-hit migration) and lands last. No canonical slice builds or wires mouse.
- Changing Service, Player, Queue authority, provider protocols, daemon boundaries, persistence formats, or content fetching.
- Applying the one-column control to non-hero browsers whose existing contract permits two columns.
- Creating a generic widget framework, renderer callback API, provider trait hierarchy, or independently mounted component per list child or row.

## Decisions

### D1 — Keep one umbrella and five implementation changes

`compose-canonical-media-lists` owns the final capability deltas, architecture decisions, cross-change dependencies, destination inventory, and campaign completion gates. It remains open and unarchived throughout implementation.

Five separate OpenSpec changes and PRs implement the work:

1. **Foundation + Emby/TV:** shared row/viewport types, both controls, hero-bearing generic Emby catalog browsing, Movies, the Emby homevideos feed view, the Emby podcast channel list, narrow TV Series browsing, and the Wide TV right rail.
2. **Home + Feeds:** Home sections and Feeds grouping on the shared controls.
3. **Music + Audiobookshelf:** grouped Music albums plus Audiobookshelf Podcast shows and Books.
4. **Queue:** fixed-row-only adoption and bounded progress presentation.
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

Before a slice replaces TV or Music handoff logic, it characterizes the current cursor, scroll, and selected-row screen position across Wide -> Narrow -> Wide. That characterization is read-only — source reading, unchanged existing evidence, and manual observation only — because it precedes the explicit user live visual approval that D11 requires before any UI fixture or test change. The replacement must match that evidence unless a separately approved behavior correction says otherwise.

**Alternatives considered:**

- **An undefined “meaningful viewport anchor.”** Rejected because it cannot protect scroll position from subtle breakpoint drift.
- **Keep two live variants synchronized every frame.** Rejected as a cursor mirror with two owners.
- **Move cursor/scroll back to `App`.** Rejected because the reusable control owns the state its geometry resolves.

### D6 — Parent maps keyboard; mouse is deferred to #638

The parent maps destination-local keyboard input to the embedded control's update API. Global chords remain owned by `router.rs`/`key_policy.rs`.

Mouse input is entirely out of campaign scope. No canonical slice adds a mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and no canonical slice touches the existing bespoke `*HitRegion` enums or hit-test code, which stay wired and untouched. `restore-mouse-support` (#638) lands as the final change on `feat/migrate-tui-to-tuirealm`, after every canonical slice, and owns all mouse work end to end: the delivery spine, gestures, arbitration, overlays, precedence, adding `HitRegions<Target>` to the landed `WideMediaList`/`InlineMediaBrowser`, and the per-surface (Queue included) row-hit migration onto those controls.

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

- `restore-feed-group-inline-expansion` is narrowed to the #634/#637 Emby homevideos feed view (and Emby podcast channel list) Narrow inline-expansion defect and lands independently. It removes its conflicting Wide expansion. That feed view is a separate surface from the Feeds Service: slice 1 — not slice 2 — later composes it with the canonical `InlineMediaBrowser`, without taking ownership of the bug-fix change's acceptance criteria. The Home/Feeds slice depends on `restore-feeds-service-wide-list` (#623, task 1.3a) plus the foundation, not on #634/#637.
- `restore-feeds-service-wide-list` independently corrects issue #623 in the Feeds Service/tab Wide panel (one column, rail framing, and selected-row geometry) before slice 2, and that accepted one-column baseline plus the foundation is the Home/Feeds slice's baseline. It does not touch the Emby homevideos feed view fixed by #634/#637.
- Slice 3 owns Audiobookshelf Books and Podcasts together, including non-list arrangement/geometry defects required for canonical composition, and repairs them without bespoke exceptions; no standalone prerequisite is sequenced.
- `restore-mouse-support` (#638) is not a prerequisite and is not revised by this campaign. It lands as the final change on the feature branch, after all five canonical slices, and owns all mouse work end to end.

### D11 — Verification combines focused automation and explicit manual evidence

Issue #641 demonstrated that some passing characterization was vacuous because fixtures lacked metadata or relevant state. Existing tests are reused only when their fixtures exercise the migrated path.

**Controlling order for any rendered media-list surface:** production visual correction first; then explicit user live visual approval of the running result; then — and only then — any UI fixture, characterization-buffer, or rendered/geometry test change for that surface. Characterization performed before that approval is read-only: source reading, unchanged existing evidence, and manual observation only, adding or editing no test or fixture. Non-visual tests (delivery, arbitration, selectable-index) MAY precede approval. This order is the `ui-design-system` "Visual verification precedes UI tests" requirement this change adds; every slice inherits it.

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

The umbrella remains open while slices are implemented. Slice 5 removes obsolete cross-family loops and geometry, updates ADR 0022 and the interactive-surface ledger, and runs final gates.

Spec sync at close-out is split by owner. The `canonical-media-lists` and `right-panel-arrangements` capabilities in `openspec/specs/` are populated only by the slice deltas (foundation / home-feeds / music-abs), synced when each slice archives normally. The umbrella `specs/` tree is the campaign contract-of-record and is archived `--skip-specs`; it is not re-synced. The umbrella contributes to `openspec/specs/` only the requirements no slice delta owns — the `ui-design-system` and `interactive-component-framework` requirements — hand-merged using the repo's established `--skip-specs` plus manual-merge workflow (same as change #621). Before any archive, the reconciliation in tasks 5.3a / 5.4 confirms no umbrella-only normative content is stranded and normalizes or hand-merges every base-less slice `## MODIFIED` block.

The umbrella is archived only after all slice PRs, independent fixes, manual evidence, and PR #606 branch checks are complete.

No bespoke exception is planned. Any discovered exception requires an umbrella design/spec update before its slice proceeds.

### D15 — Slices 2–4 develop concurrently; integrate sequentially

After slice 1 (foundation) merges, slices 2 (Home/Feeds), 3 (Music/Audiobookshelf), and 4 (Queue) have no dependency on each other and MAY be developed in parallel, each in its own worktree branched from the post-slice-1 `feat/migrate-tui-to-tuirealm`. They touch disjoint destination components.

Integration stays sequential: each merges as its own PR and rollback boundary, rebased on `feat/migrate-tui-to-tuirealm` as the prior slice lands. Slices are never combined into one merge or squash. Slice 5 (cleanup) merges only after 2–4 are all green.

Concurrent branches contend only on shared glue — `src/app/shell.rs` mount sites, `src/app/layout.rs`, arrangement primitives, `CONTEXT.md`, and the interactive-surface ledger; the last slice to merge absorbs that rebase. `LayoutMain` field deletion stays in slice 5 (D7) so concurrent slices only read those fields.

**Alternatives considered:**

- **One combined merge of 2–4.** Rejected: issue #641 requires distinct PR/rollback boundaries and forbids cross-squashing slices.
- **Strict serial development of 2–4.** Rejected: they are destination-disjoint; serializing adds calendar time for no isolation benefit.

## Risks / Trade-offs

- **[PR #606 grows while blocked]** -> Review five stacked slice PRs independently; never combine family slices into one squash or review unit.
- **[Existing characterization is vacuous]** -> Inspect fixture content, add the smallest representative case, and record manual evidence per slice.
- **[Selection jumps across breakpoints]** -> Use the defined target + selected-row offset handoff and characterize TV/Music before replacement.
- **[Scroll persistence regresses]** -> Preserve resolved-value requests and event-time persistence; never write paint-derived offsets back every frame.
- **[Images disappear after composition]** -> Require per-slice image-enabled and images-disabled evidence for affected destinations.
- **[Structural headings become selectable]** -> Keep display rows separate from selectable targets and cover the mapping with one focused table.
- **[Queue expands the shared API]** -> Admit only a bounded prepared progress percentage; keep Queue scope, reorder, playback, and title behavior parent-owned.
- **[Mouse work creeps into the canonical slices]** -> All mouse work is `restore-mouse-support`-owned and lands last; no slice task, spec, or dependency references mouse, `HitRegions`, or gestures.
- **[The abstraction becomes a callback framework]** -> Hold the model to prepared data plus opaque targets and require an umbrella update for callbacks.
- **[A slice crosses the file cap]** -> Split named near-limit files before or with wiring and run the gate in every slice.
- **[Superseded standalone Audiobookshelf `#640` work remains present]** -> `restore-audiobookshelf-podcast-wide-layout` is archived without execution; revert any standalone implementation still present and absorb the required Audiobookshelf Books/Podcasts repairs in slice 3; require user live visual approval before changing or adding regression tests.

## Migration Plan

1. Land the revised umbrella planning artifacts; keep PR #606 blocked.
2. Land `restore-feed-group-inline-expansion` (#634/#637) independently as the Emby homevideos feed view Narrow inline-expansion fix — a separate surface from the Feeds Service, later composed by slice 1, not a Home/Feeds-slice dependency. Land `restore-feeds-service-wide-list` (#623, task 1.3a) before the Home/Feeds slice; it plus the foundation is that slice's baseline. Audiobookshelf Books/Podcasts repairs belong to the canonical Music/Audiobookshelf slice; `restore-audiobookshelf-podcast-wide-layout` (#640) is superseded and archived without execution.
3. Create and approve all five slice OpenSpec changes, each naming its branch dependency, exact destination inventory, file splits, automated evidence, and manual checks.
4. Land slice 1 through slice 4 as separate PRs against `feat/migrate-tui-to-tuirealm`; do not cross-squash slices. Slices 2–4 MAY be built concurrently in separate worktrees off post-slice-1 `feat/migrate-tui-to-tuirealm` (D15); they still integrate as separate sequential PRs, each rebased as the prior lands.
5. Land slice 5 cleanup/reconciliation after all destination slices are green.
6. Run `openspec validate --strict` for the umbrella, the five canonical slices (`introduce-canonical-media-list-foundation`, `migrate-home-feeds-to-canonical-lists`, `migrate-music-audiobookshelf-to-canonical-lists`, `migrate-queue-to-canonical-list`, `remove-bespoke-media-list-loops`), `restore-feed-group-inline-expansion`, `restore-feeds-service-wide-list`, `restore-mouse-support`, and the superseded `restore-audiobookshelf-podcast-wide-layout` (#640, expected `apply` inventory 0/0); run final branch gates. After all five canonical slices land, and before the close-out below, land `restore-mouse-support` (#638) as the final change on the feature branch — it archives with its own umbrella (#603), not this campaign, but #641's gate and the PR #606 merge both wait for it. Then close out in discrete order per tasks 5.3a and 5.4: (1) manual pre-sync reconciliation; (2) archive the five slices plus the independent `restore-feed-group-inline-expansion` / `restore-feeds-service-wide-list` fixes and the superseded `restore-audiobookshelf-podcast-wide-layout`, whose deltas sync normally; (3) archive the umbrella `--skip-specs`, keeping its `specs/` tree as the contract-of-record reference; (4) hand-merge the umbrella's `ui-design-system` and `interactive-component-framework` requirements into `openspec/specs/`. Then merge PR #606.

Rollback is one slice PR at a time. Reverting a slice must not restore global legacy interaction infrastructure or activate two painters as a fallback.
