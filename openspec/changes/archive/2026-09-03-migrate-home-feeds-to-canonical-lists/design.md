## Context

The canonical media-list foundation introduces two embedded controls: `InlineMediaBrowser` for single-column selected-row replacement and `WideMediaList` for fixed-height one-column rails. Home and the Feeds Service/tab are the next composition slice. The Feeds Service/tab is not the Emby homevideos feed view; #634/#637 remain the authority for that separate surface.

## Decisions

### D1 — Compose, do not duplicate
Home sections and Feeds SHALL prepare provider-neutral rows and embed persistent canonical controls. Existing parent components retain Service effects, selection restoration, images, workspaces, group/filter state, and typed message translation. Controls retain cursor, scroll, replacement admission, and internal paint/scroll geometry.

### D2 — Home identity and state
Home section preparation preserves stable `pref_key` and `restore_section` identity. Home has exactly one active section and one flat cursor/scroll position owned by the active canonical control; only the active section's rows are projected into that control. Ordinary refresh preserves target and locally clamps without adopting parent cursor/scroll. Breakpoint or discrete navigation transitions perform one `ViewportAnchor` handoff. No per-section cursor cache and no App-wide interaction mirror are added.

### D3 — Feeds structural projection
As canonical-list content: FeedAgeGroup / date labels become non-selectable `Heading` rows, separators become non-selectable `Spacer` rows, and feed entries become selectable `Item` rows carrying stable FeedEntry targets and watched/active semantic state. As parent-owned chrome outside the canonical control: the subscription/group selector pills and the watched selector stay owned by the Feeds parent and are never projected as canonical rows. The arrangement hands the canonical control a sub-rect below the pill strip; the control never paints the pill region. Group selection remains parent-owned.

### D4 — `restore-feeds-service-wide-list` baseline and deferred indent
The accepted Feeds Wide one-column/framing baseline (umbrella task 1.3a) is a prerequisite, not reimplemented here. The outstanding two-space row-indent correction is applied in the canonical source-of-truth painter/model — the live defect locus is the "Canonical Feeds/Home rows share the two-column rail indent" behavior in `src/app/render/components/media_list.rs` — so Home and Feeds cannot drift.

### D5 — Ownership and verification
Mouse: out of scope; `restore-mouse-support` (#638) owns it and lands last. This slice adds no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and existing bespoke `*HitRegion` paths stay wired and untouched.

Keyboard resolution stays solely in `router.rs`/`key_policy.rs`. Implementation, focused stateful and rendered tests, automated gates, review, and acceptance form one uninterrupted slice. There is no pre-test visual-approval checkpoint. Live Wide/Narrow review remains required; defects found there are fixed as bugs before rerunning affected tests and gates. Evidence includes metadata, groups, focus, progress, images, watched states, stable targets, and viewport anchors.

### D6 — Scope and stacking
Do not change non-hero two-column policies or Emby homevideos feed-view work. The Music/Audiobookshelf canonical slice is out of scope; standalone #640 is superseded. The Emby homevideos feed view (#634/#637) is an out-of-scope boundary note, not a prerequisite. Stack on PR #606's `feat/migrate-tui-to-tuirealm` branch on top of the merged and archived canonical foundation (merge `a72f60f9`, archive `9122cc1b`), after the `restore-feeds-service-wide-list` Feeds Wide prerequisite (umbrella task 1.3a) has landed. The slice baseline contains the unaccepted Home/Feeds wiring (`173bdba1`/`400c0b59`) in-tree; this slice reworks that wiring to the canonical contract rather than composing from scratch or reverting silently. The stashed `home-wip-paused-during-canonical-merge` work is retained as unaccepted evidence alongside it. The main spec's "Named destinations compose without changing provider authority" requirement (`openspec/specs/canonical-media-lists/spec.md`) still enumerates only the five foundation destinations and is NOT modified by this change's delta — Home/Feeds coverage is carried by this change's ADDED requirements, and the umbrella's 5.3a reconciliation owns the umbrella-wide mapping. Keep independently reversible and enforce ≤800 lines for changed source files.

### D7 — Shared arrangement owns the status-row reserve
The one-row reserve above the status bar SHALL be applied once, by the shared hero-on-left arrangement primitive (`hero_left::shared_hero_presentation` / `hero_on_left_panes`), not re-derived per screen. Sibling tabs currently hand-roll it in two places, one per pane (`library.rs` hero-pane `saturating_sub(1)`; `hero_on_left_right_pane` `bottom_pad`), and the Feeds Wide arrangement historically applied neither, so its panels touched the status bar. `89ef789d` is a per-tab stopgap for Feeds; this slice folds the reserve into the shared primitive, removes the per-tab derivations, and records the invariant in the arrangement spec. This is consolidation, not a visual change: the rendered result matches the sibling tabs' existing one-row gap.

## Risks

- State jumps at variant transitions: record and assert target/offset anchors.
- Status-row reserve consolidation (D7) shifts every hero-on-left screen if both per-pane derivations are not fully unwound: the net reserve today is already exactly one row, so any rendered row shift is a defect. Validate rendered geometry against Movies, TV, Music, Home, and Feeds in both Wide and Narrow, with a per-family geometry test asserting exactly one blank row above the status bar, before removing the per-tab reserves.
- Structural rows becoming selectable: test display-row versus selectable-index mapping.
- Duplicate list painting: source-trace one-painter evidence per destination.
- Visual regressions: representative tests and gates precede live review; defects found during review are fixed and the affected gates rerun before acceptance.
