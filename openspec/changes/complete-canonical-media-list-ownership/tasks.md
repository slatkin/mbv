# Complete Canonical Media-List Ownership Campaign Ledger

This planning-only checklist is the authoritative status and ordering record for issue #681. A screen row closes only after its bounded change is accepted, synced, archived, and linked here with automated and human evidence.

## 1. Establish the Campaign Baseline

- [x] 1.1 Accept PR #684 as the Queue and Grouped Music painting/geometry foundation; evidence: PR `https://github.com/slatkin/mbv/pull/684`, merge `f647136a`, archived change `openspec/changes/archive/2026-09-09-repair-canonical-media-list-ownership/`, synced main specs, checked automated gates, and checked human-verification task.
- [x] 1.2 Reconcile and obtain maintainer approval for the visible-screen inventory, preserved screen-specific workspaces, two-column catalog boundary, exclusions, and fixed order; evidence: approved inventory recorded in `proposal.md`, `design.md` D1–D5, and issue comment `https://github.com/slatkin/mbv/issues/681#issuecomment-5597346374`.
- [x] 1.3 Reconcile non-grouped Music views against issue #681 before the Grouped Music follow-on is approved; decision: V1 album-folder states whose levels do not start with `group`, V2 transient group-list roots, V3 non-album intermediate levels, and V4 deeper-than-configured levels are explicitly out of scope for this campaign and for the Grouped Music follow-on, and no row may close on their behalf. Evidence: `src/app/music_actions.rs:9-29,241-289`, `src/app/lib_cursor_actions.rs:57-73`, `src/app/shell_library.rs:71-76`, `src/app/shell_music_workspace.rs:33-48,95-105`, `src/app/shell_browser.rs:185-203`, `src/app/render/components/widgets.rs:515-603`, `src/app/render/tests_music_characterization.rs:111-136`, `crates/mbv-core/src/config_parse.rs:165-177`, and `crates/mbv-core/src/config_tests_library.rs:9-19`; these show the grouped predicate, empty/album-only configuration, absent non-grouped destination, Music browser exclusion, and base-frame no-paint behavior. Issue #681's campaign-correction comment and the archived PR #684 change scope Queue plus Grouped Music only. Default-config non-grouped Music remains a separate product decision/change, not silently complete here.

## 2. Complete Grouped Music

- [ ] 2.1 Create `complete-grouped-music-list-ownership` from this row, citing PR #684 and preserving the Music track table as screen-specific workspace state; verify its proposal names Grouped Music first and scopes album-position ownership, position-free refresh, and one Wide/Normal handoff.
- [ ] 2.2 Accept, sync, and archive the Grouped Music follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence, Normal/Wide human evidence or explicit waiver, and main-spec sync here before checking this row.

## 3. Repair Home

- [ ] 3.1 Create `repair-home-media-list-ownership` after section 2 is accepted; verify it requires only the visible Home list to move, uses one Wide/Normal anchor handoff, forbids ordinary refresh from synchronizing presentations, and preserves Home sections and images.
- [ ] 3.2 Accept, sync, and archive the Home follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence, Normal/Wide human evidence or explicit waiver, and main-spec sync here before checking this row.

## 4. Repair Feeds

- [ ] 4.1 Create `repair-feeds-media-list-ownership` after section 3 is accepted; verify it requires active-list-only movement and retained current-frame interaction while preserving groups, headings, watched filtering, selectors, and feed detail.
- [ ] 4.2 Accept, sync, and archive the Feeds follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence, Normal/Wide human evidence or explicit waiver, and main-spec sync here before checking this row.

## 5. Repair Movies and Related Emby Screens

- [ ] 5.1 Create `repair-emby-media-list-ownership` after section 4 is accepted for Movies, the Emby homevideos feed view, and the Emby podcast channel list; verify the proposal names all three visible screens, preserves their distinct detail/group behavior, and does not conflate either feed-like screen with Feeds or Audiobookshelf Podcasts.
- [ ] 5.2 In the same follow-on, isolate other Emby library tabs that use two-column catalogs from the repaired one-column screen path without converting their grid presentation; verify the design records the actual included library presentations and preserves screen-owned grid interaction with arrangement-owned placement.
- [ ] 5.3 Accept, sync, and archive the Emby follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence for every named screen and the preserved two-column path, Normal/Wide human evidence or explicit waiver, and main-spec sync here before checking this row.

## 6. Repair TV Series

- [ ] 6.1 Create `repair-tv-series-media-list-ownership` only after section 5 is accepted; verify it treats Normal and Wide TV Series as one screen transition, removes duplicate series-list position/interaction ownership, and preserves seasons, episodes, and the season grid as TV-specific workspace state.
- [ ] 6.2 Accept, sync, and archive the TV Series follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence across Normal/Wide and the season/episode workspace, human evidence or explicit waiver, and main-spec sync here before checking this row.

## 7. Repair Audiobookshelf Podcasts

- [ ] 7.1 Create `repair-audiobookshelf-podcast-media-list-ownership` after section 6 is accepted; verify the show list persists across frames and owns current interaction/position while episode filtering, episode selection, images, and playback intents remain Podcast-specific workspace behavior.
- [ ] 7.2 Accept, sync, and archive the Audiobookshelf Podcasts follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence, Normal/Wide human evidence or explicit waiver, and main-spec sync here before checking this row.

## 8. Repair Audiobookshelf Books

- [ ] 8.1 Create `repair-audiobookshelf-book-media-list-ownership` after section 7 is accepted; verify the book list persists across frames and owns current interaction/position while surname buckets, chapters, images, and absolute chapter seeking remain Book-specific workspace behavior.
- [ ] 8.2 Accept, sync, and archive the Audiobookshelf Books follow-on; record its PR, accepted commit, focused component/buffer/live-tick evidence, Normal/Wide human evidence or explicit waiver, and main-spec sync here before checking this row.

## 9. Enforce and Reconcile the Completed Campaign

- [ ] 9.1 Create `finalize-canonical-media-list-ownership` only after sections 2–8 are accepted; verify its proposal is limited to surviving-boundary ratchets, compatibility-path removal or explicit disposition, and documentation/spec reconciliation rather than another screen migration.
- [ ] 9.2 Add narrow structural ratchets derived from accepted screen boundaries and verify they reject concrete regressions without enumerating speculative exemptions.
- [ ] 9.3 Reconcile `CONTEXT.md`, `AGENTS.md`, `docs/architecture/interactive-surface-ledger.md`, current comments, and `openspec/specs/canonical-media-lists/spec.md`; verify whole-screen migration and nested media-list ownership are clearly distinguished and stale Queue/Feeds and Grouped Music claims are removed.
- [ ] 9.4 Accept, sync, and archive the final follow-on; record its PR, accepted commit, architecture gates, full relevant test evidence, final Normal/Wide human verification or explicit waiver, and main-spec sync here.
- [ ] 9.5 Validate that every included screen row has an accepted change, evidence, and synced behavioral truth; mirror final status to issue #681, archive this umbrella, and close the issue only after `openspec validate` and final status both succeed.
