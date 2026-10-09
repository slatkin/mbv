# Spec Delta

## MODIFIED Requirements

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
