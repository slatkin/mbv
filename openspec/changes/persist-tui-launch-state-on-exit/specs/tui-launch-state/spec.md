# Spec Delta

## Purpose

Defines the small, coherent TUI state snapshot that one completed session leaves as the starting location for the next launch.

## ADDED Requirements

### Requirement: A completed TUI session persists one launch location
On orderly exit, a TUI SHALL persist one launch-state snapshot containing the selected tab, whether Library or Queue held Panel focus, and, for the selected tab only, the selected main Selector pill and selected library item. Tab, pill, and item values SHALL use stable identities where the destination supplies them rather than presentation indices.

The snapshot SHALL NOT contain state for unselected tabs, a selected Queue item, a nested Workspace selector such as a TV season, an overlay or Sidebar, a search query or result, multi-selection, Visual mode, undo history, scroll offsets, loading or error state, or paint geometry.

#### Scenario: A session exits from a library item
- **WHEN** a TUI exits normally with a tab, main Selector pill, and library item selected and Library holding Panel focus
- **THEN** its launch snapshot SHALL identify that tab, pill, item, and Library focus
- **THEN** it SHALL contain no selected Queue item or state for an unselected tab

#### Scenario: Queue holds focus at exit
- **WHEN** a TUI exits normally while Queue holds Panel focus
- **THEN** its launch snapshot SHALL record Queue focus
- **THEN** it SHALL still record the selected Library tab's pill and item
- **THEN** it SHALL NOT record the Queue cursor or selected Queue slot

#### Scenario: The selected tab has no main Selector pills or selectable items
- **WHEN** the selected tab presents no main Selector pill and no selectable library item at exit
- **THEN** the snapshot SHALL represent the absent pill and item without substituting a Queue selection or nested Workspace selection

### Requirement: Launch state is written only at orderly TUI exit
A TUI SHALL load the saved launch snapshot once during startup, keep subsequent launch-state changes in that TUI process's memory, and replace the saved snapshot only as part of orderly TUI exit. Cursor movement, tab changes, pill changes, item changes, Panel-focus changes, refreshes, and rendering SHALL NOT write the launch snapshot while the TUI remains open.

Explicit configuration changes, playback lifecycle and progress, queue persistence, caches, Service-owned state, and auto-reconnect state SHALL retain their own persistence lifecycles and SHALL NOT be delayed by this requirement.

#### Scenario: Two TUI Clients diverge while open
- **WHEN** two TUI Clients start from the same saved launch snapshot
- **WHEN** each Client selects a different tab, pill, item, or Panel focus
- **THEN** each Client SHALL retain its own launch-state changes in memory
- **THEN** neither Client SHALL change the saved launch snapshot before orderly exit

#### Scenario: Concurrent Clients exit in sequence
- **WHEN** two TUI Clients have different in-memory launch locations
- **WHEN** one Client completes orderly exit and the other Client completes orderly exit later
- **THEN** the later completed exit SHALL supply the snapshot loaded by the next TUI launch
- **THEN** no Client identity, field-level merge, or daemon synchronization SHALL be required

#### Scenario: An explicit setting changes
- **WHEN** the user changes explicit configuration while the TUI is open
- **THEN** that configuration SHALL keep its existing persistence behavior
- **THEN** the configuration write SHALL NOT write the TUI launch snapshot as a side effect

### Requirement: Launch restoration follows stable identities with ordered fallbacks
At startup, mbv SHALL restore the saved tab if that tab still exists. If it does not, mbv SHALL select the first guaranteed tab in presentation order. Within the restored tab, mbv SHALL restore the saved main Selector pill if that pill still exists; otherwise it SHALL select the first guaranteed pill in presentation order. Within the restored pill's list, mbv SHALL restore the saved library item if that item still exists and remains selectable; otherwise it SHALL select the first selectable item in presentation order. An empty list SHALL have no selected item.

If the restored tab presents no main Selector pills, mbv SHALL treat its unfiltered view as the restored scope and apply the item rule directly. Every fallback SHALL be resolved against current content, never by clamping or reusing a stale persisted index.

#### Scenario: Every saved identity still exists
- **WHEN** the saved tab, main Selector pill, and selected library item all exist at startup
- **THEN** mbv SHALL restore all three identities and the saved Panel focus

#### Scenario: The saved item is gone
- **WHEN** the saved tab and pill exist but the saved library item no longer exists or is no longer selectable
- **THEN** mbv SHALL select the first selectable item in that pill's current list

#### Scenario: The saved pill is gone
- **WHEN** the saved tab exists but the saved main Selector pill no longer exists
- **THEN** mbv SHALL select the first guaranteed pill in that tab's current presentation order
- **THEN** mbv SHALL restore the saved item only if it belongs to that pill's current list, otherwise selecting the first selectable item

#### Scenario: The saved tab is gone
- **WHEN** the saved tab no longer exists
- **THEN** mbv SHALL select the first guaranteed tab in current presentation order
- **THEN** it SHALL select that tab's first guaranteed pill and first selectable item

#### Scenario: The restored list is empty
- **WHEN** the restored tab and pill have no selectable library item
- **THEN** mbv SHALL restore no selected library item

### Requirement: Queue selection starts independently of launch-state restoration
Restoring Queue Panel focus SHALL NOT restore a Queue item selection. Queue selection SHALL initialize from the Queue component's normal current-content rule, independently of the saved Library launch location.

#### Scenario: Queue focus is restored
- **WHEN** a saved launch snapshot names Queue as the focused Panel
- **WHEN** the next TUI starts with Queue content
- **THEN** Queue SHALL receive Panel focus
- **THEN** its selected item SHALL come from normal Queue initialization rather than the prior TUI's Queue selection
