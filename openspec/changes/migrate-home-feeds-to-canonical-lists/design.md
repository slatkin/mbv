## Context

The canonical media-list foundation introduces two embedded controls: `InlineMediaBrowser` for single-column selected-row replacement and `WideMediaList` for fixed-height one-column rails. Home and the Feeds Service/tab are the next composition slice. The Feeds Service/tab is not the Emby homevideos feed view; #634/#637 remain the authority for that separate surface.

## Decisions

### D1 — Compose, do not duplicate
Home sections and Feeds SHALL prepare provider-neutral rows and embed the canonical controls. Existing parent components retain Service effects, selection restoration, images, workspaces, group/filter state, and typed message translation. Controls retain cursor, scroll, replacement admission, geometry, and `HitRegions`.

### D2 — Home identity and state
Home section preparation preserves stable `pref_key` and `restore_section` identity. Section-local cursor/scroll is passed through the active control and preserved on refresh and breakpoint handoff using `ViewportAnchor`; no App-wide interaction mirror is added.

### D3 — Feeds structural projection
Feed group labels become `Heading`, separators become `Spacer`, and entries become selectable `Item` rows carrying stable FeedEntry targets and watched/active semantic state. Watched selector and group selection remain parent-owned.

### D4 — #623 baseline and deferred indent
The accepted Feeds Wide one-column/framing baseline is a prerequisite, not reimplemented here. The outstanding two-space row-indent correction is applied in the canonical source-of-truth painter/model so Home and Feeds cannot drift.

### D5 — Ownership and verification
Mounted parents own mouse subscription and `MouseGestureState`; controls own child hit regions. Keyboard resolution remains solely in `router.rs`/`key_policy.rs`. Characterize current output and behavior first. Perform live Wide/Narrow visual correction and obtain explicit user confirmation before changing UI tests; then add focused buffer/geometry tests with metadata, groups, focus, progress, images, and watched states.

### D6 — Scope and stacking
Do not change non-hero two-column policies, #640, Audiobookshelf, or Emby homevideos feed-view work. Stack on PR #606's `feat/migrate-tui-to-tuirealm` branch, after accepted #634/#637, the canonical foundation, and Feeds Wide prerequisite. Keep independently reversible and enforce ≤800 lines for changed source files.

## Risks

- State jumps at variant transitions: record and assert target/offset anchors.
- Structural rows becoming selectable: test display-row versus selectable-index mapping.
- Duplicate painting/hit geometry: source-trace one-painter evidence per destination.
- Visual regressions: user live verification precedes UI test edits.
