# Complete Shared Media-List Ownership Design

## Context

See `proposal.md` for motivation and `specs/canonical-media-lists/spec.md` for the corrected contract. PR #686 introduced private `ListCore` state shared by `WideMediaList` and `InlineMediaBrowser`, but each presentation still owns a separate core, destination parents still interpret row-local input, the non-hero two-column Browser path bypasses the core, and several provider workspaces mirror or reseed row selection. The mounted destination must remain the sole TuiRealm event boundary, and the parent must continue to own provider content, chrome, pane focus, effects, and typed intent translation.

The archived `finish-canonical-media-list-ownership` umbrella records the work that produced this baseline. It is immutable history; this additive change corrects its completion claim for PR #686.

## Goals / Non-Goals

**Goals:**

- Make one persistent shared owner authoritative for every in-scope logical media-row flow, independent of its current presentation.
- Preserve existing Wide, Inline, and non-hero two-column behavior while making those presentations consume the same owner.
- Give destinations one stable input-delegation seam and only provider-neutral outcomes to translate.
- Delete all in-scope destination row-state mirrors, render-time reseeding, compatibility row maps, and shell target re-resolution.
- Leave a mechanical proof of the one-place extension property.

**Non-Goals:**

- Implement checked membership, range selection, or any multi-select behavior.
- Move sections, groups, filters, surname buckets, seasons, Queue scope, pane focus, detail workspaces, images, effects, persistence, or Service data into the media-list owner.
- Convert Inline Search, Search sidebar, Playlists, selection modals, or unrelated menu/form/device lists.
- Change the visual arrangement, breakpoint policy, key precedence, mouse gesture timing, protocols, or persistence formats.

## Decisions

### D1. One `MediaList<Target>` owns one logical row flow

Replace presentation-owned copies of `ListCore` with one persistent embedded `MediaList<Target>` per logical media-row flow. It owns rows, selectable indexing, cursor, scroll, selected target, future row-local state, delegated behavior, and the current presentation's retained geometry. Before `Component::view`, the parent configures one closed presentation value: Wide, Inline with desired detail admission, or Grid with arrangement-provided columns and row flow.

A logical list changes presentation by reconfiguring the same instance. Presentation changes preserve the owner automatically and retain only the outgoing selected-row viewport offset needed to place the receiving view. `ViewportAnchor` remains for explicit discrete navigation/restoration between genuinely different owners or destinations, not for synchronizing responsive copies.

This name is a design placeholder until implementation checks it against `CONTEXT.md`; if adopted as a public domain term, add it there and distinguish it from `WideMediaList` and `InlineMediaBrowser` presentation names.

Alternative: keep independent Wide and Inline controls and extend `ViewportAnchor` whenever new local state appears. Rejected because every new list-local feature would require new transfer logic in each destination, which is the failed acceptance condition.

Alternative: expose a separate state object borrowed by persistent Wide and Inline wrappers. Rejected because it creates multiple long-lived objects claiming one logical list and complicates retained-result lifecycle; one configured component is the smaller ownership model.

### D2. Wide, Inline, and Grid are closed presentations

The shared component selects one of three closed presentations:

- Wide: fixed one-column rows and scrollbar.
- Inline: one-column rows with selected-row replacement admission/fallback.
- Grid: the existing non-hero two-column catalog placement and scrolling policy.

Each presentation owns only placement, paint policy, viewport calculation, and current-frame geometry. All use the same provider-neutral `MediaListRow<Target>` and shared row painter. Grid preserves the current arrangement's column count, cell width, and traversal order; it does not make non-hero catalogs visually resemble Wide or Inline.

Alternative: force non-hero catalogs through `WideMediaList`. Rejected because the accepted two-column presentation is not the defect.

Alternative: leave Grid outside the shared owner. Rejected because Browser would remain a destination-local list implementation and fail the one-place criterion.

### D3. Parent routes precedence; shared list handles row-local input

The destination `AppComponent` remains the only mounted/focused/subscribed event boundary. It handles overlays, Inline Search, chrome, pane changes, and parent gestures first, then delegates every remaining eligible row-local key or normalized pointer gesture to its active `MediaList`.

The shared result is a closed provider-neutral outcome:

- unhandled;
- consumed locally;
- selected stable target changed;
- external row intent with stable target (activate or context).

Local behavior uses `consumed`, so adding another local transition does not add a destination-visible variant. Destinations translate external row intents to their existing provider-specific `Msg`s. The central Keyboard Router remains the only global precedence authority; delegation is leaf-local handling after `FallThrough`, not another router.

Mouse gesture timing remains parent-owned. The parent passes a normalized single-click, double-click, context-click, or wheel action; the shared component resolves its own retained row geometry and mutates state before returning its result.

Alternative: mount every embedded list so it receives TuiRealm events directly. Rejected by the embedded-component and focus/subscription contracts.

Alternative: retain per-destination movement methods that call shared mutators. Rejected because a future local chord would still require editing every destination.

### D4. Stable opaque targets cross the destination boundary

Change source-of-truth row target types before their callers. Targets represent stable occurrences, not display positions:

