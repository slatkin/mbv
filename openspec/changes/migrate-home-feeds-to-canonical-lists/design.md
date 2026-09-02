## Context

The canonical media-list foundation introduces two embedded controls: `InlineMediaBrowser` for single-column selected-row replacement and `WideMediaList` for fixed-height one-column rails. Home and the Feeds Service/tab are the next composition slice. The Feeds Service/tab is not the Emby homevideos feed view; #634/#637 remain the authority for that separate surface.

## Decisions

### D1 — Compose, do not duplicate
Home sections and Feeds SHALL prepare provider-neutral rows and embed persistent canonical controls. Existing parent components retain Service effects, selection restoration, images, workspaces, group/filter state, and typed message translation. Controls retain cursor, scroll, replacement admission, and internal paint/scroll geometry.

### D2 — Home identity and state
Home section preparation preserves stable `pref_key` and `restore_section` identity. Home has exactly one active section and one flat cursor/scroll position owned by the active canonical control; only the active section's rows are projected into that control. Ordinary refresh preserves target and locally clamps without adopting parent cursor/scroll. Breakpoint or discrete navigation transitions perform one `ViewportAnchor` handoff. No per-section cursor cache and no App-wide interaction mirror are added.

### D3 — Feeds structural projection
As canonical-list content: FeedAgeGroup / date labels become non-selectable `Heading` rows, separators become non-selectable `Spacer` rows, and feed entries become selectable `Item` rows carrying stable FeedEntry targets and watched/active semantic state. As parent-owned chrome outside the canonical control: the subscription/group selector pills and the watched selector stay owned by the Feeds parent and are never projected as canonical rows. Group selection remains parent-owned.

### D4 — #623 baseline and deferred indent
The accepted Feeds Wide one-column/framing baseline is a prerequisite, not reimplemented here. The outstanding two-space row-indent correction is applied in the canonical source-of-truth painter/model so Home and Feeds cannot drift.

### D5 — Ownership and verification
Mouse: out of scope; `restore-mouse-support` (#638) owns it and lands last. This slice adds no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and existing bespoke `*HitRegion` paths stay wired and untouched.

Keyboard resolution stays solely in `router.rs`/`key_policy.rs`. Implementation, focused stateful and rendered tests, automated gates, review, and acceptance form one uninterrupted slice. There is no pre-test visual-approval checkpoint. Live Wide/Narrow review remains required; defects found there are fixed as bugs before rerunning affected tests and gates. Evidence includes metadata, groups, focus, progress, images, watched states, stable targets, and viewport anchors.

### D6 — Scope and stacking
Do not change non-hero two-column policies or Emby homevideos feed-view work. The Music/Audiobookshelf canonical slice is out of scope; standalone #640 is superseded. The Emby homevideos feed view (#634/#637) is an out-of-scope boundary note, not a prerequisite. Stack on PR #606's `feat/migrate-tui-to-tuirealm` branch, after the corrected canonical foundation is accepted and the #623 Feeds Wide prerequisite has landed. Home remains paused until then; the invalid Home/Feeds commits and dirty worktree are retained as unaccepted evidence, not as the slice baseline. Keep independently reversible and enforce ≤800 lines for changed source files.

## Risks

- State jumps at variant transitions: record and assert target/offset anchors.
- Structural rows becoming selectable: test display-row versus selectable-index mapping.
- Duplicate list painting: source-trace one-painter evidence per destination.
- Visual regressions: representative tests and gates precede live review; defects found during review are fixed and the affected gates rerun before acceptance.
