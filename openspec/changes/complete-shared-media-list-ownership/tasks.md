# Complete Shared Media-List Ownership Tasks

## 1. Lock the Baseline and Shared Vocabulary

- [x] 1.1 Inventory the exact production owners and existing tests for every in-scope row flow from the delta spec; record the source/test map in `evidence/ownership-baseline.md` and verify no listed flow or breakpoint is omitted.
- [x] 1.2 Add or strengthen only the minimum existing characterization needed to preserve current Wide, Inline, two-column Grid, and provider-workspace behavior; verify the focused tests fail when their respective painter, target, movement, or retained-hit path is intentionally disconnected and pass unchanged at baseline.
- [x] 1.3 Add the accepted shared-owner and Grid terminology to `CONTEXT.md` and update `AGENTS.md` plus `.agents/skills/mbv-frontend/SKILL.md` to state the corrected boundary; verify no new term collides with an existing term or an Avoid entry.

## 2. Establish the Single Shared Owner

- [x] 2.1 Replace presentation-owned `ListCore` copies with one persistent shared media-list owner per logical row flow, preserving provider-neutral rows, stable-target refresh, cursor/scroll clamping, and retained-result invalidation; verify focused shared-control tests pass.
- [x] 2.2 Convert Wide and Inline into closed presentations over that same owner, retaining fixed-row and selected-row-replacement behavior without presentation-to-presentation state copying; verify existing Wide/Inline buffer and viewport tests pass and a responsive test observes one owner before and after the presentation change.
- [x] 2.3 Add the Grid presentation over the shared owner using the existing non-hero two-column arrangement, traversal, scrollbar, and current-frame cell geometry; verify one relational/buffer characterization and one point-resolution test preserve the current two-column behavior.
- [x] 2.4 Add the provider-neutral row-local input delegation contract and closed outcomes, keeping global precedence in the Keyboard Router and gesture timing in the parent; verify shared tests cover unhandled, consumed, selected-target-changed, activation, context, movement, paging, edge selection, click, and wheel behavior without provider data.

## 3. Convert Stable Target Boundaries

- [x] 3.1 Change Browser row targets from item positions to stable content identity at the source-of-truth projection, then update component and shell callers; verify reorder/refresh preserves the same target and Browser effects resolve the intended item.
- [x] 3.2 Introduce stable Home row targets and change play, enqueue, delete, watched-toggle, and context requests to carry the component-resolved target; remove shell cursor queries and flat-index re-resolution, then verify the existing Home effect-boundary tests act on the requested row after reorder.
- [x] 3.3 Strengthen nested workspace targets where identity is parent-relative: show-qualified Podcast episodes and book-qualified chapter/audio-part rows; update typed intents before callers and verify same-local-id siblings resolve independently.

## 4. Migrate Browser and Home

- [ ] 4.1 Move generic Emby non-hero two-column catalogs onto the shared owner and Grid presentation; remove Browser's parent `cursor`/`scroll`, legacy item-row reconstruction, `left_row_map`, and fallback cell hit arithmetic, then verify generic catalog tick navigation and mouse tests use retained Grid geometry.
- [ ] 4.2 Move Movies and the Emby homevideos feed view onto one owner whose presentation changes between Wide and Inline; remove responsive owner-to-owner anchor transfer while preserving selected-row viewport offset, and verify Wide/Normal tick tests plus stable refresh pass.
- [ ] 4.3 Move Home's active section rows onto one owner with Wide/Inline presentations and the common delegation seam; preserve section chrome and semantic section restoration, then verify existing Wide/Normal tick, refresh, pointer, and effect tests pass without shell cursor reads.

## 5. Migrate Feeds and Queue

- [ ] 5.1 Move Feeds entries onto one owner with Wide/Inline presentations and common delegation; preserve subscription group and watched-filter chrome, Heading/Spacer exclusion, and row formatting, then verify Wide/Normal navigation, refresh, wheel, click, and one-painter tests pass.
- [ ] 5.2 Convert Queue rows to the common owner and delegation outcomes while preserving `QueueSlotId`, scope pills, drag gestures, queue mutations, and fixed-row presentation in every Panel mode; verify Queue component, drag, and tick tests pass.

## 6. Migrate Grouped Music and TV Workspaces

