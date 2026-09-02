## Why

Issue #641 found that the TuiRealm migration stopped at destination-sized `AppComponent`s: primary destinations still duplicate fixed-row list state, movement, painting, scrolling, and hit geometry. This is a blocking architecture correction for open PR #606, but it must be reviewed and landed as stacked, bounded slices rather than added as one more mega-diff.

**Tracking issue:** [GitHub issue #641](https://github.com/slatkin/mbv/issues/641). The issue owns the ordered delivery checklist; this OpenSpec change owns the architecture, requirements, and completion gates.

## What Changes

- Keep this change as the architecture and delivery umbrella for issue #641; it defines the target contracts and tracks completion but does not directly implement destination code.
- Keep PR #606 blocked. Each implementation slice is reviewed as its own PR targeting `feat/migrate-tui-to-tuirealm`; PR #606 merges only after every required slice has landed on that branch and the umbrella completion gates pass.
- Deliver five implementation slices:
  1. canonical row/viewport foundation plus hero-bearing generic Emby, Movies, the Emby homevideos feed view, the Emby podcast channel list, narrow TV, and the Wide TV right rail;
  2. Home and Feeds;
  3. grouped Music and Audiobookshelf Podcasts/Books;
  4. Queue's fixed-row-only adoption; and
  5. cross-family obsolete-loop deletion, architecture reconciliation, and final verification.
- Introduce a canonical reusable `WideMediaList` plain TuiRealm `Component`, embedded inside destination `AppComponent`s rather than mounted independently. It is the single-column fixed-row control for Hero-on-left rails; Queue may compose its fixed-row behavior without entering the hero/Inline presentation contract. Existing non-hero two-column browsers remain outside its scope.
- Introduce a distinct reusable `InlineMediaBrowser` plain `Component` for the defined Narrow selected-row-replacement presentation. This term is distinct from Inline Search and is added to the project vocabulary before implementation.
- Give the reusable controls live cursor, scroll, viewport, movement, visibility clamp, row placement, semantic list styling, scrollbar, and render-derived hit geometry authority.
- Define a small provider-neutral row vocabulary with selectable item rows and non-selectable heading/spacer rows. Queue progress is prepared semantic presentation data with a bounded progress payload, not provider or Player authority.
- Keep provider content, heroes, workspaces, pills, effects, image authority, persistence, and provider-specific `Msg` translation with the destination parent and shell.
- Require composition for the exact primary surfaces in scope: Home; hero-bearing generic Emby libraries that use the shared media browser; Movies; TV Series browsing; grouped Music album browsing; the Emby homevideos feed view; the Emby podcast channel list; Audiobookshelf Podcast show browsing; Audiobookshelf Book browsing; Feeds; and Queue's fixed-row list. Non-hero two-column browsers keep their existing column policy and are not forced onto the canonical control. Any true exception is recorded as a named bespoke surface.
- Ship `restore-feed-group-inline-expansion` (#634/#637) independently before slice 1 composes that surface. It is a self-contained fix to the Emby homevideos feed view (and Emby podcast channel list) Narrow inline-expansion defect and removes its conflicting Wide expansion. It is a separate surface from the Feeds Service; the canonical `InlineMediaBrowser` later composes the Emby homevideos feed view in slice 1. The Home/Feeds slice does not depend on it.
- Ship the independent `restore-feeds-service-wide-list` prerequisite for issue #623 before the Home/Feeds slice. It corrects only the Feeds Service/tab Wide panel; it is separate from accepted #634/#637, whose scope is the Emby homevideos feed view. That accepted one-column baseline (#623, task 1.3a) plus the canonical foundation is the Home/Feeds slice's baseline.
- Absorb the Audiobookshelf Books and Podcasts corrections into the Music/Audiobookshelf canonical-list slice; do not require a standalone repair or preserve a bespoke exception for either surface. `restore-audiobookshelf-podcast-wide-layout` (#640) is superseded and archived without execution; only the separately landed #640 Home podcast sub-view hero-placement fix stays.
- Resolve mouse ownership jointly with `restore-mouse-support`: the mounted destination parent owns the mouse subscription, `MouseGestureState`, and gesture recognition; the embedded canonical child owns view-populated `HitRegions<Target>` and resolves points. Parent-produced mouse messages use the runtime-only typed origin envelope defined by `restore-mouse-support`; the shell arbitrates that source tag before unwrapping the winner. The parent recognizes a gesture, then delegates point resolution to the child; the child's explicit list targets are resolved before the parent's workspace targets. No duplicate Queue/list coordinate path exists. The per-surface canonical row-hit enum to `HitRegions<Target>` migration belongs to the canonical slices, not `restore-mouse-support`; Queue's row-hit migration belongs to the Queue slice, not both changes.
- Reconcile ADR 0022, `CONTEXT.md`, and the interactive-surface ledger with the reusable inner-component completion criterion after all slices land.

## Capabilities

### New Capabilities

- `canonical-media-lists`: Defines the reusable `WideMediaList` and `InlineMediaBrowser`, their exact scope, presentation vocabulary, authority, mouse seam, position handoff, and composition gates.

### Modified Capabilities

- `right-panel-arrangements`: Requires Hero-on-left rails to compose `WideMediaList` and Narrow selected-row replacement to compose `InlineMediaBrowser`, while preserving the non-hero two-column carve-out.
- `ui-design-system`: Requires the named primary media-list surfaces to use the canonical controls, makes any exception explicit and verified, requires one canonical list painter per migrated surface, and fixes the verification order so a production visual correction and explicit user live visual approval precede any UI fixture or test change.
- `interactive-component-framework`: Defines how an embedded plain TuiRealm `Component` owns reusable interaction and hit state beneath a mounted destination `AppComponent` without an independent registry identity, subscription, gesture recognizer, or second event-routing boundary.

## Impact

- **PR relationship:** PR #606 remains blocked; the implementation slices stack onto its feature branch and are reviewed independently. Squashing a family slice does not combine it with another family; each slice remains a distinct PR and rollback boundary.
- **Umbrella lifecycle:** this change remains open and unarchived until all five implementation slices, the independent `restore-feed-group-inline-expansion` and `restore-feeds-service-wide-list` fixes, the `restore-audiobookshelf-podcast-wide-layout` supersession, the cross-change mouse decision, and final gates are complete.
- **Primary code areas:** `src/app/components/`, `src/app/render/components/`, `src/app/render/arrangements/`, `src/app/shell_*.rs`, `src/app/layout.rs`, component tests, and render characterization tests.
- **Likely file splits:** `src/app/components/tv_workspace.rs`, `src/app/components/audiobookshelf_podcast.rs`, and `src/app/components/browser.rs` are near the 800-line cap. Their slice plans SHALL split them before or with new wiring rather than discovering over-limit files at final verification.
- **Active-change impact:** `restore-feed-group-inline-expansion` becomes a focused independent Emby homevideos feed view Narrow bug fix; `restore-mouse-support` records the parent-gesture/child-hit contract and removes overlapping Queue/list row-hit ownership. `restore-audiobookshelf-podcast-wide-layout` (#640) is superseded and will be archived as non-executable, not landed; only the separately landed #640 Home podcast sub-view hero-placement fix stays. The Audiobookshelf podcast *library* Wide repair and the Audiobookshelf Books repair are owned by the canonical Music/Audiobookshelf slice.
- **User-visible corrections owned by slices:** Wide Feeds remain one-column; Wide Audiobookshelf Books no longer duplicate selected detail in the right rail; every migrated destination preserves the established Hero-on-left or selected-row-replacement contract.
- No new dependency, protocol, daemon, provider, configuration, or external API change.
