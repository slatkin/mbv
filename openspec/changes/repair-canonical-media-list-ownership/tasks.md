## 1. Establish the Landed Baseline

- [ ] 1.1 Start from the accepted #674 HEAD, confirm its live-tick painted-owner arbitration tests and one-logical-row wheel tests pass, and record the commit in this change before modifying shared mouse/list paths.
- [ ] 1.2 Update the existing canonical-control tests with one compile-time TuiRealm `Component` bound and one active-only Wide↔Inline `ViewportAnchor` transition case; verify the focused media-list test target fails for the current helper-only or lockstep behavior.

## 2. Make Canonical Controls the Source of Truth

- [ ] 2.1 Implement the plain TuiRealm `Component` view contract for `WideMediaList` using the existing canonical row painter, a closed semantic paint policy, and retained paint/row geometry; verify the control test and representative Wide buffer test pass.
- [ ] 2.2 Implement the same contract for `InlineMediaBrowser`, including replacement admission, ordinary-row fallback, and an exposed provider-detail rectangle; verify the existing replacement/fallback and pointer-resolution tests pass without raw style or callback inputs.
- [ ] 2.3 Add architecture fixtures and rules rejecting production canonical-control construction under `src/app/render/**`; run `ast-grep test` for the new rules and `ast-grep scan`.

## 3. Correct Shared Responsive Destinations

- [ ] 3.1 Change Home so only its currently painted control receives movement, selection, wheel, and point-resolution delegation, with one transition anchor instead of lockstep mutation; verify the existing Home component/buffer tests plus a focused inactive-control regression test pass.
- [ ] 3.2 Change Feeds to the same active-control and one-anchor model while preserving grouped headings, watched filtering, and parent-owned selector pills; verify the existing Feeds component/buffer tests plus a focused inactive-control regression test pass.
- [ ] 3.3 Remove Home/Feeds compatibility row maps or paint writeback made redundant by child-owned geometry, then run their existing Normal/Wide one-painter characterization tests.

## 4. Isolate Browser and TV Ownership

- [ ] 4.1 Introduce an explicitly grid-only Browser position type for the accepted non-hero two-column path, move canonical cursor/scroll/selection reads to the active embedded control, and verify existing generic/Movies/homevideos/podcast Browser tests at Normal and Wide presentations.
- [ ] 4.2 Convert Browser activation, context-menu, persistence, navigation re-anchor, and #674 wheel paths to use control-resolved targets/resting positions without replaying deltas; verify the existing Browser component tests and relevant shell routing tests.
- [ ] 4.3 Remove TV's parent series-cursor shadow and cursor-based mouse fallback, resolving selection and row hits through its persistent series control while retaining season/episode workspace state; verify TV component selection, mouse, buffer, and Wide↔Normal handoff tests.

## 5. Correct Music Ownership

- [ ] 5.1 Move grouped Music album selection and scroll into its persistent Wide and Inline controls, remove `album_cursor`/`album_scroll` lockstep helpers and ordinary-render writeback, and keep track focus/workspace authority in Music; verify existing Music component key/mouse tests.
- [ ] 5.2 Convert Music shell requests, selected-item lookup, persistence, and breakpoint handoff to control-resolved stable targets/resting positions; verify existing shell Music workspace and Wide↔Normal re-anchor tests.
- [ ] 5.3 Route both Music album presentations through the embedded controls' component views and verify existing narrow/wide buffer and one-painter characterization tests remain unchanged.

## 6. Correct Audiobookshelf Ownership

- [ ] 6.1 Add a persistent Wide show-list control to `AudiobookshelfPodcastComponent`, project show rows during content updates, and remove render-time control construction plus parent show cursor/scroll authority; verify Podcast component key/mouse and Normal/Wide buffer tests.
- [ ] 6.2 Convert Podcast selection, show-fetch requests, persistence, and breakpoint handoff to control-resolved stable targets while retaining episode filter/selection in the provider workspace; verify existing shell Podcast and breakpoint tests.
- [ ] 6.3 Add a persistent Wide book-list control to `AudiobookshelfBookComponent`, project book rows during content updates, and remove render-time control construction plus parent book cursor/browser-offset authority; verify Book component key/mouse and Normal/Wide buffer tests.
- [ ] 6.4 Convert Book selection, bucket/book requests, persistence, and breakpoint handoff to control-resolved stable targets while retaining bucket and chapter workspace state; verify existing shell Book and breakpoint tests.

## 7. Finish Paint and Geometry Cleanup

- [ ] 7.1 Route Queue's fixed rows through its embedded control's component view and delete only the parent compatibility geometry/state superseded by the control, preserving Queue scope and drag ownership; verify existing Queue component, drag, and one-painter tests and confirm `QueueHitRegion` remains absent.
- [ ] 7.2 Delete canonical compatibility row maps, render-function constructors, parent mirrors, ordinary-render selection/scroll writeback, and any remaining per-surface canonical row-hit `*HitRegion` enums after their last consumers are gone; verify targeted source searches are empty and `cargo check -p mbv` passes.
- [ ] 7.3 Add narrow structural ratchets for the known mirror/writeback forms removed by this change, with positive and negative fixtures, then run `ast-grep test` and `ast-grep scan`.

## 8. Reconcile Documentation and Acceptance Evidence

- [ ] 8.1 Update `CONTEXT.md`, canonical-control comments, the interactive-surface ledger, and related architecture documentation to describe actual embedded component views, child-owned point resolution, active-only handoff, and the grid carve-out; verify terminology matches `CONTEXT.md` and no stale “no hit-resolution API” or per-render-control claim remains.
- [ ] 8.2 Run the focused media-list, Home, Feeds, Browser, TV, Music, Audiobookshelf, Queue, render-characterization, and real-`Application::tick()` mouse tests; confirm #674 painted-owner arbitration, one-row wheel, throttle, routing, and Playlists/Queue regression evidence remains passing.
- [ ] 8.3 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; fix all failures before acceptance.
- [ ] 8.4 Have a human verify representative Normal and Wide Home, Feeds, Browser, TV, Music, Audiobookshelf, and Queue behavior, plus focused-sidebar wheel arbitration; record the result or an explicit human waiver before acceptance.
- [ ] 8.5 Sync both delta specs into the main specs and archive the change only after implementation, automated gates, review, and human verification are accepted; verify final OpenSpec validation succeeds.
