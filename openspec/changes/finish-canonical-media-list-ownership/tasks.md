# Finish Canonical Media-List Ownership Campaign Ledger

This planning-only ledger records campaign status. It does not authorize proposal creation or implementation. Before any pending destination row advances, the user must select that row for interactive exploration, confirm its scope, authorize one bounded proposal, and later issue a separate apply request.

A row closes only when its bounded change is accepted, synced where applicable, archived, and represented consistently here and in GitHub issue #681 with durable requirement-to-source-and-test evidence. Non-grouped Music and the Emby podcast channel list are outside this campaign and are not ledger rows.

## 1. Establish the Corrected Baseline

- [x] 1.1 Record PR #684 (`f647136a`) as the accepted component-view and retained-current-frame geometry foundation; verify its archived change remains at `openspec/changes/archive/2026-09-09-repair-canonical-media-list-ownership/`.
- [x] 1.2 Record Queue as complete under the full current contract; verify the audit found a persistent `WideMediaList<QueueSlotId>`, stable-target refresh preservation, explicit re-anchor, control-owned movement, retained-geometry point resolution, and no parent cursor/row-map mirror.
- [x] 1.3 Correct the prior Grouped Music completion claim to incomplete; verify the audit records parent album cursor/scroll, inactive-control synchronization, render-time target/scroll reseeding, paint-result writeback, and compatibility geometry.
- [x] 1.4 Record the remaining audited destination failures and preserved boundaries in `design.md` D2; verify Home, Feeds, the Browser family, TV Series, Audiobookshelf Podcasts, and Audiobookshelf Books each have a named evidence-backed gap while non-hero two-column catalogs remain preserved.
- [ ] 1.5 Establish the paired human record in GitHub issue #681; verify #681 links this umbrella, carries the same status matrix and authorization boundary, and neither record contradicts the other.

## 2. Complete Grouped Music

- [ ] 2.1 After separately authorized exploration, accept and archive one bounded Grouped Music ownership change; verify the active control solely owns album position, ordinary refresh does not reseed either presentation, a breakpoint transition uses one `ViewportAnchor`, paint does not write position back, pointer resolution uses retained geometry, and the Music track workspace remains Music-owned.
- [ ] 2.2 Record the accepted change path, PR/commit, requirement-to-source trace, focused component/buffer/live-`Application::tick()` evidence, Normal/Wide human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing Grouped Music.

## 3. Complete Home

- [ ] 3.1 After separately authorized exploration, accept and archive one bounded Home ownership change; verify only the active Wide/Inline control moves, shell/per-section cursor mirrors are removed or explicitly proven non-live, ordinary refresh preserves local target, one `ViewportAnchor` performs responsive handoff, and row hits use retained current-frame geometry while Home sections, pills, hero, images, and effects remain parent/shell-owned.
- [ ] 3.2 Record the accepted change path, PR/commit, requirement-to-source trace, focused component/buffer/live-`Application::tick()` evidence, Normal/Wide human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing Home.

## 4. Complete Feeds

- [ ] 4.1 After separately authorized exploration, accept and archive one bounded Feeds ownership change; verify only the active Wide/Inline control moves, ordinary refresh preserves local target, one `ViewportAnchor` performs responsive handoff, parent row maps and compatibility point resolution are removed, group/heading structural rows and selector/filter parent ownership remain, and Feeds presentation is conformed to the Emby-derived canonical design with deviations repaired rather than preserved.
- [ ] 4.2 Record the accepted change path, PR/commit, requirement-to-source trace, focused component/buffer/live-`Application::tick()` evidence, Normal/Wide human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing Feeds.

## 5. Complete Movies and the Emby homevideos feed view

- [ ] 5.1 After separately authorized exploration, accept and archive one bounded Browser-family ownership change for Movies and the Emby homevideos feed view; verify embedded controls solely own canonical cursor/scroll, row hits use retained current-frame geometry, ordinary refresh and explicit re-anchor remain distinct, and each visible screen's provider behavior is preserved.
- [ ] 5.2 Verify the same bounded change preserves non-hero two-column catalogs as isolated screen-owned grid interaction with arrangement-owned placement; record evidence that canonical controls neither read nor overwrite grid state.
- [ ] 5.3 Record the accepted change path, PR/commit, requirement-to-source trace, focused evidence for every named screen and the preserved grid, Normal/Wide human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing the Browser family.

## 6. Complete TV Series

- [ ] 6.1 After section 5 is accepted and after separately authorized exploration, accept and archive one bounded TV Series ownership change; verify the Wide series control has no parent cursor mirror, Normal/Wide handoff preserves one stable target/row offset, row hits resolve only painted retained geometry with blank/header space unclaimed, and seasons, episodes, season pills, and the episode workspace retain their accepted TV ownership.
- [ ] 6.2 Record the accepted change path, PR/commit, requirement-to-source trace, focused component/buffer/live-`Application::tick()` evidence across Normal/Wide and provider workspace behavior, human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing TV Series.

## 7. Complete Audiobookshelf Podcasts

- [ ] 7.1 After separately authorized exploration, accept and archive one bounded Audiobookshelf Podcast ownership change; verify the show-list controls persist across frames, the active control owns show position, ordinary refresh and responsive handoff follow the shared contract, row hits use retained geometry without parent show-row maps, and paint does not write position back; episode filter/selection state, images, and playback intents remain Podcast-owned authority, and Podcast presentation is conformed to the Emby-derived canonical design with deviations repaired rather than preserved.
- [ ] 7.2 Record the accepted change path, PR/commit, requirement-to-source trace, focused component/buffer/live-`Application::tick()` evidence, Normal/Wide human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing Audiobookshelf Podcasts.

## 8. Complete Audiobookshelf Books

- [ ] 8.1 After separately authorized exploration, accept and archive one bounded Audiobookshelf Book ownership change; verify the book-list controls persist across frames, the active control owns book position, ordinary refresh and responsive handoff follow the shared contract, book-row hits use retained geometry without parent maps or paint writeback; surname buckets, chapter state, images, and absolute chapter-seek authority remain Book-owned, and Book presentation is conformed to the Emby-derived canonical design with deviations repaired rather than preserved.
- [ ] 8.2 Resolve chapter-list geometry within the same exploration only if required by the applicable current specification; verify the accepted scope explicitly distinguishes canonical book-list ownership from legitimate provider-workspace chapter state rather than silently expanding the change.
- [ ] 8.3 Record the accepted change path, PR/commit, requirement-to-source trace, focused component/buffer/live-`Application::tick()` evidence, Normal/Wide human evidence or explicit waiver, spec-sync disposition, and matching GitHub status before closing Audiobookshelf Books.

## 9. Reconcile and Close

- [ ] 9.1 After sections 2-8 are complete, run a fresh source-to-spec audit over every included destination; verify every applicable requirement maps to a source owner and passing evidence, with explicit reviewer dispositions for unclear or not-applicable clauses.
- [ ] 9.2 Correct only the stale comments, contradictory Wide-orientation scenario titles, compatibility paths, and narrow structural enforcement still proven necessary by the final audit; verify no destination implementation is folded into reconciliation.
- [ ] 9.3 Run final focused and project gates required by the accepted changes, including formatting, `cargo check -p mbv`, relevant `cargo nextest run -p mbv`, clippy, `ast-grep scan`, and the file-size gate; record any unrelated failure without marking this campaign complete.
- [ ] 9.4 Verify this ledger and GitHub issue #681 agree on every row, evidence link, and completion state; archive this umbrella and close #681 only after final OpenSpec validation succeeds and no included row remains open.
