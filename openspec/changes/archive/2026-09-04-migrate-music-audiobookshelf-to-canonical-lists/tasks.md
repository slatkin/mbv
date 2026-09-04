## 1. Preconditions and characterization

- [x] 1.1 Confirm the two predecessors (canonical media-list foundation — an accepted prerequisite slice that must merge first; the landed #640 Home podcast hero-on-left correction), PR #606 feature-branch stacking, and that `migrate-home-feeds-to-canonical-lists` is a sibling slice not a dependency; when implementation is issued, record the feature-branch baseline SHA in the implementation handoff/task evidence (no SHA pinned in this plan); do not edit umbrella artifacts.
- [x] 1.2 Read current Music, Podcast, and Book render/component callers — including the bespoke `render_book_browser` Wide path that reuses Narrow selected-row-replacement logic — and record source-of-truth precedent from working TV/Movies; document that the Emby podcast channel list, the Emby homevideos feed view, and #623/#634/#637 are out of scope.
- [x] 1.3 Record grouped Music re-anchor behavior before replacement with metadata-bearing Wide/Normal and breakpoint-transition evidence for selected target, cursor, scroll, and selected-row offset.

## 2. Canonical composition

- [x] 2.1 Prepare only grouped Music album rows for `WideMediaList`/`InlineMediaBrowser`; preserve group headings/buckets, parent-owned track workspace, images, selectors, and typed intents.
- [x] 2.2 Compose only Audiobookshelf Podcast show rows with canonical controls; retain the parent-owned selected-show episode workspace, episode/played filter, images, and provider playback authority.
- [x] 2.3 Compose only Audiobookshelf Book rows with canonical controls; remove the Wide right-rail selected-row replacement (the `render_book_browser` reuse of Narrow logic) so Wide is provider detail workspace on the LEFT plus ordinary fixed-height one-column rows on the RIGHT, with no Inline hero in the right rail; retain parent-owned book detail, chapter/audio-file authority, images, surname buckets, and absolute chapter seek intents.
- [x] 2.4 Repair the owned Audiobookshelf Podcast and Book non-list arrangement/framing defects: route Podcast Wide through the shared hero-on-left right pane so the Wide pill row matches Narrow, and correct Book Wide left-workspace framing/spacing to the shared policy — no bespoke painter or destination-specific breakpoint. Preserve Wide, Normal/Narrow, and short-height fallback. Covered by the `right-panel-arrangements` delta.
- [x] 2.5 Preserve stable `ViewportAnchor` target/offset handoff across breakpoint changes and navigation events without shell mirrors or per-frame writeback.
- (2.6 removed: mouse deferred to #638. `restore-mouse-support` lands after every canonical slice and owns the parent/child point-resolution seam; this slice adds no mouse wiring and leaves existing bespoke `*HitRegion` paths untouched.)
- [x] 2.7 Split `src/app/components/audiobookshelf_podcast.rs` and any other near-limit changed files into cohesive modules before/with wiring; enforce every changed source file ≤800 lines. (No split needed — every changed source file landed ≤692 lines; enforcement clause satisfied.)

## 3. Implementation evidence

- [x] 3.1 Add/update the smallest focused stateful, rendered-buffer, and geometry tests with metadata-bearing fixtures covering one-column geometry, selected/active/played states, images, selectors/buckets, chapter/episode targets, breakpoint fallback, ordinary-refresh target retention, and target/offset anchoring.
- [x] 3.2 Prove one painter per destination/breakpoint by source trace and execution counter/assertion; prove no destination-sized duplicate list or Wide Book selected-row replacement remains.
- (3.3 removed: mouse deferred to #638. This slice adds no mouse parent/child seam; `restore-mouse-support` owns it and lands last.)

## 4. Verification, review, and acceptance

- [x] 4.1 Run `make check-code-file-lines` and ensure changed source files are ≤800 lines. (All slice-changed files ≤692; the sole failure is pre-existing `src/app/shell_home.rs` at 801, untouched by this slice.)
- [x] 4.2 Run `openspec validate migrate-music-audiobookshelf-to-canonical-lists --strict`. (valid)
- [x] 4.3 Run `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, and relevant `cargo nextest run` suites; fix only slice-caused failures. (fmt clean; check clean; nextest 1288 passed / 0 failed; clippy only pre-existing warnings.)
- [x] 4.4 Review the complete slice, then perform live Wide, Normal/Narrow, and short-height acceptance for Music, Podcast, and Book. (Whole-slice reviewer PASS. Live: user accepted Music Wide+Narrow 2026-09-03; user directed "pass visual verification, no need to pause" 2026-09-04 for Podcast + Book.)
- [x] 4.5 Confirm no Service, provider, playback, daemon, protocol, persistence, dependency, Feeds Service, Emby homevideos feed view, or Emby podcast channel list behavior changed. (Whole-slice reviewer confirmed scope containment; diff `819dbd0c..HEAD` touches only `src/app/render/`, `src/app/components/`, and the change's own openspec dir.) Do not mark umbrella tasks complete.
