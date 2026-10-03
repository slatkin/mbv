# Spec Delta

## Purpose

Provides an explicit global action for returning live and saved TUI presentation state to defaults without resetting media, playback, Service setup or configuration.

## ADDED Requirements

### Requirement: F2 exposes a global Reset UI State action

The F2 Settings sidebar SHALL expose an action labelled `Reset UI State`. Activating it SHALL reset the running TUI as a whole, including retained inactive destinations, rather than only the selected destination or focused Panel. The action SHALL execute immediately, dismiss Settings and other transient overlays, and return to the default interface. It SHALL NOT require an application restart or introduce a new keyboard shortcut.

#### Scenario: Reset from Settings while Queue was focused
- **WHEN** the user opens F2 from Queue and activates Reset UI State
- **THEN** the entire TUI SHALL reset and Settings SHALL close
- **AND** the action SHALL not be limited to Queue or the selected browse destination

### Requirement: Reset restores presentation defaults throughout the TUI

Reset SHALL select Home, restore the existing default Panel mode, widths, artwork visibility and artwork-versus-visualizer presentation, and apply the existing startup focus rules for the current terminal geometry. At ordinary two-panel geometry Library SHALL hold focus; geometry-forced Mini SHALL retain its existing startup Queue-focus rule.

Every destination SHALL return to its root/default browse scope, default sort and filter, default main Selector pill, first selectable item (or no selection for empty content), browser rather than Workspace-local focus, and initial scroll. Trees SHALL collapse to their normal initial expansion. Search queries/results, Inline Search, Library Hero overlays, multi-selection, Visual mode, transient drafts, pointer/resize gestures and pending UI navigation/re-anchor intents SHALL be cleared. Queue's local cursor, marks and scroll SHALL return to its normal current-content initialization without changing the viewed Queue scope or its data.

Defaults SHALL use current content and the destination's existing count-dependent rules, never stale persisted indices. Reset SHALL retain fetched media and artwork rather than invalidate caches or run a bulk data refresh. Ordinary lazy reads needed to display an uncached default scope SHALL retain their existing navigation behavior. Late results SHALL NOT restore discarded search, browse or launch intents.

#### Scenario: Inactive destinations reset too
- **WHEN** multiple destinations have different pills, scroll positions and expanded trees and Reset UI State is activated from another destination
- **THEN** returning to each retained destination SHALL show its default context
- **AND** its available fetched content SHALL remain available

#### Scenario: Narrow geometry and a populated Queue
- **WHEN** Reset UI State is activated in geometry-forced Mini while Queue contains media
- **THEN** existing Mini startup presentation and focus SHALL apply
- **AND** Queue's local selection SHALL initialize normally without changing its slots or playback

#### Scenario: An old search result arrives after reset
- **WHEN** a search or UI-navigation completion initiated before reset arrives afterward
- **THEN** it SHALL NOT reopen the discarded UI or re-anchor it to the discarded selection
- **AND** ordinary valid content updates SHALL remain eligible under their existing guards

### Requirement: Reset clears saved presentation state selectively

Reset SHALL immediately clear saved TUI launch location, per-library browse positions and custom presentation-layout preferences, including legacy presentation values that could reinstate those choices. A new Client launched after successful clearing SHALL start from defaults until a subsequent orderly exit supplies a new launch snapshot. This explicit reset SHALL NOT erase unrelated values merely because they share a persistence file. A missing presentation-state file SHALL count as already cleared.

If a required persistence operation fails, the live UI SHALL still reset, but mbv SHALL report that saved UI state could not be fully cleared rather than claim full success. Reset SHALL NOT change another running Client's in-memory UI or prevent its later orderly exit from saving its own launch location.

#### Scenario: Saved presentation state is forgotten immediately
- **WHEN** the user resets UI state and starts another Client before the resetting Client exits
- **THEN** the new Client SHALL start with default launch, browse and layout state
- **AND** unrelated saved state SHALL remain available

#### Scenario: Saved-state clearing fails
- **WHEN** clearing saved presentation state fails
- **THEN** the running TUI SHALL show its reset interface
- **AND** mbv SHALL report the saved-state failure without a full-success notification

### Requirement: Reset does not reset domain state or explicit settings

Reset SHALL NOT modify explicit settings or keybindings; Service setups, credentials or availability; feed subscriptions or library routes; Owner attachments, cast attachments or playback targets; queue slots, ordering, source, lineage or queue-edit undo; playback transport, position, volume or mute; watched/listening progress; cached media/artwork; or launch-window timestamps and Latest acknowledgements. It SHALL NOT send Player-owner queue or transport commands. Neither UI-state reset nor F5 SHALL perform a factory reset.

#### Scenario: Reset during playback with configured Services
- **WHEN** media is playing and the user activates Reset UI State
- **THEN** playback, queue, attachment/target and volume/mute SHALL remain unchanged by the action
- **AND** settings, Service credentials, subscriptions, routes and progress SHALL remain intact

#### Scenario: Latest has already been acknowledged
- **WHEN** a destination's Latest marker has been acknowledged and the user resets the UI
- **THEN** the destination SHALL use its default presentation selection
- **AND** its acknowledged marker SHALL remain cleared for the current launch window
