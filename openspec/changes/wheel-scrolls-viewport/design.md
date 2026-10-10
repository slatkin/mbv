# Design

## Context

See proposal.md for the motivation. These facts about the current code shape the approach:

- `MouseGestureState` (`crates/mbv-components/src/mouse/gesture.rs`) emits `MouseGesture::Scroll { at, delta: ±1 }`. It drops any wheel event that arrives within 30 ms of the last accepted one. The run loop polls with `PollStrategy::Once`, which returns one event per tick, so a burst is already spread over several ticks. The throttle drops real notches.
- Every canonical media list sends the wheel through `MediaListSurfaceInput::Wheel`, and `media_list/selection.rs:9` maps it to `MediaListOperation::Move(delta)`. The tree browsers (Music, TV show tree) call `TreeOperation::Move(delta)` directly.
- Viewport arithmetic is shared through the `Viewported` trait (`list/viewport.rs`), implemented by `MediaList`, `WideMediaList`, `ThreeLineFlatList` and `TreeState`. `resolved_viewport_offset` pulls the offset back to the selection on every resolve. That is the line that would undo a free scroll on the next paint.
- Shell paging (`maybe_fetch_next_page`, `PREFETCH_AHEAD = 25`) is called from the Emby library cursor report (`EmbyLibraryCursorIndex`) and the Music album-cursor report (`AlbumCursorKind::Move`). The Emby library and Music wheels currently send those cursor reports. The TV series root and feed-home-video roots paginate without a trigger, so they need nothing.
- `QueueComponent::set_cursor` runs on every queue projection push. It clamps `scroll` to `<= cursor` even for `QueueCursorUpdate::Preserve`, which would pull a scrolled Queue back to the cursor.
- Two overlays are list-shaped but keep raw `cursor`/`scroll` integers outside the shared list layer. The Playlists overlay (`crates/mbv-components/src/playlists.rs:21-27`) holds two independent lists (saved playlists, open-playlist items). The shell projects a cursor/scroll mirror for both through `PlaylistsContent` (`:48-59`). Its painter (`mbv-render/src/components/playlists.rs` `paint_rows`, `:221-224`) clamps the scroll to the cursor on every paint and writes it back through `&mut scroll`. The Global Search sidebar keeps `cursor`, `scroll` and a painter-written `list_height` in `mbv-ui-model`'s `SearchSidebar` (`search_sidebar.rs:7-8,13`). Its cursor indexes the filtered view (`filtered_results()`, type-filter chips), not the source results. Both wheels move the cursor one row.
- Settings, Help and the library hero overview already scroll their own offset by `delta`, and nothing pulls that offset back to a cursor on paint (D7).

## Goals / Non-Goals

**Goals:**
- Give every list one viewport model, shared through `Viewported`, so lists cannot diverge.
- Make "free" versus "following the selection" a type, not a convention.

**Non-Goals:**
- Wheel acceleration or a configurable step.
- Changing `PollStrategy` or draining several events per tick. Each wheel event still costs one tick and draw.
- Horizontal wheel, and the status-bar volume wheel (it reads raw events, not gestures).
- Keyboard paging distances.

## Decisions

### D1: The wheel step lives in the recognizer

`MouseGestureState` emits `Scroll { delta: ±WHEEL_STEP }` with `const WHEEL_STEP: i64 = 3`, and the throttle and its `last_scroll` field are deleted. Every consumer receives the step already scaled. Settings, Help and the library hero overview already add `delta` to their offset, so they get 3 lines with no edit. The one constant is the uniform policy.

*Alternative:* each component multiplies `±1` by a shared constant. Rejected: that is one multiply per handler, which can be forgotten, for no gain. Sessions currently takes only the sign of `delta`; it moves to the shared scroll operation (D3), which uses the value.

### D2: A viewport anchor enum on `Viewported`

```rust
pub enum ViewportAnchor { FollowSelection, Free }
```