- [ ] 6.1 Move Grouped Music album rows onto one owner across Wide/Inline presentations and common delegation while preserving group pills, Inline Search, images, and album effects; verify album target/order, responsive, keyboard, mouse, and buffer tests pass.
- [ ] 6.2 Replace Music `track_cursor` with explicit track-pane focus and make the track owner authoritative for selection, scrolling, delegated input, painting, and retained hits; verify track activation/context behavior and Wide workspace tests pass with no cursor synchronization.
- [ ] 6.3 Convert TV series and episode flows to the common owner/delegation API while preserving `Pane`, season chrome, episode content refresh, Inline Search, and Normal routing through Browser; verify Wide series/episode and Normal TV tick tests pass with no destination row mutator calls.

## 7. Migrate Audiobookshelf Workspaces

- [ ] 7.1 Move Audiobookshelf Podcast show rows onto one owner across Wide/Inline presentations and common delegation, preserving title buckets, detail loading, images, and typed intents; verify Wide/Normal refresh, breakpoint, keyboard, mouse, and one-painter tests pass.
- [ ] 7.2 Replace Podcast `episode_selection` with explicit episode-pane focus, project filtered episode rows before view, and make the episode owner authoritative; remove render-time `set_content`/`select_index` and episode row maps, then verify filter transitions and episode activation at Wide/Normal use retained shared geometry.
- [ ] 7.3 Move Audiobookshelf Book rows onto one owner across Wide/Inline presentations and common delegation, preserving surname buckets, detail loading, images, and typed intents; verify Wide/Normal refresh, breakpoint, keyboard, mouse, and one-painter tests pass.
- [ ] 7.4 Replace Book `chapter_selection` with explicit chapter-pane focus, project chapter/audio-part rows before view, and make the chapter owner authoritative; remove render-time `set_content`/`select_index` and chapter row maps, then verify chapter focus and absolute seek resolve the stable book-qualified target at Wide/Normal.

## 8. Delete Compatibility and Enforce the Boundary

- [ ] 8.1 After the final caller migrates, make direct cursor/scroll/movement/point-selection mutators private to the shared subsystem and delete caller-area `resolve_point`/`claims_point`, mutable `RowGeometry` exports, compatibility painters, parent row maps, and fallback geometry; verify production search finds no in-scope caller and focused hit-lifecycle tests pass.
- [ ] 8.2 Add narrowly scoped ast-grep rules and fixtures rejecting destination calls to shared row mutators, compatibility painters, and row-map reconstruction while allowing legitimate parent chrome state; verify `ast-grep test` and `ast-grep scan` pass.
- [ ] 8.3 Add one maintained architecture inventory check proving every in-scope logical row flow stores the shared owner and no destination stores authoritative media-row cursor/scroll/selection fields; verify the check fails against representative forbidden fixtures and passes on production.
- [ ] 8.4 Consolidate redundant destination ownership tests only where the shared and integration tests supersede them; verify each retained test protects a distinct provider behavior or real TuiRealm composition boundary and the focused suite remains green.

## 9. Prove the Original Outcome and Close PR #686

- [ ] 9.1 Run the disposable acceptance probe: add one test-only row-local transition and decoration solely in the shared subsystem, exercise it through existing Wide, Inline, Grid, and provider-workspace destination routing, record the commands/results in `evidence/one-place-acceptance.md`, and revert the probe; fail this task if any destination production edit is required.
- [ ] 9.2 Perform live Wide, Normal, Grid, and provider-workspace review for one-painter ownership, preserved visuals, current-frame hit geometry, selector chrome, and responsive viewport offset; record results and fix any defect before completion.
- [ ] 9.3 Run `cargo fmt --all -- --check`, `cargo check -p mbv`, the focused tests identified in task 1.1, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep test`, and `ast-grep scan`; record unrelated failures without marking the task complete.
- [ ] 9.4 Run the pre-push-only `make check-code-file-lines`, validate `complete-shared-media-list-ownership` strictly, sync its delta into the current canonical-media-lists spec, and verify the change artifacts, current spec, PR #686 body, and issue #681 all state that the one-place criterion is the acceptance bar.
- [ ] 9.5 Obtain final reviewer/advisor sign-off by re-reading the original motivating outcome and inspecting production code; mark PR #686 passing only if the reviewer confirms that a purely list-local behavior and decoration require no destination production edits.
