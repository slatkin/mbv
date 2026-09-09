# Finish Canonical Media-List Ownership Campaign Ledger

This planning-only ledger records campaign status. It does not authorize proposal creation or implementation. Before any pending destination row advances, the user must select that row for interactive exploration, confirm its scope, authorize one bounded proposal, and later issue a separate apply request.

A row closes when its bounded change is merged plus reviewer-signed-off and both this ledger and GitHub issue #681 record the status. No per-requirement trace or live-evidence bundle is required. The Emby families (sections 2-5) complete before the skeleton families (sections 6-8). Non-grouped Music is outside this campaign. The obsolete Emby podcast channel-list requirement has been removed; neither is a ledger row.

## 1. Establish the Corrected Baseline

- [x] 1.1 Record PR #684 (`f647136a`) as the accepted component-view and retained-current-frame geometry foundation; verify its archived change remains at `openspec/changes/archive/2026-09-09-repair-canonical-media-list-ownership/`.
- [x] 1.2 Record Queue as complete under the full current contract; verify the audit found a persistent `WideMediaList<QueueSlotId>`, stable-target refresh preservation, explicit re-anchor, control-owned movement, retained-geometry point resolution, and no parent cursor/row-map mirror.
- [x] 1.3 Correct the prior Grouped Music completion claim to incomplete; verify the audit records parent album cursor/scroll, inactive-control synchronization, render-time target/scroll reseeding, paint-result writeback, and compatibility geometry.
- [x] 1.4 Record the remaining audited destination failures and preserved boundaries in `design.md` D2; verify Home, the Browser family, TV Series, Feeds, Audiobookshelf Podcasts, and Audiobookshelf Books each have a named evidence-backed gap while non-hero two-column catalogs remain preserved.
- [x] 1.5 Establish the paired human record in GitHub issue #681; verify #681 links this umbrella, carries the same status matrix and authorization boundary, and neither record contradicts the other.

## 2. Complete Grouped Music

- [x] 2.1 After separately authorized exploration, merge one bounded Grouped Music ownership change with reviewer sign-off; verify the active control solely owns album position, ordinary refresh does not reseed either presentation, a breakpoint transition uses one `ViewportAnchor`, paint does not write position back, pointer resolution uses retained geometry, and the Music track workspace remains Music-owned.
- [x] 2.2 Record the change path and matching GitHub status before closing Grouped Music.
  - Change path: `fix-grouped-music-media-list-ownership` implemented at `99e53fd9` + correction `e69bb2f5` on `refactor/finish-canonical-media-list-ownership`; two-axis review BLOCK, correction, focused re-review PASS. GitHub #681 updated to match.

## 3. Complete Home

- [x] 3.1 After separately authorized exploration, merge one bounded Home ownership change with reviewer sign-off; verify only the active Wide/Inline control moves, shell/per-section cursor mirrors are removed or explicitly proven non-live, ordinary refresh preserves local target, one `ViewportAnchor` performs responsive handoff, and row hits use retained current-frame geometry while Home sections, pills, hero, images, and effects remain parent/shell-owned.
- [x] 3.2 Record the change path and matching GitHub status before closing Home.
  - Change path: `fix-home-media-list-ownership` implemented at `54e704f8` + tests `d74309b1` + correction `23291256` on `refactor/finish-canonical-media-list-ownership`; two-axis review BLOCK, correction, focused re-review PASS. CW-only menu/toggle semantics documented as intended. GitHub #681 updated to match.

## 4. Complete Movies and the Emby homevideos feed view

