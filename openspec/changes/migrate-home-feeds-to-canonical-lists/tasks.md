## 1. Preconditions and characterization

- [ ] 1.1 Keep Home paused until the corrected canonical foundation is accepted. Preserve the invalid Home/Feeds commits and dirty worktree as unaccepted evidence; when implementation resumes, record the then-current accepted feature-branch baseline SHA and confirm dependencies on the corrected foundation plus the accepted #623 Feeds Wide prerequisite (umbrella task 1.3a). The Emby homevideos feed view (#634/#637) is an out-of-scope boundary, not a prerequisite.
- [ ] 1.2 Record current Home active-section identity/cursor/scroll/restore behavior and Feeds watched/group behavior from source, existing evidence, and the running app.
- [ ] 1.3 Record current Wide/Narrow rendering, images/workspaces, selection, scrolling, and one-painter paths as the pre-replacement baseline.

## 2. Home composition

- [ ] 2.1 Prepare the active Home section's rows for canonical `Item`/`Heading`/`Spacer` vocabulary without changing section identity, `pref_key`, `restore_section`, images, or workspace effects; only the active section is projected into the control.
- [ ] 2.2 Compose persistent `InlineMediaBrowser` and `WideMediaList` controls for the applicable Home paths. Keep the active control authoritative: ordinary refresh preserves target and locally clamps without parent cursor/scroll input; breakpoint or discrete navigation transitions perform one `ViewportAnchor` handoff. Add no per-section cursor cache or App-wide interaction mirror.
- [ ] 2.3 Remove Home destination list duplication and retain existing parent-owned effects, overlays, and typed message translation. Leave the bespoke `*HitRegion` path wired and untouched — do not remove it as "duplication"; `restore-mouse-support` (#638) owns its migration. Add no new mouse wiring.

## 3. Feeds Service/tab composition

- [ ] 3.1 Prepare grouped FeedEntries as selectable `Item` rows and project FeedAgeGroup/date labels to `Heading` rows and separators to `Spacer` rows as canonical-list content, preserving stable targets and watched semantic state.
- [ ] 3.2 Compose `InlineMediaBrowser`/`WideMediaList` by named arrangement; keep the subscription/group selector pills and the watched selector as parent-owned chrome outside the canonical control, and preserve group selection, images, focus, and scroll semantics.
- [ ] 3.3 Retain the accepted #623 Wide one-column/framing baseline and implement the deferred two-space row-indent correction in canonical model/painter source-of-truth.
- [ ] 3.4 Remove bespoke Feeds list mechanics and prove one list painter runs; leave the Emby homevideos feed view (#634/#637) and non-hero two-column policies untouched. Mouse/hit geometry is out of scope: the bespoke `*HitRegion` path stays wired and untouched and `restore-mouse-support` (#638) owns its migration. The Music/Audiobookshelf canonical slice is out of scope; standalone #640 is superseded.

## 4. Tests, gates, review, and acceptance

- [ ] 4.1 Add/update the smallest focused stateful, rendered, and geometry tests with metadata-, grouping-, focus-, progress-, watched-, image-, and breakpoint-bearing fixtures; cover structural-row indexing, ordinary-refresh target retention, and target/offset anchoring.
- [ ] 4.2 Attach one-painter evidence and source-trace absence proofs; verify no parent underpaint, per-frame child construction, second router, global hit map, callback/provider framework, or authority leak.
- [ ] 4.3 Run `rtk make check-code-file-lines` and ensure every changed source file is ≤800 lines.
- [ ] 4.4 Run `rtk openspec validate migrate-home-feeds-to-canonical-lists --strict`.
- [ ] 4.5 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and the relevant `rtk cargo nextest run` suite.
- [ ] 4.6 Review the complete slice, then perform live Wide/Narrow acceptance for Home and Feeds covering selection, movement, scrolling, group/filter state, images/workspaces, selected-row replacement, rail framing, and row indent. Treat defects as bugs, fix them, and rerun affected tests and gates before acceptance.
