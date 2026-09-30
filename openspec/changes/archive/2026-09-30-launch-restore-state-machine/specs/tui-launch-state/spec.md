# Spec Delta

## MODIFIED Requirements

### Requirement: Launch restoration follows stable identities with ordered fallbacks
At startup, mbv SHALL restore the saved tab if that tab still exists. If it does not, mbv SHALL select the first guaranteed tab in presentation order. Within the restored tab, mbv SHALL restore the saved main Selector pill if that pill still exists; otherwise it SHALL select the first guaranteed pill in presentation order. Within the restored pill's list, mbv SHALL restore the saved library item if that item still exists and remains selectable; otherwise it SHALL select the first selectable item in presentation order. An empty list SHALL have no selected item.

If the restored tab presents no main Selector pills, mbv SHALL treat its unfiltered view as the restored scope and apply the item rule directly. Every fallback SHALL be resolved against current content, never by clamping or reusing a stale persisted index.

A saved Service tab SHALL be resolved only once the outcome of that Service's startup is known. If the Service's catalog arrived, the tab SHALL be restored when its library exists and otherwise fall back as above. If the Service is not configured, or its startup failed, the saved tab SHALL be treated as gone. Once that outcome is known, launch restoration SHALL NOT change the selected tab at any later time, including when the Service later connects.

The saved pill, item, and Panel focus SHALL be applied only to the tab that launch restoration selected. If a different tab is selected before they are applied, the remaining launch state SHALL be abandoned.

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
- **WHEN** a different tab becomes selected before the saved pill, item, and Panel focus are applied
- **THEN** mbv SHALL NOT apply them to the newly selected tab