`Viewported` gains `viewport_anchor()` / `set_viewport_anchor()` accessors (each implementor stores one field, default `FollowSelection`) and a shared method:

```
scroll_viewport(flow, viewport_len, delta)  -> offset := clamp(offset + delta); anchor := Free
```

`resolved_viewport_offset` applies the selection pull only when the anchor is `FollowSelection`. With `Free` it only clamps to the flow bounds. The reset to `FollowSelection` belongs to the **operations**, not to the `Cursored` primitives: the `delegate_operation` arms for move, page, first/last, `Select`, `Toggle`, `Range`, `Activate`, `ActivateCurrent` and `ContextCurrent`; the matching `TreeOperation` arms (including `AnchorSelection` restore); and the explicit shell re-anchor seams (`QueueCursorUpdate::Set`, restore). The re-anchor then happens in the existing resolve path with no new code.

The `Cursored` primitives (`select_target`, `move_selection`) must **not** reset the anchor, because content replacement reuses them. `MediaList::set_content` (`media_list.rs:426`) re-resolves the preserved stable target through `Cursored::select_target` on every row push, and the geometry clamp paths (`clamp_viewport`) run on every resize. Both must keep `Free`. Otherwise a queue projection during playback, a library refresh or a corpus arrival would snap a scrolled list back to the selection.

*Alternative:* a `bool detached`. Rejected under the types-over-flags rule: the enum names the state machine, and `match` keeps later readers honest.

*Alternative:* re-anchor only on the next selection change, but without storing an anchor (just skip one resolve). Rejected: a repaint, resize or content push between the scroll and the next key would snap the viewport back.

### D3: New list operations, not a re-use of `Move`

- `MediaListOperation::Scroll(i64)`: `selection.rs` maps `Wheel { delta }` to it. `delegate_operation` calls `scroll_viewport`, so it is not in `extends_range` and does not touch selection or multi-selection. Its disposition is `Consumed` when the offset changed and `Unhandled` at a boundary (the existing claim rules keep claiming the gesture either way).
- `MediaListOperation::Context(target)` (pointer right-click) leaves the anchor unchanged, so right-clicking a visible row while scrolled does not jump the view. Every other operation runs through selection writes that set `FollowSelection` (D2). `ActivateCurrent`/`ContextCurrent` explicitly set `FollowSelection` because they are keyboard operations on the selection (spec: a key after a scroll brings the selection back).
- `TreeOperation::Scroll(i64)` does the same for `TreeState`. Music and TV tree wheel handlers call it in place of `Move`.
- `ThreeLineFlatList` (Sessions) exposes the same `scroll_viewport`, and the Sessions wheel calls it in place of `move_selection`.

### D4: One typed request for wheel paging

New `ShellRequest::LibraryViewportReach { index: usize }`. After a wheel scroll, a paged library surface resolves the **last painted selectable row** to the same index space its cursor report already uses: the item index for the Emby library list, the album display index for Grouped Music. It then sends this request in place of today's cursor report. The shell handler sets Library panel focus (the Grouped Music spec requires that a claimed wheel focuses Library; the Emby list gets the same, so library lists don't diverge) and calls `maybe_fetch_next_page(active lib_idx, index)` without the navigation-idle gate. That gate exists to hold back image fetches during rapid keyboard moves. Paging already guards `loading` and `is_fully_loaded`, and a gated wheel could stop scrolling on the loaded edge with no later event to retry.

The Emby library wheel stops sending `EmbyLibraryCursorIndex`, and the Music wheel stops sending `pointer_album_selection_request(Move)`. Both of those were selection effects. The Library panel's `LibraryScroll { key, index, scroll }` persistence stays: it now persists the free offset, and restore re-anchors to the selection (D2, restore sets `FollowSelection`).

*Alternative:* put the last painted index into `LibraryScroll`. Rejected: that request is scroll persistence for `LibraryKey::Service` owners only, and Music does not route through it.

### D5: Queue only clamps on an authoritative set

