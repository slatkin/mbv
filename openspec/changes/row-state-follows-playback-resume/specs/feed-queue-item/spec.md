# Spec Delta

## ADDED Requirements

### Requirement: Mark Played starts a feed entry over

Mark Played on a feed entry SHALL store played as true and position as zero, as playback completion does. This SHALL apply to a single row, a multi-selection, and the played toggle on a Home `Recently added` row. Mark Unplayed SHALL store played as false and SHALL keep the entry's position. The loaded Feeds list SHALL show the stored state at once, without a refetch.

#### Scenario: Mark Played on a half-played entry

- **WHEN** the user chooses Mark Played on a feed entry stored at 40% of its runtime
- **THEN** mbv SHALL store played as true and position as zero
- **AND** the entry's Feeds row SHALL show played state without a refetch
- **AND** playing the entry SHALL start from the beginning

#### Scenario: Mark Played on a multi-selection

- **WHEN** three feed entries with saved positions are selected and the user chooses Mark Played
- **THEN** mbv SHALL store played as true and position as zero for each of them

#### Scenario: Mark Unplayed keeps the position

- **WHEN** the user chooses Mark Unplayed on a played feed entry
- **THEN** mbv SHALL store played as false and SHALL keep the stored position
- **AND** the entry's Feeds row SHALL show unplayed state without a refetch

### Requirement: Stored played feed entries keep no position

The feed-entry store SHALL never hold a played row with a positive position. When the store loads a played row with a positive position, written before this rule, it SHALL set that row's position to zero and SHALL save the store once.

#### Scenario: Old marked row loads

- **WHEN** the store file holds a row with played set to true and a position at 40% of runtime
- **THEN** after the store loads, that row SHALL hold played as true and position as zero
- **AND** the store file SHALL be saved once with the corrected row

#### Scenario: Nothing to correct

- **WHEN** the store file holds no played row with a positive position
- **THEN** loading the store SHALL NOT write the store file
