## 1. Wide TV Layout Foundation

- [x] 1.1 Add the TV-only wide-layout dispatch at the shared breakpoint and preserve the existing narrow hero-on-top fallback.
- [x] 1.2 Add layout bookkeeping for the wide TV right rail, left episode workspace, one-column row map, and interactive hit targets.
- [x] 1.3 Compose the shared hero-on-left panes, right-rail TV letter/search pills, one-column Series list, and existing list-panel focus chrome.

## 2. Persistent Series Workspace

- [x] 2.1 Resolve the selected Series from the active right-rail source, including the inline-search result cursor, and prevent stale detail from appearing after a Series change.
- [x] 2.2 Render the existing Series artwork, metadata, overview, loading state, and empty state persistently in the left pane.
- [x] 2.3 Render the current Series season pills in the left pane and keep them separate from the right-rail TV letter-range pills.
- [x] 2.4 Render the selected Series' current-season episodes as a persistent preview, with a bounded viewport and selected-episode visibility when episode selection is active.
- [x] 2.5 Keep season changes scoped to the selected Series' episode source and preserve existing episode fetching behavior for uncached seasons.

## 3. Pane Interaction

- [x] 3.1 Reuse the existing Series-selection state as the wide-pane mode boundary: Series browsing focuses right, and episode selection focuses left.
- [x] 3.2 Preserve Enter, Up/Down, season switching, Escape/Backspace, and episode playback behavior in wide episode-selection mode.
- [x] 3.3 Add wide TV mouse targets for Series rows, episode rows, and season pills; keep artwork and blank left-pane space inert.
- [x] 3.4 Ensure changing the right-rail Series clears or safely reinitializes episode selection and season/episode cursors.

## 4. Verification

- [x] 4.1 Add render coverage for wide TV's left Series workspace, persistent episodes, separate season and letter pill bars, and one-column right-rail Series rows.
- [x] 4.2 Add coverage proving the left workspace follows a Series cursor whose right-rail row is scrolled out of view and follows active search results.
- [x] 4.3 Add keyboard coverage for entering/exiting episode selection, moving episodes, changing seasons, and activating playback without changing the Series cursor.
- [x] 4.4 Add mouse coverage for episode and season targets, right-rail Series targets, and inert artwork/blank space; verify narrow TV mouse behavior remains unchanged.
- [x] 4.5 Run focused TV/render tests, workspace formatting and file-size checks, then verify wide, narrow, loading, empty, long-season, and queue-focus captures.
