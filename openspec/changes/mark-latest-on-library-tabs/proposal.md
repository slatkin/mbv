# Proposal

## Why

The new-content marker only appears on a destination's `Latest` pill, so it is invisible until the user is already on that library's tab. Showing it on the library's tab in the tab bar tells the user which library has new items without visiting each one.

## What Changes

- The tab bar shows the Iris new-content marker on each library tab (Emby library, Audiobookshelf podcast library, Feeds) whose Latest marker is currently lit.
- The tab marker uses exactly the same condition and acknowledgement as the pill marker: it clears when that destination's `Latest` is selected, and stays cleared for the run.
- The pill marker is kept as-is (least churn; this area is expected to change again soon).
- Tab widths do not change: the marker occupies one of the tab's existing padding columns.

## Capabilities

### New Capabilities

### Modified Capabilities
- `destination-latest-modes`: "Latest markers follow their destination pills" extends the marker to the destination's tab in the tab bar.

## Impact

- `src/app/shell/chrome_panels.rs` (`sync_tab_panel`), `src/app/components/tab_panel.rs`, `src/app/render/components/chrome_tabs.rs` (`TabBarModel`, `render_tab_bar`).
- Marker condition shared with the existing per-destination push sites in `src/app/shell/{emby_library_content,tv_workspace,audiobookshelf_podcast,feeds}.rs`.
- No new fetches, persistence, or protocol changes.
