# tui-launch-state Specification

## Purpose

Defines the small, coherent TUI state snapshot that one completed session leaves as the starting location for the next launch.

## Requirements

### Requirement: A completed TUI session persists one launch location

On orderly exit, a TUI SHALL persist one launch-state snapshot containing the selected tab, whether Library or Queue held Panel focus, and, for the selected tab only, the destination's persisted pill scope. The persisted pill scope is the `Latest` pill for every destination whose main Selector offers a `Latest` pill, the selected group for a Grouped Music library, the selected bucket for an Audiobookshelf book library, and Home's fixed scope. A destination whose selector presents no pills SHALL record no pill. The snapshot SHALL NOT record the selected library item or any other live pill selection; pill choices other than the persisted scope are session memory.

The snapshot SHALL NOT contain state for unselected tabs, a selected Queue item, a nested Workspace selector such as a TV season, an overlay or Sidebar, a search query or result, multi-selection, Visual mode, undo history, scroll offsets, loading or error state, or paint geometry.

#### Scenario: A session exits from a library item
- **WHEN** a TUI exits normally with a tab, a non-default main Selector pill, and a library item selected and Library holding Panel focus
- **THEN** its launch snapshot SHALL identify that tab, the tab's persisted pill scope, and Library focus
- **THEN** it SHALL NOT record the selected library item, the live pill, or state for an unselected tab

#### Scenario: Queue holds focus at exit
- **WHEN** a TUI exits normally while Queue holds Panel focus
- **THEN** its launch snapshot SHALL record Queue focus
- **THEN** it SHALL still record the selected Library tab's persisted pill scope
- **THEN** it SHALL NOT record the Queue cursor or selected Queue slot

#### Scenario: The selected tab has no main Selector pills or selectable items
- **WHEN** the selected tab presents no main Selector pill and no selectable library item at exit
- **THEN** the snapshot SHALL represent the absent pill without substituting a Queue selection or nested Workspace selection

### Requirement: Launch state is written only at orderly TUI exit

A TUI SHALL load the saved launch snapshot once during startup, keep subsequent launch-state changes in that TUI process's memory, and replace the saved snapshot only as part of orderly TUI exit. Cursor movement, tab changes, pill changes, item changes, Panel-focus changes, refreshes, and rendering SHALL NOT write the launch snapshot while the TUI remains open.

A Client that a Pin swap replaces SHALL save its snapshot when the Owner process asks it to prepare, before the replacement Client starts, and SHALL NOT save it again when it exits. If the swap is abandoned, that Client SHALL keep running and its later orderly exit SHALL save as usual.

The explicit F2 Reset UI State action SHALL clear the saved launch snapshot immediately and abandon any pending in-memory launch restoration. Clearing is the sole explicit UI-reset exception; it SHALL NOT write a continuously updated or default replacement snapshot. Later orderly exit SHALL again save that Client's current bounded launch location under the existing last-completed-exit rule. Another running Client SHALL retain its live location and remain able to supply a later exit snapshot.

Explicit configuration changes, playback lifecycle and progress, queue persistence, caches, Service-owned state, and auto-reconnect state SHALL retain their own persistence lifecycles and SHALL NOT be delayed by this requirement.

#### Scenario: Two TUI Clients diverge while open
- **WHEN** two TUI Clients start from the same saved launch snapshot
- **WHEN** each Client selects a different tab, pill, item, or Panel focus
- **THEN** each Client SHALL retain its own launch-state changes in memory
- **THEN** neither Client SHALL change the saved launch snapshot before orderly exit unless the user explicitly invokes Reset UI State

#### Scenario: Concurrent Clients exit in sequence
- **WHEN** two TUI Clients have different in-memory launch locations
- **WHEN** one Client completes orderly exit and the other Client completes orderly exit later
- **THEN** the later completed exit SHALL supply the snapshot loaded by the next TUI launch
- **THEN** no Client identity, field-level merge, or daemon synchronization SHALL be required

#### Scenario: An explicit setting changes
- **WHEN** the user changes explicit configuration while the TUI is open
- **THEN** that configuration SHALL keep its existing persistence behavior
- **THEN** the configuration write SHALL NOT write the TUI launch snapshot as a side effect

#### Scenario: Reset cancels pending startup restoration
- **WHEN** Reset UI State is activated before saved tab, pill, item or Panel focus has finished restoring
- **THEN** pending restoration SHALL be abandoned
- **THEN** later catalog or detail arrivals SHALL NOT restore the discarded launch location

#### Scenario: A Client exits after reset
- **WHEN** the user resets UI state, selects a new launch location and exits normally
- **THEN** the current bounded location SHALL be saved as the next launch's starting point
- **THEN** reset SHALL NOT permanently disable launch-state persistence

