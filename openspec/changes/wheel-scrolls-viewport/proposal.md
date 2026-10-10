# Proposal

## Why

The mouse wheel moves a list's selection one row at a time. It also passes through a 30 ms throttle that drops the extra notches of a fast flick. So scrolling a long list is slow, and spinning the wheel faster does not help. Each step also changes the selection, which re-targets the hero/detail pane, writes the resting position and extends a live range selection. All of that is work the user did not ask for: they wanted to look, not to select.

## What Changes

- **BREAKING (interaction):** On every scrollable surface, a wheel notch scrolls the **viewport**. It does not move the selection. The selection may scroll out of view. The detail pane, hero and resting cursor do not change while the user scrolls.
- One wheel notch moves a fixed step of **3 rows** (lists) or **3 lines** (text viewports). The step is the same on every surface. This replaces the one-row rule.
- The wheel throttle is removed. Every wheel event that reaches a recognizer becomes a step, and no notch is dropped.
- A list's viewport has two anchor states: it follows the selection, or it was scrolled freely by the wheel. Any selection-changing operation (keyboard move, page, first/last, click, select, restore, shell re-anchor) returns it to following the selection. A key pressed after a scroll therefore first brings the selection back into view, then acts on it. This is the GUI convention.
- A wheel scroll never extends a live range selection.
- Lazy library paging (Emby library lists, Grouped Music) also triggers when the bottom of the viewport nears the loaded edge, not only when the cursor does. This keeps a wheel scroll from stopping on unloaded rows.
- The Playlists and Global Search sidebar overlays get the same viewport scroll. Their painters stop pulling the offset back to the cursor while the viewport is scrolled freely.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mouse-input`: The uniform wheel policy changes from a one-row selection step to a fixed 3-row/line viewport scroll. Gesture recognition loses the wheel throttle. The pointed-list wheel scenario and the per-surface verification rule change to match.
- `shared-list-components`: "The viewport keeps the selection visible" becomes "visible unless the user scrolled the viewport freely". It adds the follow/free anchor states and the rule for returning to follow.
- `canonical-media-lists`: Search-result wheel input scrolls the results viewport instead of moving the cursor. The window-raise requirement no longer says the wheel keeps its old meaning.

## Impact

- `crates/mbv-components/src/mouse/gesture.rs`: throttle removed; the scroll step constant lives here.
- `crates/mbv-components/src/list/viewport.rs` (`Viewported`) and its implementors `MediaList`, `WideMediaList`, `ThreeLineFlatList`, `TreeState`: free-scroll operation and the follow/free anchor.
- `crates/mbv-components/src/media_list/selection.rs`, `media_list.rs`: `Wheel` maps to a new scroll operation, not `Move`.
- Wheel handlers in Home, Emby library, TV (episodes and show tree), Music tree, Queue, Podcast/Feeds, Audiobookshelf books, Sessions, Playlists, Search sidebar, Settings, Help, and the library hero overview.
- `crates/mbv-ui-msg/src/shell.rs` and `src/app/shell/`: a typed viewport-reach request feeds `maybe_fetch_next_page`; the Emby library wheel stops sending `EmbyLibraryCursorIndex`; the Music wheel stops sending an album-cursor move.
- `crates/mbv-render/src/components/playlists.rs`, `search_sidebar.rs`: painters respect a free viewport.
- `docs/architecture/interactive-surface-ledger.md`: wheel rows updated.
- No dependency, config or protocol changes.
