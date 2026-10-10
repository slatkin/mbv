# Design

## Context

See proposal.md for the motivation. These facts about the current code shape the approach:

- `MouseGestureState` (`crates/mbv-components/src/mouse/gesture.rs`) emits `MouseGesture::Scroll { at, delta: ±1 }`. It drops any wheel event that arrives within 30 ms of the last accepted one. The run loop polls with `PollStrategy::Once`, which returns one event per tick, so a burst is already spread over several ticks. The throttle drops real notches.
- Every canonical media list sends the wheel through `MediaListSurfaceInput::Wheel`, and `media_list/selection.rs:9` maps it to `MediaListOperation::Move(delta)`. The tree browsers (Music, TV show tree) call `TreeOperation::Move(delta)` directly.
- Viewport arithmetic is shared through the `Viewported` trait (`list/viewport.rs`), implemented by `MediaList`, `WideMediaList`, `ThreeLineFlatList` and `TreeState`. `resolved_viewport_offset` pulls the offset back to the selection on every resolve. That is the line that would undo a free scroll on the next paint.
- Shell paging (`maybe_fetch_next_page`, `PREFETCH_AHEAD = 25`) is called from the Emby library cursor report (`EmbyLibraryCursorIndex`) and the Music album-cursor report (`AlbumCursorKind::Move`). The Emby library and Music wheels currently send those cursor reports. The TV series root and feed-home-video roots paginate without a trigger, so they need nothing.
- `QueueComponent::set_cursor` runs on every queue projection push. It clamps `scroll` to `<= cursor` even for `QueueCursorUpdate::Preserve`, which would pull a scrolled Queue back to the cursor.
- The Playlists painter (`mbv-render/src/components/playlists.rs:221`) clamps the scroll to the cursor on every paint. The Search sidebar keeps `sidebar.scroll` in its model and changes it only in `move_cursor`, so a free scroll there needs no anchor. Settings and Help already scroll their own offset by `delta`.

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

### D6: Overlay painters

- **Playlists:** the component owns `playlists_scroll`/`open_scroll` plus one `ViewportAnchor` per list. The wheel scrolls the offset (clamped to the row count minus the painted height) and sets `Free`. Cursor moves set `FollowSelection`. The painter's cursor clamp (`playlists.rs:221`) runs only when its params say the anchor is `FollowSelection`; it always clamps to the bounds.
- **Global Search sidebar:** the wheel adds `delta` to `sidebar.scroll`, clamped to `filtered_count - list_height`, and leaves the cursor alone. `move_cursor` already re-anchors on the next key. The only other writes to `scroll` are the type-filter resets (`search_sidebar.rs:199`, `:253`), which set both `cursor` and `scroll` to 0. Those are content resets, not anchor writes, so no anchor field is needed.

## Risks / Trade-offs

- [A terminal that sends several wheel events per physical notch now scrolls 3× as far per event] → The step is one constant; tune `WHEEL_STEP` if the real terminal over-scrolls. The user's terminal (Ghostty) can also scale wheel input itself.
- [Without the throttle, each wheel event costs a full tick and draw; heavy frames (images) could lag behind a long flick] → Accepted; this is a non-goal for this change. If it shows up, the follow-up is to drain queued events per tick (`PollStrategy::UpTo`), not to reintroduce dropping.
- [The detail/hero pane shows an item that is scrolled out of view] → This is the chosen model (GUI convention), confirmed by the user.
- [Many existing tests assert that the wheel moves the cursor] → Those assertions encode the replaced contract. Rewrite each to the new contract it owns (viewport moved, selection unchanged) or delete it where another test already owns that contract. Do not keep both.

## Migration Plan

Behavior-only change in one release. No persisted data changes shape: the persisted library scroll offset keeps its meaning (a flow-row offset), and restore re-anchors to the selection as before.
