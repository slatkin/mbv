## 1. Preconditions and characterization

- [ ] 1.1 Confirm accepted foundation and Home/Feeds prerequisites, clean accepted base 25c6e77, PR #606 feature-branch stacking, and #640 supersession; do not edit umbrella artifacts.
- [ ] 1.2 Read current Music, Podcast, and Book render/component callers and record source-of-truth precedent from working TV/Movies; document no #623/#634/#637 scope.
- [ ] 1.3 Characterize grouped Music re-anchor before replacement with metadata-bearing Wide/Normal and breakpoint-transition evidence: selected target, cursor, scroll, and selected-row offset.

## 2. Canonical composition

- [ ] 2.1 Prepare grouped Music album rows for `WideMediaList`/`InlineMediaBrowser`; preserve group headings/buckets, track workspace, images, selectors, and typed intents.
- [ ] 2.2 Compose Audiobookshelf Podcast show rows with canonical controls; retain selected-show episode workspace, episode/played filter, images, and provider playback authority.
- [ ] 2.3 Compose Audiobookshelf Book rows with canonical controls and remove Wide selected-row replacement; retain book detail, chapter/audio-file authority, images, surname buckets, and absolute chapter seek intents.
- [ ] 2.4 Repair Podcast and Book non-list arrangement/framing defects required for composition using shared placement/rail policies; preserve Wide, Normal/Narrow, and short-height fallback.
- [ ] 2.5 Preserve stable `ViewportAnchor` target/offset handoff across breakpoint changes and navigation events without shell mirrors or per-frame writeback.
- [ ] 2.6 Wire parent-owned mouse gesture subscription to child-owned canonical hit regions; ensure child explicit targets precede workspace targets and no duplicate list hit path exists.
- [ ] 2.7 Split `src/app/components/audiobookshelf_podcast.rs` and any other near-limit changed files into cohesive modules before/with wiring; enforce every changed source file ≤800 lines.

## 3. Visual-first evidence and tests

- [ ] 3.1 Perform live visual correction at Wide, Normal/Narrow, and short-height layouts for Music, Podcast, and Book, including selection, scrolling, focus, images enabled/disabled, grouping/filter/bucket state, rail framing, and workspace composition; obtain explicit user confirmation before UI test edits.
- [ ] 3.2 After confirmation, add/update focused rendered buffer and geometry tests with metadata-bearing fixtures covering one-column geometry, selected/active/played states, images, selectors/buckets, chapter/episode targets, breakpoint fallback, and target/offset anchoring.
- [ ] 3.3 Prove one painter per destination/breakpoint by source trace and execution counter/assertion; prove no destination-sized duplicate list or Wide Book selected-row replacement remains.
- [ ] 3.4 Verify mouse parent/child seam with focused hit-region/typed-intent evidence; do not add a global hit map or second router.

## 4. Verification and acceptance

- [ ] 4.1 Run `rtk make check-code-file-lines` and ensure changed source files are ≤800 lines.
- [ ] 4.2 Run `rtk openspec validate migrate-music-audiobookshelf-to-canonical-lists --strict`.
- [ ] 4.3 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and relevant `rtk cargo nextest run` suites; fix only slice-caused failures.
- [ ] 4.4 Confirm no Service, provider, playback, daemon, protocol, persistence, dependency, Feeds Service, or Emby homevideos feed-view behavior changed; attach exact command outputs and live user confirmation to the slice PR.
- [ ] 4.5 Keep this slice independently reviewable/reversible and do not mark umbrella tasks complete.
