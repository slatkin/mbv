# Proposal

OpenSpec change: `separate-data-refresh-from-ui-reset`.
Planning source: `openspec/changes/separate-data-refresh-from-ui-reset/`.

## Why

[F5 issue #745](https://github.com/slatkin/mbv/issues/745) asks what else refresh should reset, but the intended boundary is the opposite: F5 fetches current data without disturbing the interface. A separate, global F2 action should reset live and saved TUI presentation state without touching playback or configuration.

## What Changes

- Make F5 refresh the selected browse destination (Home, one Emby library, one Audiobookshelf book/podcast library, or Feeds), regardless of Library/Queue focus or whether the Library panel is visible. Keep existing blocking-overlay precedence.
- Remove F5's Queue refresh branch, split-width reset, preference write, and saved-library-position invalidation. Do not change explicit Player-owner Queue refresh operations elsewhere.
- Preserve the destination's valid browsing context while fetching and applying updated content, including through Audiobookshelf's paged catalog and episode reloads. Missing identities use existing destination reconciliation rules rather than retaining invalid selections.
- Add **Reset UI State** to F2 Settings as a global action. Reset active and retained inactive destinations, search/overlays, selection/scroll/focus, navigation and presentation layout to their existing defaults.
- Reset saved launch, library-position and layout state immediately, not just at a future exit. Ordinary exit may subsequently save a new launch location; another running Client keeps its own live state.
- Preserve explicit settings, keybindings, Service setup/credentials, subscriptions, library routes, playback targets, queue contents/source/undo, volume/mute, media progress, cached media/artwork and the existing launch-window Latest acknowledgements.
- Leave other broken global-key behavior, automatic-refresh policies, API changes and broader cache eviction outside this change.

## Capabilities

### New Capabilities

- `tui-state-reset`: Explicit F2 reset of global live and saved presentation state, with protected domain state and clear persistence-failure feedback.

### Modified Capabilities

- `service-browse-dispatch`: F5 targets the selected browse destination independently of Panel focus, never refreshes Queue, and preserves valid UI context during data refresh.
- `tui-launch-state`: Explicit UI reset is a narrow exception to exit-only launch-state persistence; it clears saved and pending restoration without changing ordinary last-completed-exit behavior.
- `feed-subscriptions`: Recognize F5 alongside existing explicit `r` refresh, including Latest, without introducing automatic fetches or changing stored feed progress semantics.

## Impact

- TUI shell refresh/effect dispatch, Audiobookshelf catalog/detail result handling, library-position and launch restoration, retained Library content owners, Queue-local presentation, overlay lifecycle and Settings action dispatch.
- Shared UI vocabulary in `mbv-ui-model` / `mbv-ui-msg`, existing `LibraryContentOwner` ownership seam in `mbv-components`, and targeted presentation-state persistence in `mbv-config` / TUI preferences.
- Existing refresh and selection tests should be adapted rather than multiplied; new verification targets reset's cross-boundary protection and selective persistence.
- No new dependencies, remote endpoints, ctrl protocol changes, Player-owner behavior changes or new keyboard-routing site. This is planning only; main specs and project code remain unchanged until apply.
