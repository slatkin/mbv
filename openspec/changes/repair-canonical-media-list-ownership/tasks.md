# Repair Canonical Media-List Ownership

## 1. Establish the Landed Baseline and Control Contract

- [ ] 1.1 Start from landed PR #683 commit `6b58a608`; confirm its live-tick painted-owner arbitration, one-logical-row wheel, throttle, and routing tests pass before altering shared mouse/list paths, and record that baseline in this change.
- [ ] 1.2 Add compile-time plain TuiRealm `Component` bounds for both canonical controls and focused control tests for one configured child view with retained Wide row/selected geometry and Inline admitted-detail geometry; verify the current helper/second-painter behavior would fail.
- [ ] 1.3 Implement the closed per-frame canonical paint policy (outer paint rectangle, named content inset, focus/selection, throbber, and Inline detail request) and retained paint-result getters; verify every migrated parent calls its child view once and consumes no recomputed geometry.
- [ ] 1.4 Add architecture fixtures and rules rejecting production canonical-control construction under `src/app/render/**`, except the explicit provider-workspace seams; run `ast-grep test` for the new rules and `ast-grep scan`.

## 2. Make Canonical Controls the Source of Truth

- [ ] 2.1 Route `WideMediaList` through its component view and retained result using the existing canonical row painter, then verify focused control and representative Wide buffer tests pass.
- [ ] 2.2 Route `InlineMediaBrowser` through its component view and retained result, including replacement admission and ordinary-row fallback; verify existing replacement, fallback, and pointer-resolution tests pass without raw style or callback inputs.
- [ ] 2.3 Delete only compatibility `left_item_rows`/`left_row_map` output whose canonical consumers have moved to retained child results; verify a targeted source search finds no remaining canonical consumer and `cargo check -p mbv` passes.

## 3. Correct Shared Responsive Destinations

- [ ] 3.1 Change Home so only its painted control receives movement, selection, wheel, and point delegation, with one transition anchor instead of lockstep mutation; use `QueueItemContentId` as the row target and verify existing Home tests plus a mixed-Service native-id collision regression.
- [ ] 3.2 Change Feeds to the same active-control and one-anchor model while preserving grouped headings, watched filtering, and parent-owned selector pills; verify existing Feeds component/buffer tests plus an inactive-control regression test.
- [ ] 3.3 Remove Home/Feeds compatibility row maps or paint writeback made redundant by child-owned geometry; verify their Normal/Wide one-painter characterization tests.

## 4. Isolate Browser and TV Ownership

- [ ] 4.1 Introduce `BrowserGridState` and `BrowserGridGeometry` for the accepted non-hero two-column path, including grid row maps, and remove their use from canonical paths; verify existing generic/Movies/homevideos/podcast Browser tests at Normal and Wide presentations.
- [ ] 4.2 Replace Browser canonical index targets with stable Emby identity and map the target back to current content at activation, context-menu, persistence, navigation, and wheel boundaries; verify a refresh/reorder and Wide↔Normal target-plus-offset regression test and relevant shell routing tests.
- [ ] 4.3 Make TV series content position-free, move series selection/scroll to its persistent controls, and remove parent cursor shadow and cursor-based mouse fallback; retain season/episode workspace state and verify existing TV selection, mouse, buffer, and breakpoint-handoff tests.
- [ ] 4.4 Declare TV's episode pane a parent-owned provider workspace exemption from destination-rail ratchets; preserve its typed episode activation and verify its existing episode selection and buffer tests.

## 5. Correct Music and Audiobookshelf Rail Ownership

- [ ] 5.1 Introduce position-free Music content plus a separately named discrete resting-position/re-anchor input; move album selection and scroll to persistent Wide/Inline controls, remove ordinary-render writeback, and verify existing Music key/mouse and Wide↔Normal tests.
- [ ] 5.2 Introduce position-free Audiobookshelf Podcast and Book browse-content projections plus separately named re-anchor inputs; project show/book rows during content updates and remove render-time rail construction, seeded selection, and parent list offsets; verify Podcast/Book component key/mouse and Normal/Wide buffer tests.
- [ ] 5.3 Add focused source or component ratchets proving ordinary Music, TV, and Audiobookshelf content pushes carry no cursor/scroll; verify each control preserves or clamps local state until an explicit re-anchor.
- [ ] 5.4 Declare Audiobookshelf Podcast episode and Book chapter panes parent-owned provider workspace exemptions from destination-rail ratchets; preserve typed episode intent and chapter absolute-seek target ownership and verify existing workspace tests.

## 6. Finish Queue, Ratchets, and Documentation

- [ ] 6.1 Route Queue fixed rows through its embedded control component view and delete only superseded parent compatibility geometry/state, preserving Queue scope and drag ownership; verify existing Queue component, drag, and one-painter tests and confirm `QueueHitRegion` remains absent.
- [ ] 6.2 Add narrow structural ratchets for parent cursor/scroll mirrors, ordinary-render writeback, position-bearing content projections, and isolated grid access, with positive and negative fixtures; run `ast-grep test` and `ast-grep scan`.
- [ ] 6.3 Update `CONTEXT.md`, canonical-control comments, the interactive-surface ledger, and architecture documentation to describe actual embedded component views, retained paint results, stable targets, active-only handoff, grid isolation, and provider-workspace exemptions; verify terminology matches `CONTEXT.md`.

## 7. Verify and Accept the Slice

- [ ] 7.1 Run focused media-list, Home, Feeds, Browser, TV, Music, Audiobookshelf, Queue, render-characterization, and real-`Application::tick()` mouse tests; confirm PR #683 painted-owner arbitration, one-row wheel, throttle, routing, and Playlists/Queue regression evidence remains passing.
- [ ] 7.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; fix all failures before acceptance.
- [ ] 7.3 Have a human verify representative Normal and Wide Home, Feeds, Browser, TV, Music, Audiobookshelf, and Queue behavior, plus focused-sidebar wheel arbitration; record the result or an explicit human waiver before acceptance.
- [ ] 7.4 Sync both delta specs into the main specs and archive the change only after implementation, automated gates, review, and human verification are accepted; verify final OpenSpec validation succeeds.
