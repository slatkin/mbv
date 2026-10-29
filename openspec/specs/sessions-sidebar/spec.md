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

### Requirement: Cursor selection and connection are indicated separately

The selected F3 target and a connected target SHALL be indicated separately: connection SHALL NOT be shown by marking the connected row as selected. A connected Emby Session or attached Cast target SHALL keep its connection mark whether its item is selected or not.

#### Scenario: Connected target is not selected

- **WHEN** the selected cursor is on one item and a different target is connected
- **THEN** the cursor item alone is marked selected
- **AND** the connected item is marked connected without selection marking

#### Scenario: Connected target is selected

- **WHEN** the cursor selects the connected target
- **THEN** that target carries both marks
- **AND** the separator line below it carries neither

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

### Requirement: F3 offers Hand off and Take on a peer target

F3 SHALL offer Hand off and Take as separate actions on the selected target when that target has a direct Player-owner endpoint other than the local Owner and the home of the Client is the local Owner. Each action SHALL name the selected target by its stable identity, as activation does. F3 SHALL NOT offer them on a target without a direct endpoint; activation of such a target SHALL stay unchanged.

#### Scenario: Peer target selected

- **WHEN** the selected F3 target is an mbv or mbvd peer with a direct endpoint, the local Owner plays, and the row shows the peer playing
- **THEN** Hand off and Take SHALL be available for it
- **AND** the footer SHALL list them

#### Scenario: Emby Session without a direct endpoint selected

- **WHEN** the selected F3 target has no direct endpoint
- **THEN** Hand off and Take SHALL NOT be available
- **AND** activation SHALL keep its current behavior

#### Scenario: Selected peer disappears before the request resolves

- **WHEN** the selected peer disappears before the shell resolves a Hand off or Take request
- **THEN** the request SHALL act on no target

### Requirement: Hand off and Take are offered only while the source plays

F3 SHALL offer Hand off only while the local Owner plays, and Take only while the selected row shows the peer playing.

#### Scenario: Only the local Owner plays

- **WHEN** the selected F3 target is a peer with a direct endpoint, the local Owner plays, and the row shows the peer idle
- **THEN** Hand off SHALL be available
- **AND** Take SHALL NOT be available
