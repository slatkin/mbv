## 1. Lock The Placement Contract

- [x] 1.1 Add ADR 0021 recording hero-on-left as the sole wide hero arrangement, inline hero as the fallback presentation, short-height inline degradation, and rejection of retained hero-on-top compatibility.
- [x] 1.2 Update `CONTEXT.md` to remove Hero-on-top, define Inline hero, narrow Hero-on-left to the sole wide arrangement, and reconcile Panel mode descriptions.
- [x] 1.3 Add a focused invariant check or test inventory covering every hero-bearing surface at wide, narrow, and width-wide/height-short geometry before changing render paths.

## 2. Make Shared Hero Plumbing Placement-Neutral

- [x] 2.1 Add failing focused tests for inline flow insertion, variable detail height, insufficient-height suppression, scroll visibility, inert hero rows, and explicit child targets.
- [x] 2.2 Extract or reuse the minimum common inline display-flow accounting needed by generic Emby, Home, Feeds, and Audiobookshelf browsers without introducing the #563 component framework.
- [x] 2.3 Rename the selected-detail shell and border policy around framing/focus rather than top placement, preserving the current inline visual treatment.
- [x] 2.4 Centralize the width plus minimum-height placement decision and hero-on-left pane geometry so surface renderers do not define their own thresholds.
- [x] 2.5 Verify already-correct Movies, TV, and grouped Music remain hero-on-left when wide and inline when narrow or height-constrained.

## 3. Convert Audiobookshelf Surfaces

- [x] 3.1 Add failing Audiobookshelf podcast render and interaction tests for wide left workspace, one-column right rail, narrow inline selected-show detail, scrolling, filters, episodes, and short-height fallback.
- [x] 3.2 Convert Audiobookshelf podcasts to shared hero-on-left and inline placement while preserving cover loading, metadata, played-state filters, episode state, focus, and provider-native identities.
- [x] 3.3 Add failing Audiobookshelf book tests for narrow inline selected-book/chapter detail, inert framing, chapter targets, surname pills, scrolling, and suppression while retaining wide behavior.
- [x] 3.4 Replace the Audiobookshelf book top fallback with inline selected-book detail and preserve its existing wide left workspace and pane-focus behavior.

## 4. Convert Feeds And Home

- [x] 4.1 Add failing Feeds tests for read-only wide left detail, group/watched selectors plus one-column right rail, narrow inline selected-entry detail, grouped headings, scrolling, hit targets, empty/loading states, and short-height fallback.
- [x] 4.2 Convert Feeds to shared hero-on-left and inline placement without changing filter state, grouping order, played indicators, entry identities, or playback actions.
- [x] 4.3 Add failing Home tests for inline detail in the selected section's flow across Emby, Audiobookshelf, and Feed items, including section pills, scrolling, inert detail rows, empty sections, and short-height suppression.
- [x] 4.4 Replace Home's narrow pinned hero with inline selected-item detail while preserving its existing wide hero-on-left cards, provider-specific content, section state, and playback routing.

## 5. Complete Emby Wide Coverage

- [x] 5.1 Add failing wide tests for Emby podcast and home-video browsing that require left selected detail, a one-column right rail, correctly owned pills/count/search controls, and unchanged activation.
- [x] 5.2 Add explicit shared hero-on-left composition for Emby podcast browsing while preserving Series/episode detail and existing interaction state.
- [x] 5.3 Add explicit shared hero-on-left composition for Emby home-video browsing while preserving selected-media content, count/search behavior, image handling, and row activation.
- [x] 5.4 Verify every remaining hero-bearing Emby library selects inline rather than top placement when the minimum-height guard fails.

## 6. Delete Hero-On-Top

- [x] 6.1 Remove the top layout structure/helper and any reservation path after the final caller is migrated; replace any zero-height helper use with placement-neutral pill/list geometry.
- [x] 6.2 Remove `SelectedBlockBorderStyle::HeroOnTop`, top-specific hero activation handling, and obsolete layout state that has no inline or left use.
- [x] 6.3 Remove or rewrite stale top-placement tests, comments, module docs, and current source identifiers without altering archived OpenSpec history.
- [x] 6.4 Run a scoped repository search proving current source, tests, live specs, `CONTEXT.md`, and current ADRs contain no hero-on-top terminology or symbols.

## 7. Verify And Hand Off

- [x] 7.1 Run focused Ratatui render and input tests for all surface families at wide, narrow, and height-constrained dimensions, including images-disabled behavior.
- [x] 7.2 Run `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, and `rtk make check-code-file-lines`.
- [x] 7.3 Validate `eliminate-hero-on-top`, sync its deltas into the live specs, and confirm the live requirements contain no contradictory top fallback.
- [x] 7.4 Re-read the resulting render tree and revise the paused `enforce-mbv-ui-design-system` proposal, design, specs, and tasks to remove completed migration assumptions before #563 implementation begins.