- [x] 4.1 After separately authorized exploration, merge one bounded Browser-family ownership change for Movies and the Emby homevideos feed view with reviewer sign-off; verify embedded controls solely own canonical cursor/scroll, row hits use retained current-frame geometry, ordinary refresh and explicit re-anchor remain distinct, and each visible screen's provider behavior is preserved.
- [x] 4.2 Verify the same bounded change preserves non-hero two-column catalogs as isolated screen-owned grid interaction with arrangement-owned placement; record evidence that canonical controls neither read nor overwrite grid state.
- [x] 4.3 Record the change path and matching GitHub status before closing the Browser family.
  - Change path: `fix-browser-media-list-ownership` implemented at `7658e07e` (partial) → `c468fb05` (dilution, recovered) → `880c9b17` (canonical seam restored) → `638162ba` (tasks 1.1/1.2 tests) → `a70fffee` (persistence/seed/anchor correction) → `466bfc83` (apply_position branch tests) on `refactor/finish-canonical-media-list-ownership`; two-axis review BLOCK → correction → focused re-review verified findings fixed → final mechanical closure. GitHub #681 updated to match. Deferred to row 9.3: `browser/mod.rs` 825 and `browser_component_tests.rs` 840 lines exceed the 800-line ceiling (campaign-end constraint only).

## 5. Complete TV Series

- [ ] 5.1 After section 4 is accepted and after separately authorized exploration, merge one bounded TV Series ownership change with reviewer sign-off; verify the Wide series control has no parent cursor mirror, Normal/Wide handoff preserves one stable target/row offset, row hits resolve only painted retained geometry with blank/header space unclaimed, and seasons, episodes, season pills, and the episode workspace retain their accepted TV ownership.
- [ ] 5.2 Record the change path and matching GitHub status before closing TV Series.

## 6. Complete Feeds

- [ ] 6.1 After sections 2-5 are complete and after separately authorized exploration, merge one bounded Feeds ownership change with reviewer sign-off; verify only the active Wide/Inline control moves, ordinary refresh preserves local target, one `ViewportAnchor` performs responsive handoff, parent row maps and compatibility point resolution are removed, group/heading structural rows and selector/filter parent ownership remain, and Feeds presentation converges fully on the Emby reference (visual design, chrome, framing, interaction) with deviations repaired rather than preserved.
- [ ] 6.2 Record the change path and matching GitHub status before closing Feeds.

## 7. Complete Audiobookshelf Podcasts

- [ ] 7.1 After sections 2-5 are complete and after separately authorized exploration, merge one bounded Audiobookshelf Podcast ownership change with reviewer sign-off; verify the show-list controls persist across frames, the active control owns show position, ordinary refresh and responsive handoff follow the shared contract, row hits use retained geometry without parent show-row maps, and paint does not write position back; episode filter/selection state, images, and playback intents remain Podcast-owned authority, and Podcast presentation converges fully on the Emby reference with deviations repaired rather than preserved.
- [ ] 7.2 Record the change path and matching GitHub status before closing Audiobookshelf Podcasts.

## 8. Complete Audiobookshelf Books

- [ ] 8.1 After sections 2-5 are complete and after separately authorized exploration, merge one bounded Audiobookshelf Book ownership change with reviewer sign-off; verify the book-list controls persist across frames, the active control owns book position, ordinary refresh and responsive handoff follow the shared contract, book-row hits use retained geometry without parent maps or paint writeback; surname buckets, chapter state, images, and absolute chapter-seek authority remain Book-owned, and Book presentation converges fully on the Emby reference with deviations repaired rather than preserved.
- [ ] 8.2 Resolve chapter-list geometry within the same exploration only if required by the applicable current specification; verify the accepted scope explicitly distinguishes canonical book-list ownership from legitimate provider-workspace chapter state rather than silently expanding the change.
- [ ] 8.3 Record the change path and matching GitHub status before closing Audiobookshelf Books.

## 9. Reconcile and Close

- [ ] 9.1 After sections 2-8 are complete, run a final review pass over every included destination.
- [ ] 9.2 Correct only the stale comments, contradictory Wide-orientation scenario titles, compatibility paths, and narrow structural enforcement still judged necessary in that review; verify no destination implementation is folded into reconciliation.
- [ ] 9.3 Run final focused and project gates required by the accepted changes, including formatting, `cargo check -p mbv`, relevant `cargo nextest run -p mbv`, clippy, `ast-grep scan`, and the file-size gate; record any unrelated failure without marking this campaign complete.
- [ ] 9.4 Verify this ledger and GitHub issue #681 agree on every row and completion state; archive this umbrella and close #681 only after final OpenSpec validation succeeds and no included row remains open.
