## 1. Preconditions and characterization

- [ ] 1.1 Confirm the two landed predecessors (canonical media-list foundation with its parent/child mouse seam; #640 Home podcast hero-on-left correction), PR #606 feature-branch stacking, and that `migrate-home-feeds-to-canonical-lists` is a sibling slice not a dependency; when implementation is issued, record the feature-branch baseline SHA in the implementation handoff/task evidence (no SHA pinned in this plan); do not edit umbrella artifacts.
- [ ] 1.2 Read current Music, Podcast, and Book render/component callers — including the bespoke `render_book_browser` Wide path that reuses Narrow selected-row-replacement logic — and record source-of-truth precedent from working TV/Movies; document that the Emby podcast channel list, the Emby homevideos feed view, and #623/#634/#637 are out of scope. Read-only: source-reading and manual observation only, no test or fixture changes.
- [ ] 1.3 Characterize grouped Music re-anchor before replacement using source/state inspection and live visual evidence only, with metadata-bearing Wide/Normal and breakpoint-transition evidence for selected target, cursor, scroll, and selected-row offset; do not change or add UI tests before explicit user live visual approval.

## 2. Canonical composition

- [ ] 2.1 Prepare only grouped Music album rows for `WideMediaList`/`InlineMediaBrowser`; preserve group headings/buckets, parent-owned track workspace, images, selectors, and typed intents.
- [ ] 2.2 Compose only Audiobookshelf Podcast show rows with canonical controls; retain the parent-owned selected-show episode workspace, episode/played filter, images, and provider playback authority.
- [ ] 2.3 Compose only Audiobookshelf Book rows with canonical controls; remove the Wide right-rail selected-row replacement (the `render_book_browser` reuse of Narrow logic) so Wide is provider detail workspace on the LEFT plus ordinary fixed-height one-column rows on the RIGHT, with no Inline hero in the right rail; retain parent-owned book detail, chapter/audio-file authority, images, surname buckets, and absolute chapter seek intents.
- [ ] 2.4 Repair the owned Audiobookshelf Podcast and Book non-list arrangement/framing defects: route Podcast Wide through the shared hero-on-left right pane so the Wide pill row matches Narrow, and correct Book Wide left-workspace framing/spacing to the shared policy — no bespoke painter or destination-specific breakpoint. Preserve Wide, Normal/Narrow, and short-height fallback. Covered by the `right-panel-arrangements` delta.
- [ ] 2.5 Preserve stable `ViewportAnchor` target/offset handoff across breakpoint changes and navigation events without shell mirrors or per-frame writeback.
- [ ] 2.6 Resolve pointer gestures delivered by the mounted parent to child-owned canonical hit regions and delegate: child explicit targets precede parent workspace targets, and any duplicate list hit path is removed. This task owns child hit-region resolution/delegation ONLY; mouse subscription and gesture recognition stay with the parent per the landed `restore-mouse-support` spine and are not added or modified here.
- [ ] 2.7 Split `src/app/components/audiobookshelf_podcast.rs` and any other near-limit changed files into cohesive modules before/with wiring; enforce every changed source file ≤800 lines.

## 3. Visual-first evidence and tests

- [ ] 3.1 Perform live visual correction at Wide, Normal/Narrow, and short-height layouts for Music, Podcast, and Book, including selection, scrolling, focus, images enabled/disabled, grouping/filter/bucket state, rail framing, and workspace composition; obtain explicit user live visual approval before changing or adding any UI tests.
- [ ] 3.2 Only after explicit user live visual approval, add/update focused rendered buffer and geometry tests with metadata-bearing fixtures covering one-column geometry, selected/active/played states, images, selectors/buckets, chapter/episode targets, breakpoint fallback, and target/offset anchoring.
- [ ] 3.3 Prove one painter per destination/breakpoint by source trace and execution counter/assertion; prove no destination-sized duplicate list or Wide Book selected-row replacement remains.
- [ ] 3.4 Verify mouse parent/child seam with focused hit-region/typed-intent evidence; do not add a global hit map or second router.

## 4. Verification and acceptance

- [ ] 4.1 Run `rtk make check-code-file-lines` and ensure changed source files are ≤800 lines.
- [ ] 4.2 Run `rtk openspec validate migrate-music-audiobookshelf-to-canonical-lists --strict`.
- [ ] 4.3 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, and relevant `rtk cargo nextest run` suites; fix only slice-caused failures.
- [ ] 4.4 Confirm no Service, provider, playback, daemon, protocol, persistence, dependency, Feeds Service, Emby homevideos feed view, or Emby podcast channel list behavior changed; attach exact command outputs and live user confirmation to the slice PR.
- [ ] 4.5 Keep this slice independently reviewable/reversible and do not mark umbrella tasks complete.
