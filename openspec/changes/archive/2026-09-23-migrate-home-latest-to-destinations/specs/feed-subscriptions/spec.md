# Spec Delta

## ADDED Requirements

### Requirement: Feeds offers a Latest mode beside its existing group and watched selectors
When subscriptions exist, the Feeds tab SHALL offer a `Latest` pill alongside its existing subscription/group choices. Selecting Latest SHALL show the loaded combined entries newest-first regardless of the current watched filter or selected subscription, without changing either underlying choice. Leaving Latest through the selector SHALL restore the previously chosen group and watched filter. Pressing `w` on Latest SHALL leave Latest, restore the previous group, and cycle its previous watched filter once, as `w` does elsewhere in Feeds. The tab SHALL still fetch entries only on explicit `r` refresh, and a refresh SHALL update the Latest rows from that same loaded snapshot rather than starting a separate fetch.

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
- **WHEN** the user presses `r` on Latest
- **THEN** the Latest list updates from the refreshed entries
