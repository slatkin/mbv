## MODIFIED Requirements

### Requirement: A completed TUI session persists one launch location

On orderly exit, a TUI SHALL persist one launch-state snapshot containing the selected tab, whether Library or Queue held Panel focus, and, for the selected tab only, the destination's persisted pill scope. The persisted pill scope is the `Latest` pill for every destination whose main Selector offers a `Latest` pill, the selected group for a Grouped Music library, the selected bucket for an Audiobookshelf book library, and Home's fixed scope. A destination whose selector presents no pills SHALL record no pill. The snapshot SHALL NOT record the selected library item or any other live pill selection; pill choices other than the persisted scope are session memory. When the GUI Client exits with two or more groups selected in a Grouped Music library, the persisted pill scope SHALL be the first selected group, and the snapshot SHALL also carry the other selected groups, which only the GUI Client restores.

The snapshot SHALL NOT contain state for unselected tabs, a selected Queue item, a nested Workspace selector such as a TV season, an overlay or Sidebar, a search query or result, multi-selection other than the GUI Client's selected Grouped Music groups, Visual mode, undo history, scroll offsets, loading or error state, or paint geometry.

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
- **THEN** a Library destination whose Selector offers a Latest pill SHALL still restore on Latest, whether or not the exit view painted its pill row

#### Scenario: The GUI Client exits with several Music groups selected
- **WHEN** the GUI Client exits normally with Music active and the Hip-Hop and Soul groups selected, in that pill order
- **THEN** the snapshot SHALL record Hip-Hop as the persisted pill scope and Soul as an extra selected group
- **THEN** a TUI that starts from this snapshot SHALL restore Music on Hip-Hop only
- **THEN** a GUI Client that starts from this snapshot SHALL restore Music with Hip-Hop and Soul selected

#### Scenario: A saved extra group no longer exists
- **WHEN** the GUI Client starts from a snapshot whose extra Music groups include a group the library no longer has
- **THEN** it SHALL restore the remaining saved groups and drop the missing one without selecting another group in its place


### Requirement: Launch state is written only at orderly TUI exit

A TUI SHALL load the saved launch snapshot once during startup, keep subsequent launch-state changes in that TUI process's memory, and replace the saved snapshot only as part of orderly TUI exit. The GUI Client SHALL follow the same rule and write the same snapshot at its own orderly exit; the last completed exit of any Client supplies the next launch of either Client. Cursor movement, tab changes, pill changes, item changes, Panel-focus changes, refreshes, and rendering SHALL NOT write the launch snapshot while the TUI remains open.

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

#### Scenario: The GUI Client and a TUI exit in sequence
- **WHEN** a TUI exits normally on Shows and the GUI Client later exits normally on Music with Library focus
- **THEN** the next TUI launch and the next GUI Client launch SHALL both start from Music with Library focus
- **THEN** the GUI Client SHALL NOT write the snapshot while it remains open
