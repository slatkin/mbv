## Why

Issue #641 found that the TuiRealm migration stopped at destination-sized `AppComponent`s: primary destinations still duplicate fixed-row list state, movement, painting, scrolling, and hit geometry. This is a blocking architecture correction for open PR #606, but it must be reviewed and landed as stacked, bounded slices rather than added as one more mega-diff.

**Tracking issue:** [GitHub issue #641](https://github.com/slatkin/mbv/issues/641). The issue owns the ordered delivery checklist; this OpenSpec change owns the architecture, requirements, and completion gates.

## What Changes

- Keep this change as the architecture and delivery umbrella for issue #641; it defines the target contracts and tracks completion but does not directly implement destination code.
- Keep PR #606 blocked. Each implementation slice is reviewed as its own PR targeting `feat/migrate-tui-to-tuirealm`; PR #606 merges only after every required slice has landed on that branch and the umbrella completion gates pass.
- Deliver five implementation slices:
  1. canonical row/viewport foundation plus generic Emby, Movies, HomeVideos, narrow TV, and the Wide TV right rail;
  2. Home and Feeds;
  3. grouped Music and Audiobookshelf Podcasts/Books;
  4. Queue's fixed-row-only adoption; and
  5. cross-family obsolete-loop deletion, architecture reconciliation, and final verification.
- Introduce a canonical reusable `WideMediaList` plain TuiRealm `Component`, embedded inside destination `AppComponent`s rather than mounted independently. It is the single-column fixed-row control for Hero-on-left rails; Queue may compose its fixed-row behavior without entering the hero/Inline presentation contract. Existing non-hero two-column browsers remain outside its scope.
- Introduce a distinct reusable `InlineMediaBrowser` plain `Component` for the defined Narrow selected-row-replacement presentation. This term is distinct from Inline Search and is added to the project vocabulary before implementation.
- Give the reusable controls live cursor, scroll, viewport, movement, visibility clamp, row placement, semantic list styling, scrollbar, and render-derived hit geometry authority.
- Define a small provider-neutral row vocabulary with selectable item rows and non-selectable heading/spacer rows. Queue progress is prepared semantic presentation data with a bounded progress payload, not provider or Player authority.
- Keep provider content, heroes, workspaces, pills, effects, image authority, persistence, and provider-specific `Msg` translation with the destination parent and shell.
- Require composition for the exact primary surfaces in scope: Home; generic Emby libraries that use the shared media browser; Movies; TV Series browsing; grouped Music album browsing; Emby home-video and podcast libraries; Audiobookshelf Podcast show browsing; Audiobookshelf Book browsing; Feeds; and Queue's fixed-row list. Any true exception is recorded as a named bespoke surface.
- Ship #634/#637 independently through the existing focused Feed change before the Home/Feeds slice; it establishes a green Narrow baseline that the canonical Inline control later replaces. The conflicting Wide expansion is removed from that small fix.
- Ship the independent `restore-feeds-service-wide-list` prerequisite for issue #623 before the Home/Feeds slice. It corrects only the Feeds Service/tab Wide panel; it is separate from accepted #634/#637, whose scope is the Emby homevideos feed view.
- Ship #640 independently as the focused Audiobookshelf Podcast Hero-on-left arrangement correction before the Music/Audiobookshelf slice.
- Resolve mouse ownership jointly with `restore-mouse-support`: the mounted destination parent owns the mouse subscription and `MouseGestureState`; the embedded list owns `HitRegions<Target>` populated by its own view; the parent recognizes a gesture and delegates point resolution to the child. Queue row-hit migration belongs to the Queue slice, not both changes.
- Reconcile ADR 0022, `CONTEXT.md`, and the interactive-surface ledger with the reusable inner-component completion criterion after all slices land.

## Capabilities

### New Capabilities

- `canonical-media-lists`: Defines the reusable `WideMediaList` and `InlineMediaBrowser`, their exact scope, presentation vocabulary, authority, mouse seam, position handoff, and composition gates.

### Modified Capabilities

- `right-panel-arrangements`: Requires Hero-on-left rails to compose `WideMediaList` and Narrow selected-row replacement to compose `InlineMediaBrowser`, while preserving the non-hero two-column carve-out.
- `ui-design-system`: Requires the named primary media-list surfaces to use the canonical controls and makes any exception explicit and verified.
- `interactive-component-framework`: Defines how an embedded plain TuiRealm `Component` owns reusable interaction and hit state beneath a mounted destination `AppComponent` without an independent registry identity, subscription, gesture recognizer, or second event-routing boundary.

## Impact

- **PR relationship:** PR #606 remains blocked; the implementation slices stack onto its feature branch and are reviewed independently. Squashing a family slice does not combine it with another family; each slice remains a distinct PR and rollback boundary.
- **Umbrella lifecycle:** this change remains open and unarchived until all five implementation slices, independent Feed/Podcast fixes, cross-change mouse decision, and final gates are complete.
- **Primary code areas:** `src/app/components/`, `src/app/render/components/`, `src/app/render/arrangements/`, `src/app/shell_*.rs`, `src/app/layout.rs`, component tests, and render characterization tests.
- **Likely file splits:** `src/app/components/tv_workspace.rs`, `src/app/components/audiobookshelf_podcast.rs`, and `src/app/components/browser.rs` are near the 800-line cap. Their slice plans SHALL split them before or with new wiring rather than discovering over-limit files at final verification.
- **Active-change impact:** `restore-feed-group-inline-expansion` becomes a focused independent Narrow bug fix; `restore-mouse-support` records the parent-gesture/child-hit contract and removes overlapping Queue/list row-hit ownership; #640 lands independently.
- **User-visible corrections owned by slices:** Wide Feeds remain one-column; Wide Audiobookshelf Books no longer duplicate selected detail in the right rail; every migrated destination preserves the established Hero-on-left or selected-row-replacement contract.
- No new dependency, protocol, daemon, provider, configuration, or external API change.