#### Scenario: Pin swap carries the launch location
- **WHEN** a Client showing a Library tab, pill and item is replaced by a Pin swap
- **THEN** the snapshot SHALL be saved before the replacement Client starts
- **THEN** the replacement Client SHALL restore that tab, pill, item and Panel focus

#### Scenario: Abandoned Pin swap
- **WHEN** a Client saved its snapshot for a Pin swap and the swap is abandoned
- **WHEN** the user later quits that Client normally
- **THEN** its exit SHALL save its launch location at that time

### Requirement: Launch restoration follows stable identities with ordered fallbacks

At startup, mbv SHALL restore the saved tab if that tab still exists. If it does not, mbv SHALL select the first guaranteed tab in presentation order. Within the restored tab, mbv SHALL apply the recorded persisted pill scope: the `Latest` pill for every destination whose main Selector offers one, the recorded group for a Grouped Music library, and the recorded bucket for an Audiobookshelf book library. A recorded Music group or book bucket that no longer exists SHALL fall back to the first guaranteed pill in that destination's presentation order. Within the restored scope's list, mbv SHALL select the first selectable item in presentation order; an empty list SHALL have no selected item. A saved library item, letter range, show, watched filter, or other non-scope pill in a snapshot written by an older version SHALL NOT restore.

If the restored tab presents no main Selector pills, mbv SHALL treat its unfiltered view as the restored scope and apply the item rule directly. Every fallback SHALL be resolved against current content, never by clamping or reusing a stale persisted index.

A saved Service tab SHALL be resolved only once the outcome of that Service's startup is known. If the Service's catalog arrived, the tab SHALL be restored when its library exists and otherwise fall back as above. If the Service is not configured, or its startup failed, the saved tab SHALL be treated as gone. Once that outcome is known, launch restoration SHALL NOT change the selected tab at any later time, including when the Service later connects.

The recorded pill scope and Panel focus SHALL be applied only to the tab that launch restoration selected. If a different tab is selected before they are applied, the remaining launch state SHALL be abandoned.

#### Scenario: Every saved identity still exists
- **WHEN** the saved tab exists and its recorded pill scope is available at startup
- **THEN** mbv SHALL restore the tab and the recorded pill scope with the first selectable item selected

#### Scenario: The saved item is gone
- **WHEN** a snapshot written by an older version records a selected library item, whether or not the item still exists
- **THEN** mbv SHALL restore the saved tab and its recorded pill scope
- **THEN** the first selectable item in the restored scope's list is selected and the saved item SHALL NOT restore

#### Scenario: The saved pill is gone
- **WHEN** the saved tab is a Grouped Music library or an Audiobookshelf book library whose recorded pill no longer exists
- **THEN** mbv SHALL select the first guaranteed pill in that destination's presentation order
- **THEN** the first selectable item in that pill's list is selected

#### Scenario: The saved tab is gone
- **WHEN** the saved tab no longer exists
- **THEN** mbv SHALL select the first guaranteed tab in current presentation order
- **THEN** it SHALL select that tab's persisted pill scope and first selectable item

#### Scenario: The restored list is empty
- **WHEN** the restored tab and pill scope have no selectable library item
- **THEN** mbv SHALL restore no selected library item

#### Scenario: The saved tab's Service is not configured
- **WHEN** the saved tab is a library of a Service that is not configured at startup
- **THEN** mbv SHALL select the first guaranteed tab without waiting for that Service

#### Scenario: The saved tab's Service fails at startup
- **WHEN** the saved tab is a library of a Service whose startup fails
- **THEN** mbv SHALL select the first guaranteed tab
- **THEN** a later successful connection of that Service SHALL NOT change the selected tab

#### Scenario: A Service is configured after launch
- **WHEN** the saved tab names a library of a Service that was not configured at startup
- **WHEN** the user configures that Service during the session
- **THEN** the selected tab SHALL NOT change

#### Scenario: The user selects a tab before the catalog arrives
- **WHEN** the user selects a tab explicitly before the saved tab's Service catalog has arrived
- **THEN** mbv SHALL discard the remaining launch state
- **THEN** the arrival of that catalog SHALL NOT change the selected tab

#### Scenario: The tab changes before the pill and item are applied
- **WHEN** launch restoration selected a tab
- **WHEN** a different tab becomes selected before the recorded pill scope and Panel focus are applied
- **THEN** mbv SHALL NOT apply them to the newly selected tab

### Requirement: Queue selection starts independently of launch-state restoration
Restoring Queue Panel focus SHALL NOT restore a Queue item selection. Queue selection SHALL initialize from the Queue component's normal current-content rule, independently of the saved Library launch location.

#### Scenario: Queue focus is restored
- **WHEN** a saved launch snapshot names Queue as the focused Panel
- **WHEN** the next TUI starts with Queue content
- **THEN** Queue SHALL receive Panel focus
- **THEN** its selected item SHALL come from normal Queue initialization rather than the prior TUI's Queue selection
