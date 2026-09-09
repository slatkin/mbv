# Fix Book Media-List Ownership Design

## Context

See `proposal.md` for motivation. `AudiobookshelfBookComponent` already keeps persistent `InlineMediaBrowser<String>` and chapter-workspace `WideMediaList<String>` controls, but its Wide book rail is a fresh `WideMediaList<String>` built during every render. Live book position instead survives in `AudiobookshelfBookBrowseState`, `browser_offset`, and paint writeback. `AudiobookshelfBookGeometry` republishes book and chapter row maps; clicks and wheel input search those maps, while responsive handoff reseeds the content struct and renderer.

Queue is the concrete ownership precedent (umbrella D4), and the accepted Podcast repair supplies the closest active-control, retained-hit, placeholder-invalidation, shell-tick, and responsive-handoff pattern. Umbrella D8 requires full Emby-reference convergence. The current `audiobookshelf-book-browsing` specification already requires the selected book’s hero and chapter detail to replace the active row in the inline presentation, so this change implements that requirement without a spec delta.

## Goals / Non-Goals

**Goals:**

- Give persistent Wide and Inline book controls exclusive live cursor, scroll, viewport, and current-frame book-row geometry authority.
- Keep Book content, workspace state, shell effects, and persistence separate from presentation position, with movement resolved once by the component.
- Implement the existing narrow chapter-detail presentation and converge cover-loading/placeholder behavior through accepted shared vocabulary.
- Prove the boundary through focused and real shell-tick tests at Wide and Normal/Narrow.

**Non-Goals:**

- Change surname-bucket data or grouping, chapter selection/list workspace mechanics, chapter-focus no-op storage, or absolute chapter-seek authority.
- Change image fetching or cover painting beyond shared placeholder dimensions/flags, playback/enqueue intents, download/progress state, or Book persistence mechanisms.
- Change `BrowserComponent`, other destinations, or shared media-list, painter, anchor, arrangement, pill, detail-shell, hero-content, duration, palette, or tick-harness helpers.
- Add another router, mounted identity, subscription, focus boundary, or gesture recognizer.

## Decisions

### D1. Persistent book controls own live position

Add `wide_book_list: WideMediaList<String>` beside the existing persistent `narrow_list`. The active book control owns selection, scroll, viewport, and row geometry. Book-row refresh uses stable-target-preserving control APIs and clamps locally when a target disappears; it does not adopt the content struct’s cursor during ordinary projection. Delete `browser_offset` and per-frame Wide-control construction.

`AudiobookshelfBookBrowseState::cursor()` and `select()` remain only on the effect path: after the component resolves a movement target/index, shell-owned detail fetch and library-position persistence may consume that resolved value.

Alternative: synchronize both controls from the content-struct cursor. Rejected because it preserves duplicate live authority and turns ordinary refresh into implicit reseeding.

### D2. Every book-selection path drives only the active control

Introduce a `select_book()`-style path that selects on the active control, then emits the resolved book request used by shell effects. Keyboard movement, bucket cycling, bucket-edge selection, clicks, and bucket-scoped clamps all use this path. The inactive control changes only during explicit responsive handoff, not ordinary interaction.

Alternative: keep `move_book`, `cycle_bucket`, and `select_bucket_edge` mutating `AudiobookshelfBookBrowseState` first. Rejected because the control would remain a projection of parent state instead of the interaction owner.

### D3. Book pointer interaction uses retained current-frame control geometry

Add a `book_index_at()` path mirroring Podcast’s `show_index_at()`. Clicks resolve with the active control’s `resolve_current_point`; blank unclaimed space returns no request. Wheel input is eligible only when the active control `claims_current_point`, moves that control directly, and returns the resolved book request. Delete the `book_rows` map and `push_book_rows` rebuild.

Alternative: retain explicit rectangle scans as a fallback. Rejected because a fallback is a second hit authority that can resolve geometry the control did not paint in the current frame.

### D4. Responsive handoff is control-to-control through one anchor

On a Wide/Normal transition, capture `viewport_anchor()` from the outgoing active control and apply it to the incoming persistent control. The anchor carries stable selected target plus selected ordinary-row offset. Delete the `state.select(idx)` handoff reseed and derive the offset from the control rather than retaining `painted_row_offset`.

Alternative: reconstruct the destination cursor and scroll from content state plus paint writeback. Rejected because that recreates destination-specific viewport authority outside the controls.

### D5. Chapter workspace authority is explicitly excluded under umbrella row 8.2