`QueueComponent::set_cursor` clamps scroll to the cursor only inside the `QueueCursorUpdate::Set` arm, where `select_index` already sets `FollowSelection`. `Preserve` leaves the viewport alone. There is no automatic follow-the-playhead today. Re-anchors happen only on jump-to-now-playing, scope switch and deletion, which are all `Set`, so the Queue needs no extra rule.

### D6: List-shaped overlays move onto the shared list owner

User decision: "centralising overlays takes priority". The Playlists overlay and the Global Search sidebar do not get their own anchor or clamp. Each list moves onto the shared canonical owner, `MediaList`, through its public carrier `MediaListCarrier` (`media_list/carrier.rs`), as Queue and Inline Search already do. The wheel then goes through `MediaListOperation::Scroll` → `Viewported::scroll_viewport` → `ViewportAnchor::Free`. The follow and clamp happen in the one `resolved_viewport_offset`. No overlay holds a `ViewportAnchor`, an offset clamp, or a cursor-follow rule of its own.

Why `MediaList` and not another shared type: both overlays paint flat, one-line, all-selectable rows. `ThreeLineFlatList` is a three-line item shape with a fixed painter, and `TreeState` is a nesting shape. `MediaList` is the flat one-row owner. The carrier already has every operation the overlays need (`set_content`, `select_first`, `reset_presentation`, `delegate_operation` with `Move`/`First`/`Last`/`Select`/`Scroll`, `cursor`, `scroll`, `set_scroll`, and the anchor-aware `clamp_viewport(height)`), so the shared type needs no groundwork.

- **Targets.** Shared-list-components requires stable targets ("Content replacement preserves selection by stable identity"). The saved-playlists list uses the playlist `EmbyItem::id`. The open-playlist list uses `EmbyItem::playlist_item_id`, because one item can appear twice in a playlist, so the item id does not identify a row. `update_playlist_items` already relies on `PlaylistItemId` from the same `/Playlists/{id}/Items` endpoint. The Search sidebar uses the item id, like Inline Search (`MediaListCarrier<String>`).
- **Rows.** Both overlays keep their own painters, so the visible rows do not change. Each painter reads the overlay's `EmbyItem` slices, as today. The carrier gets one `MediaListRow::Item` per row for the flow. The Search sidebar reuses Inline Search's `search_result_row`.
- **Painters stop owning the offset.** A painter takes the cursor by value, plus a `resolve_offset: &mut dyn FnMut(usize) -> usize` (painted list height → offset). The component wires that to the visible carrier: `clamp_viewport(height)`, then `scroll()`. The Playlists cursor clamp and `&mut scroll` write-back (`playlists.rs:217-224`), its open-list cursor write-back (`:339`), and the Search sidebar's `list_height` write (`search_sidebar.rs:177`) are deleted. *Recommendation:* use the resolver closure over a separately exported list-rect function, so the list layout stays in the painter that draws it. `mbv-render` cannot name the carrier, because it sits below `mbv-components`.
- **Playlists: two lists, one component.** The component holds `list` and `open_list` carriers and one `active_list_mut()`, chosen by `open.is_some()`. Every key, click and wheel arm goes through it, in place of today's paired `if self.open.is_some()` branches. The shell mirror is deleted: the four `PlaylistsContent` cursor/scroll fields and the four `App` fields behind them. AGENTS.md: "`sync_*` and `push_*` carry shell-owned content, not cursor, scroll, or selection mirrors." No shell code reads them, and their values were always 0: the shell only resets them (`load.rs:177-178`) or clamps them (`event.rs:285`). A saved-playlists refresh preserves the selected playlist by target. Opening a different playlist resets the open list (`reset_presentation`), which matches today's reset to row 0. The `Playlists*` shell requests keep today's index payloads, read from `carrier.cursor()`. Every row is selectable, so the selectable index equals the projected-vec index the shell resolves.
- **Search sidebar: the filtered view and the model split.** `mbv-ui-model` sits below `mbv-components` and cannot hold the carrier. So `cursor`, `scroll` and `list_height` leave `SearchSidebar`. The carrier lives in `SearchSidebarComponent`, and `SearchSidebar` keeps the query, results, `type_filter`, loading and error. The carrier's rows are always `filtered_results()`. A private `publish_results()` sets them and resets to the first row at the top: the same reset Inline Search does in `publish_rows(reset = true)`. It runs after every change to the filtered view: a query edit, an applied drain, and a type-filter change (Tab/BackTab and a chip click). Those are the only writers, so the carrier's index is always an index into the current filtered view. `apply_drain` returns whether it applied, so a stale-query discard does not reset the selection.
- **Hit testing is unchanged.** Overlay rows stay in `HitRegions`, as irregular painted chrome (mouse-input). The component maps a painted row index to the carrier row's target. The Playlists wheel stays focus-owned. The Search sidebar wheel stays gated on a painted result row.

