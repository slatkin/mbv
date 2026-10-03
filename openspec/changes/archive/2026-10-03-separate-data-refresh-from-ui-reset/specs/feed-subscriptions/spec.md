# Spec Delta

## MODIFIED Requirements

### Requirement: Feed entries refresh only on explicit user action

Feed entries SHALL be fetched and their stored playback state applied only when the user requests refresh with the existing `r` action on the Feeds tab or the global F5 data-refresh action while Feeds is the selected browse destination. F5 SHALL refresh Feeds regardless of Panel focus or Library visibility and SHALL NOT refresh Queue. Each successful refresh SHALL read stored state once for the whole subscription rather than once per entry. The client SHALL NOT auto-refresh on tab open, on a timer, or when the watched filter changes. Refresh SHALL preserve the current group, watched filter, Latest mode and still-valid selected entry under the global data-refresh contract.

#### Scenario: Manual refresh
- **WHEN** the user presses `r` on the Feeds tab
- **THEN** the subscriptions SHALL be re-fetched and the entry lists updated with currently stored state

#### Scenario: F5 while Queue holds focus
- **WHEN** Feeds is the selected browse destination, Queue holds focus and the user presses F5
- **THEN** the subscriptions SHALL be re-fetched through the existing feed refresh
- **AND** Queue SHALL NOT be refreshed and the selected Feeds context SHALL remain intact where valid

#### Scenario: Stored state is applied on refresh
- **WHEN** a refresh returns an entry that has stored playback state
- **THEN** the entry SHALL show that position and played state without a per-entry state read

#### Scenario: State changes on another machine
- **WHEN** a matching feed entry gains a new position or played state on another machine
- **THEN** this machine's stored state SHALL remain unchanged
- **AND** pressing `r` or F5 SHALL NOT make the other machine's state visible here

#### Scenario: No automatic refresh
- **WHEN** the Feeds tab is opened or re-opened without an explicit refresh action
- **THEN** the previously fetched entries SHALL be shown without an automatic feed or state read

#### Scenario: Changing the watched filter
- **WHEN** the user changes the watched filter without an explicit refresh action
- **THEN** the client SHALL filter the state already loaded in the Feeds tab and SHALL NOT perform a state read or write

### Requirement: Feeds offers a Latest mode beside its existing group and watched selectors

When subscriptions exist, the Feeds tab SHALL offer a `Latest` pill alongside its existing subscription/group choices. Selecting Latest SHALL show the loaded combined entries newest-first regardless of the current watched filter or selected subscription, without changing either underlying choice. Leaving Latest through the selector SHALL restore the previously chosen group and watched filter. Pressing `w` on Latest SHALL leave Latest, restore the previous group, and cycle its previous watched filter once, as `w` does elsewhere in Feeds. The tab SHALL still fetch entries only on explicit `r` or selected-destination F5 refresh, and a refresh SHALL update the Latest rows from that same loaded snapshot rather than starting a separate fetch. Refresh SHALL retain the selected Latest mode and its existing acknowledgement.

#### Scenario: Latest ignores current grouping and filter
- **WHEN** a subscription group and Unplayed filter are selected and the user selects Latest
- **THEN** the Latest list includes loaded entries from all subscriptions, including played entries, in newest-first order
- **WHEN** the user leaves Latest
- **THEN** the previous group and Unplayed filter are restored

#### Scenario: Watched shortcut on Latest
- **WHEN** a subscription group and Unplayed filter were selected before entering Latest
- **WHEN** the user presses `w` on Latest
- **THEN** Latest closes, that subscription group is selected, and its watched filter advances from Unplayed to All

#### Scenario: Refresh behavior does not change
- **WHEN** the Feeds tab opens or the user selects Latest
- **THEN** no feed fetch occurs solely because of the selection
- **WHEN** the user presses `r` on Latest or invokes F5 while Feeds Latest is selected
- **THEN** Latest remains selected and its rows update from the refreshed entries
- **AND** its acknowledgement and prior group/filter choices remain intact
