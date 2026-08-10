## ADDED Requirements

### Requirement: Artist headers are non-selectable grouping labels
The grouped music album view SHALL render artist headers as visual grouping labels and SHALL exclude them from keyboard and mouse selection, current-item scope, and playback or queue actions. Album rows SHALL remain the selectable targets within each artist group.

#### Scenario: Keyboard navigation crosses an artist boundary
- **WHEN** the user moves the album cursor across an artist-group boundary
- **THEN** selection moves between album rows without landing on the artist header

#### Scenario: Artist header is clicked
- **WHEN** the user clicks an artist header
- **THEN** the current album selection and action scope remain unchanged

#### Scenario: Grouped music action is invoked
- **WHEN** the user invokes an item, playback, queue, or context-menu action in the grouped music album view without track selection active
- **THEN** the action targets the selected album rather than an artist header

### Requirement: Album navigation remains visible across artist groups
Keyboard album navigation in the grouped music view SHALL keep the selected album visible while crossing artist-group boundaries. Visual artist-header rows SHALL contribute to scroll geometry without becoming cursor targets.

#### Scenario: Cursor crosses an artist boundary
- **WHEN** album navigation moves selection from one artist group to an adjacent group
- **THEN** the destination album is selected and the viewport adjusts as needed to keep it visible

## MODIFIED Requirements

### Requirement: Grouped-view continuity
When a settled grouped snapshot is replaced, the system SHALL preserve the current album selection by stable album identity when that album remains present. It SHALL retain the closest practical viewport position around that selection. Artist headers SHALL remain stable visual labels and SHALL NOT become selection or action targets.

#### Scenario: Selected album survives a replacement
- **WHEN** a replacement snapshot contains the album selected in the prior snapshot
- **THEN** that album remains selected and remains visible after the replacement is committed

#### Scenario: Selected album is absent from a replacement
- **WHEN** the selected album is not present in a replacement
- **THEN** the system selects a valid album using its normal fallback selection behavior

#### Scenario: Artist grouping survives a replacement
- **WHEN** a replacement snapshot is committed
- **THEN** its settled artist headers continue to group the visible album rows without receiving selection or action focus