*Alternative:* a `ViewportAnchor` field and a guarded clamp per overlay (the previous D6). Rejected by the user decision: it is a second copy of the shared anchor and clamp in each overlay.

*Alternative:* paint both overlays through the canonical `WideMediaList` painter. Not taken here, because it changes the visible rows (zebra stripes, the loaded-playlist colour, the numbered lead, the type badge). It is an open product choice, not a step this change needs.

### D7: Document and text viewports keep their own offset

Settings, Help and the library hero overview take no `ViewportAnchor`, because the anchor has nothing to do in them:

- **Settings** is a variable-height document. `geometry.cursor_lines` maps cursors to document lines, and header rows are non-selectable. Three cursors (`cursor`, `services_cursor`, `keys_cursor`) across four destinations share one `scroll`. This is not a one-row flow over one selection, so it does not fit `Viewported`. Its painter takes `scroll` by value. Only the wheel and the key arms write `scroll`, and the key arms call `scroll_cursor_into_view`. So a wheel scroll already stays put until a key brings the cursor back, which is the D2 behaviour with no anchor.
- **Help** and the **hero overview** are prose viewports with no rows and no selection. With no selection, there is nothing to follow. Their painters only clamp at `max_scroll`.

All three already add the gesture `delta`, so D1 alone gives them the 3-line step. Wheel tests are missing. Help has only the tick test `tick_help_sidebar_scrolls_immediately_after_open_without_click`, which still asserts a 1-line step. Settings and the hero overview have no wheel test at any layer. The mouse-input requirement "Wheel behavior is verified for each scrollable surface" needs one proof per surface (task 5.5).

## Risks / Trade-offs

- [A terminal that sends several wheel events per physical notch now scrolls 3× as far per event] → The step is one constant; tune `WHEEL_STEP` if the real terminal over-scrolls. The user's terminal (Ghostty) can also scale wheel input itself.
- [Without the throttle, each wheel event costs a full tick and draw; heavy frames (images) could lag behind a long flick] → Accepted; this is a non-goal for this change. If it shows up, the follow-up is to drain queued events per tick (`PollStrategy::UpTo`), not to reintroduce dropping.
- [The detail/hero pane shows an item that is scrolled out of view] → This is the chosen model (GUI convention), confirmed by the user.
- [A saved-playlists refresh (rename, delete, `r`) used to put the cursor back on row 0, through the dead shell mirror. It now keeps the selected playlist by target] → This follows the existing shared-list-components rule for every list. A vanished target falls back to the first row.
- [The open-playlist target is `playlist_item_id`. If a server returned it empty, all rows would share one target] → `update_playlist_items` already depends on this field from the same endpoint. Confirm it with a live probe before 5.3b lands.
- [Many existing tests assert that the wheel moves the cursor] → Those assertions encode the replaced contract. Rewrite each to the new contract it owns (viewport moved, selection unchanged) or delete it where another test already owns that contract. Do not keep both.

## Migration Plan

Behavior-only change in one release. No persisted data changes shape: the persisted library scroll offset keeps its meaning (a flow-row offset), and restore re-anchors to the selection as before.