- Browser rows use content identity rather than indexes into `items`.
- Home rows use a stable destination-local identity sufficient to resolve the owning section and `QueueItem` after refresh.
- Podcast episode targets include show identity where episode identity alone is insufficient.
- Book chapter/audio-part targets include book identity plus the stable row discriminator.
- Queue continues to use `QueueSlotId`; existing stable album, track, series, show, book, FeedEntry, and episode identities are retained or strengthened where necessary.

Typed destination requests carry the resolved target. The shell looks up shell-owned content by that target and never queries a component cursor or reinterprets a numeric display position.

Alternative: keep numeric indexes because lists already clamp them. Rejected because indexes do not preserve row-local state across reorder and force shell re-resolution.

### D5. Provider workspace focus is separate from row selection

Replace `track_cursor: Option<usize>`, `episode_selection: Option<usize>`, and `chapter_selection: Option<usize>` with explicit parent-owned pane/focus state. The corresponding `MediaList` remains authoritative for selected row and scroll whether or not its workspace currently has focus. Filter, season, bucket, or selected parent changes project new rows into the owner and explicitly select the required stable target only at that discrete boundary.

TV already follows this shape: `Pane` and `season_cursor` are legitimate parent state while its episode `WideMediaList` owns episode selection. TV therefore needs delegation and the unified owner API, not another episode cursor migration.

Alternative: treat `Option<usize>` as both focus and selection. Rejected because it creates a parallel cursor and makes focus changes destroy list state.

### D6. Content changes precede view; view publishes retained facts once

Destinations project rows in content/update methods and after parent-owned filter/season/bucket changes. Render Components paint frame, chrome, detail, and arrangement-owned surfaces, but do not call `set_content`, `select_index`, or compatibility list painters. The destination view configures the active presentation and calls its `Component::view` once.

The shared component invalidates prior retained facts on content/geometry configuration and at view start, then publishes only the completed current frame's claim, content, selected-row/detail, and point-resolution facts. Remove public caller-area `resolve_point`/`claims_point`, mutable `RowGeometry`, `left_row_map`, chapter/episode row maps, and destination fallback arithmetic after each caller migrates.

Alternative: preserve compatibility outputs for tests and legacy readers. Rejected because they permit a second row authority to return after this change.

### D7. Mechanical enforcement defines completion

Completion requires all of the following:

1. Direct cursor, scroll, movement, point-selection, and compatibility paint APIs are private to `media_list`.
2. An architecture inventory identifies every in-scope logical row flow and proves it stores the shared owner.
3. ast-grep rules reject destination calls to forbidden row mutators/compatibility painters and destination-owned row-map reconstruction.
4. Existing integration coverage is consolidated to one representative real `Application::tick()` path for each presentation class (Wide, Inline, Grid) and one provider workspace list; other tests remain only where they protect distinct destination behavior.
5. A disposable acceptance probe adds a test-only local state transition and decoration solely in the shared subsystem, exercises all four paths through existing destination routing, and is reverted before commit. Any required destination production edit fails acceptance.

Alternative: rely on review and per-destination tests. Rejected because PR #686 passed those checks while missing the motivating outcome.

## Risks / Trade-offs

- **[A broad refactor changes visible behavior]** → Characterize each presentation before migration, preserve existing arrangements and key semantics, and compare representative buffers rather than absolute geometry.
- **[One input API becomes a second keyboard router]** → Keep global precedence exclusively in `router.rs`/`key_policy.rs`; the shared API handles only row-local input after the parent has selected the active row flow.
- **[Stable-target conversion changes effect semantics]** → Change target types first, then component requests, then shell lookup; retain explicit tests for play, enqueue, delete, watched-toggle, context, and nested activation boundaries.
- **[Workspace focus is confused with selection]** → Model focus as a separate closed parent state and derive row selection only from the shared owner.
- **[Architecture scans become name-based and brittle]** → Match forbidden AST shapes/import paths and maintain one explicit in-scope inventory rather than scanning arbitrary `cursor` names used by legitimate chrome.
- **[The change expands into every list in the application]** → Limit the inventory to the proposal's primary destination and provider-workspace media rows; keep Inline Search and unrelated sidebars/modals outside.

## Migration Plan

1. Add characterization where current Wide, Inline, Grid, or workspace behavior lacks a durable check; improve existing tests instead of duplicating them.
2. Introduce the single-owner component, closed presentations, stable outcomes, and Grid presentation in the shared subsystem.
3. Convert target types and typed requests at their source-of-truth boundaries.
4. Migrate Browser/Grid and remove its parent cursor, scroll, row maps, and fallback geometry.
5. Migrate responsive top-level flows to one owner per logical list: Home, Feeds, Movies/homevideos, grouped albums, Podcast shows, and Book titles.
6. Migrate provider workspace rows: Music tracks, TV episodes, Podcast episodes, and Book chapter/audio parts; separate pane focus from selection and remove render-time reseeding.
7. Convert Queue to the common delegation seam and remove remaining public compatibility APIs after the last caller disappears.
8. Add architecture enforcement, run the disposable acceptance probe, then run focused and project gates plus live Wide/Normal/Grid review.
9. Update PR #686 and issue #681 to pass only after reviewer sign-off confirms the one-place criterion.

Rollback is a normal commit revert of this additive change. No persisted or wire data changes require migration.