The controlling specification text is `canonical-media-lists`: “chapter rows remain provider-owned seek targets for the selected book” and “provider workspaces, images, selectors, surname buckets, effects, and typed intent translation remain parent-owned.” Therefore the chapter `WideMediaList`, `chapter_selection`, `chapter_rows`, `move_chapter`, `ChapterFocus` no-op storage, and `activate_audiobookshelf_book_row` absolute-seek mechanics remain Book-owned and are not converted by this repair.

The narrow-projection requirement is not a transfer of chapter mechanics: the existing `audiobookshelf-book-browsing` specification says “the selected book’s hero and chapter detail SHALL replace the active book row.” This change projects those existing provider-owned chapter rows into the admitted inline detail while preserving their ownership and seek semantics.

Alternative: migrate chapter-list geometry as a second canonical rail. Rejected because it silently expands row 8.2 and contradicts the explicit provider-workspace boundary.

### D6. Chapter activation carries its resolved index across the typed boundary

Change the component intent to `ActivateChapter(Option<usize>)`, populated from component-owned chapter selection at event time. The shell dispatches that value without downcasting the mounted component. This changes only intent plumbing; absolute seek remains in the existing shell/App effect path.

Alternative: keep the shell downcast. Rejected because it reads component-local interaction state back across the boundary at effect time.

### D7. Presentation convergence reuses shared policies

Build narrow selected-book detail with the shared selected-detail shell and hero-content budgeting pattern, analogous to `podcast_hero_content_rows`, so chapter rows occupy the replacement budget and ordinary-row fallback remains controlled by `InlineMediaBrowser`. Adopt `SERIES_IMAGE_COLS`/`SERIES_IMAGE_ROWS` instead of forked `18x12` literals and use the accepted cover placeholder flag while loading. Empty/loading paths call `invalidate_paint()` on both book controls at both breakpoints.

Hero meta already supplies the required inline `%`/`Finished` state and remains verify-only. Bucket pills and Wide rail composition already use conforming shared painters and remain unchanged.

Alternative: add Book-specific geometry, constants, or painters. Rejected because accepted shared helpers already express the presentation and umbrella D8 requires convergence rather than another fork.

### D8. Verification distinguishes the rail from the legitimate chapter workspace

Register `tests_tick_integration_book.rs` and exercise Wide and Normal/Narrow through `Application::tick()`, `draw_frame`, `sync_mounted_surfaces`, and the shell sync pass. Assertions use discriminating `raw_messages` checks for claimed and unclaimed interactions, prove the painted selected book matches navigation, cover both wheel legs, verify blank clicks are no-ops, and preserve target plus row offset through a breakpoint round-trip.

With cached chapters, the Book workspace legitimately invokes a second `render_wide_media_list`. Paint accounting therefore proves exactly one **book rail** painter and no base-frame underpaint, rather than asserting that the global wide-list counter is one or that the chapter workspace paints zero times. Focused tests cover refresh preservation/clamp, bucket paths, retained invalidation, control anchors, and resolved chapter carry; buffers cover narrow chapter detail, hero percentage metadata, pill rows, and cover placeholder parity.

Alternative: use only direct component tests or a global one-paint assertion. Rejected because the former misses TuiRealm delivery and shell synchronization, while the latter misclassifies the legitimate provider workspace as duplicate rail painting.

## Risks / Trade-offs

- [Moving all selection paths exposes stale assumptions in shell persistence/detail fetch] → Keep content selection effect-path-only and assert component-resolved indices through real shell ticks.
- [Bucket changes can leave a selected target outside the visible bucket] → Perform bucket-scoped clamp on the active control and cover cycle and edge paths with focused tests.
- [Inline chapter detail exceeds available replacement height] → Reuse `InlineMediaBrowser` admission/fallback and a bounded shared-style content budget.
- [Deleting book geometry breaks legitimate shell readers] → Retain only chapter-workspace and shell-owned chrome/image/overlay rects; prove book hits use only control-retained geometry.
- [Global paint counters obscure duplicate rail painting] → Add destination-aware rail accounting or an equivalent discriminating assertion while explicitly allowing one cached-chapter workspace paint.

## Migration Plan

1. Characterize focused list behavior, Wide and Normal/Narrow shell-tick composition, narrow chapter presentation, cover placeholder parity, and empty/loading claim invalidation.
2. Add the persistent Wide book control; move selection paths, retained hits, wheel claims, and responsive handoff to active controls; remove parent/content presentation mirrors and bespoke book geometry.
3. Carry the resolved chapter index across the typed intent boundary and converge narrow detail and placeholder projection through existing shared helpers.
4. Run breakpoint-focused tests and the full bounded gate list. No persisted-data migration is involved; rollback reverts source and tests together.
5. Close umbrella rows 8.1–8.3 only after the bounded PR merges and receives reviewer sign-off, per umbrella D5, then proceed to section 9 close-out.
