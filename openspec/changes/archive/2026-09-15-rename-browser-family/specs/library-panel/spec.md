## RENAMED Requirements

- FROM: `### Requirement: The Browser pane has one Selector row and one optional List controls row`
  TO: `### Requirement: The list pane has one Selector row and one optional List controls row`

## MODIFIED Requirements

### Requirement: The list pane has one Selector row and one optional List controls row

The list pane (the Wide panel's left pane, and the whole Narrow panel) SHALL present, top to bottom:
at most one Selector row (a single pill bar followed by the panel's spacer row), at most one List
controls row, and the list box. The Selector row carries destination browse selectors, including the
Feeds All / Played / Unplayed watched filter followed by its feed-group pills. The List controls row
carries secondary controls such as the Emby home-video item count. No destination SHALL render a
second pill bar.

#### Scenario: Feeds renders its selectors
- **WHEN** the Feeds destination renders with subscriptions available
- **THEN** its All / Played / Unplayed filter and feed-group pills render in the one Selector row
- **AND** no List controls row or second pill bar renders

#### Scenario: An Emby home-video library renders its count
- **WHEN** an Emby home-video library renders
- **THEN** its item count renders in the List controls row, not above or inside the Selector row

#### Scenario: Selector pill hit geometry
- **WHEN** the user clicks a pill in any destination's Selector row
- **THEN** the pill painted under the pointer in the latest frame is selected
