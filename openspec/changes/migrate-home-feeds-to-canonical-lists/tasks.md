## 1. Preconditions and characterization

- [ ] 1.1 Record the then-current accepted feature-branch baseline SHA when implementation is issued; confirm PR #606 feature-branch stacking and dependencies on the landed canonical-list foundation and the accepted #623 Feeds Wide prerequisite (umbrella task 1.3a). The Emby homevideos feed view (#634/#637) is an out-of-scope boundary, not a prerequisite.
- [ ] 1.2 Characterize current Home active-section identity/cursor/scroll/restore behavior and Feeds watched/group behavior by reading source and manually observing the running app only. This step makes no test or fixture edits; any test-fixture change is gated behind explicit user live visual approval (task 4.1).
- [ ] 1.3 Record current Wide/Narrow rendering, images/workspaces, selection, scrolling, and one-painter paths by source-reading and manual observation only; obtain explicit user live visual approval after visual correction before changing or adding any UI test or fixture.

## 2. Home composition

- [ ] 2.1 Prepare the active Home section's rows for canonical `Item`/`Heading`/`Spacer` vocabulary without changing section identity, `pref_key`, `restore_section`, images, or workspace effects; only the active section is projected into the control.
- [ ] 2.2 Compose `InlineMediaBrowser` for the inline Home section and `WideMediaList` for the approved Wide rail; carry the single active-section cursor/scroll through the control and preserve `ViewportAnchor` transitions for refresh and breakpoint handoff, with no per-section cursor cache and no App-wide interaction mirror.
- [ ] 2.3 Remove Home destination list duplication and retain parent-owned effects, overlays, mouse subscription, and typed message translation.

## 3. Feeds Service/tab composition

- [ ] 3.1 Prepare grouped FeedEntries as selectable `Item` rows and project FeedAgeGroup/date labels to `Heading` rows and separators to `Spacer` rows as canonical-list content, preserving stable targets and watched semantic state.
- [ ] 3.2 Compose `InlineMediaBrowser`/`WideMediaList` by named arrangement; keep the subscription/group selector pills and the watched selector as parent-owned chrome outside the canonical control, and preserve group selection, images, focus, and scroll semantics.
- [ ] 3.3 Retain the accepted #623 Wide one-column/framing baseline and implement the deferred two-space row-indent correction in canonical model/painter source-of-truth.
- [ ] 3.4 Remove bespoke Feeds list mechanics and prove one painter/one hit-geometry path; leave the Emby homevideos feed view (#634/#637) and non-hero two-column policies untouched. The Music/Audiobookshelf canonical slice is out of scope; standalone #640 is superseded.

## 4. Tests and gates (after visual approval)

- [ ] 4.1 After explicit user live approval, add/update focused rendered and geometry tests with metadata-, grouping-, focus-, progress-, watched-, image-, and breakpoint-bearing fixtures; cover structural-row indexing and target/offset anchoring.
- [ ] 4.2 Perform manual/live Wide and Narrow checks for Home and Feeds: selection, movement, scrolling, group/filter state, images/workspaces, selected-row replacement, rail framing, and row indent.
- [ ] 4.3 Attach one-painter evidence and source-trace absence proofs; verify no second router, global hit map, callback/provider framework, or authority leak.
- [ ] 4.4 Run `rtk make check-code-file-lines` and ensure every changed source file is ≤800 lines.
- [ ] 4.5 Run `rtk openspec validate migrate-home-feeds-to-canonical-lists --strict`.
- [ ] 4.6 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and the relevant `rtk cargo nextest run` suite.
