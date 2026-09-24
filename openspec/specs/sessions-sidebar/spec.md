# sessions-sidebar Specification

## Purpose

The F3 Sessions sidebar presents Emby Sessions and discovered Cast receivers as distinct, selectable playback targets with clear selection and connection cues.

## Requirements

### Requirement: F3 presents three-line targets through a flat list
The F3 Sessions sidebar SHALL show each Emby Session and Cast receiver as a three-line flat-list item, with items adjacent by default and the list's separator gap currently zero (no blank separator rows in F3). It SHALL preserve the existing target order, kind labels, Emby device/client/user/host and playback details, and Cast receiver name/address details; a Cast item without a third detail line SHALL leave that line blank. Existing loading, empty-state, footer, refresh, dismiss, and detach behavior SHALL remain available. Its mounted sidebar SHALL remain the sole focus and gesture boundary; the embedded list SHALL not become an independently mounted surface.

#### Scenario: Both discovery channels report the same device
- **WHEN** a device appears as an Emby Session and a Cast receiver
- **THEN** F3 shows two separately selectable three-line items bearing their distinct kind labels
- **AND** selecting one uses that item's existing control channel

#### Scenario: Only Cast targets are available
- **WHEN** the sidebar has Cast receivers but no Emby Sessions
- **THEN** each receiver still occupies a three-line item with a blank third content line
- **AND** navigation and activation work without a Session

### Requirement: Cursor selection and connection have separate visual meanings
The currently selected F3 target SHALL use the standard filled selected-row bar across its three content lines, never an accent cursor rail. A connected Emby Session or attached Cast target SHALL be marked by an aqua `✚` badge, never by a connected-state filled row. The badge SHALL remain visible whether its item is selected or not; its colour SHALL remain aqua over the selected-row bar. Zebra striping SHALL alternate per target item, not per painted line; the blank separator SHALL remain unstriped and unselected.

#### Scenario: Connected target is not selected
- **WHEN** the selected cursor is on one item and a different target is connected
- **THEN** the cursor item alone carries the selected-row bar
- **AND** the connected item carries the aqua `✚` without a connection fill

#### Scenario: Connected target is selected
- **WHEN** the cursor selects the connected target
- **THEN** that target carries both the selected-row bar and the aqua `✚`
- **AND** the separator line below it carries neither fill

### Requirement: F3 activates the selected stable target
F3 SHALL preserve selection by kind-qualified stable target identity across independent Emby Session and Cast discovery refreshes. Enter or a click on the already selected item SHALL request connection to the selected identity, not to a row number; a first click on a different item SHALL select it without connecting. The shell SHALL resolve the identity against its current target snapshot and SHALL do nothing when it has disappeared, rather than connect a replacement at the old position. Pointer resolution SHALL use only the latest painted item geometry; clicks on blank separators SHALL not activate a target.

#### Scenario: A refresh reorders targets before activation
- **WHEN** the displayed targets change order while the selected target still exists
- **THEN** the selection remains on that target
- **AND** Enter requests that same target regardless of its new position

#### Scenario: Selected target disappears after an activation request
- **WHEN** the selected target disappears before the shell resolves its connection request
- **THEN** the request connects no target
- **AND** it does not connect the item now occupying the former index

#### Scenario: Pointer lands on a separator
- **WHEN** a pointer click lands on the gap between two target items
- **THEN** it neither changes selection nor connects either target
