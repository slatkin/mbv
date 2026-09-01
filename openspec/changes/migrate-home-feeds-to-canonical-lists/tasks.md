## 1. Preconditions and characterization

- [ ] 1.1 Confirm accepted HEAD 8b8df5c, PR #606 feature-branch stacking, and dependencies on accepted #634/#637, canonical foundation, and Feeds Wide prerequisite.
- [ ] 1.2 Characterize current Home section identity/cursor/scroll/restore behavior and Feeds watched/group behavior with metadata-bearing fixtures before UI test edits.
- [ ] 1.3 Record current Wide/Narrow rendering, images/workspaces, selection, scrolling, and one-painter paths; obtain explicit user live verification after visual correction before changing UI tests.

## 2. Home composition

- [ ] 2.1 Prepare Home rows for canonical `Item`/`Heading`/`Spacer` vocabulary without changing section identity, `pref_key`, `restore_section`, images, or workspace effects.
- [ ] 2.2 Compose `InlineMediaBrowser` for inline Home sections and `WideMediaList` for the approved Wide rail; preserve per-section cursor/scroll and `ViewportAnchor` transitions.
- [ ] 2.3 Remove Home destination list duplication and retain parent-owned effects, overlays, mouse subscription, and typed message translation.

## 3. Feeds Service/tab composition

- [ ] 3.1 Prepare grouped FeedEntries as selectable Items; project group labels to Heading and separators to Spacer, preserving stable targets and watched semantic state.
- [ ] 3.2 Compose `InlineMediaBrowser`/`WideMediaList` by named arrangement; preserve watched selector, group selection, images, focus, and scroll semantics.
- [ ] 3.3 Retain the accepted #623 Wide one-column/framing baseline and implement the deferred two-space row-indent correction in canonical model/painter source-of-truth.
- [ ] 3.4 Remove bespoke Feeds list mechanics and prove one painter/one hit-geometry path; leave Emby homevideos feed view (#634/#637), #640, Audiobookshelf, and non-hero two-column policies untouched.

## 4. Tests and gates (after visual approval)

- [ ] 4.1 After explicit user live approval, add/update focused rendered and geometry tests with metadata-, grouping-, focus-, progress-, watched-, image-, and breakpoint-bearing fixtures; cover structural-row indexing and target/offset anchoring.
- [ ] 4.2 Perform manual/live Wide and Narrow checks for Home and Feeds: selection, movement, scrolling, group/filter state, images/workspaces, selected-row replacement, rail framing, and row indent.
- [ ] 4.3 Attach one-painter evidence and source-trace absence proofs; verify no second router, global hit map, callback/provider framework, or authority leak.
- [ ] 4.4 Run `rtk make check-code-file-lines` and ensure every changed source file is ≤800 lines.
- [ ] 4.5 Run `rtk openspec validate migrate-home-feeds-to-canonical-lists --strict`.
- [ ] 4.6 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and the relevant `rtk cargo nextest run` suite.
