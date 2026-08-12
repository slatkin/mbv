## Purpose

Provide a settled, artist-grouped music album view that opens predictably and remains visually stable while metadata is resolved in the background.

## ADDED Requirements

### Requirement: Settled initial music grouping
For a configured music library at its album level, the system SHALL publish artist headers and artist-sorted album rows only after every album in the current loaded snapshot has a terminal grouping identity. A terminal identity is either resolved artist metadata or the deterministic fallback used when metadata is unavailable.

#### Scenario: Initial artist data is incomplete
- **WHEN** an album snapshot first opens with one or more unresolved artist identities
- **THEN** the system shows an organizing state instead of progressively changing artist-grouped rows

#### Scenario: Initial snapshot becomes settled
- **WHEN** all albums in the initial snapshot have terminal grouping identities
- **THEN** the system publishes one artist-grouped ordering for that snapshot

#### Scenario: Artist lookup cannot supply metadata
- **WHEN** artist metadata cannot be obtained within the grouping resolution window
- **THEN** the system assigns the affected albums their deterministic fallback identities and publishes the settled grouping without waiting indefinitely

### Requirement: Stable grouped snapshot revisions
Once a grouped album snapshot is visible, individual metadata results SHALL NOT reorder its displayed artist headers or albums. A changed loaded album set SHALL be prepared as a replacement snapshot and committed atomically only after that replacement has settled.

#### Scenario: Individual artist results arrive after publication
- **WHEN** a late artist metadata result arrives for an album in the visible settled snapshot
- **THEN** the currently displayed artist grouping and order remain unchanged

#### Scenario: A later page changes the loaded album set
- **WHEN** newly loaded albums create a replacement snapshot while a settled snapshot is visible
- **THEN** the system keeps the settled snapshot visible until the replacement snapshot is ready and then replaces it in one update

#### Scenario: Obsolete results arrive after navigation
- **WHEN** artist metadata results belong to a snapshot that is no longer current because the user changed groups or navigated away
- **THEN** those results do not alter the current grouped display

### Requirement: Grouped-view continuity
When a settled grouped snapshot is replaced, the system SHALL preserve the current album selection by stable album identity when that album remains present. It SHALL retain the closest practical viewport position around that selection and SHALL ensure artist-header actions use the same group membership that is visible to the user.

#### Scenario: Selected album survives a replacement
- **WHEN** a replacement snapshot contains the album selected in the prior snapshot
- **THEN** that album remains selected and remains visible after the replacement is committed

#### Scenario: Selected album is absent from a replacement
- **WHEN** the selected album is not present in a replacement snapshot
- **THEN** the system selects a valid album using its normal fallback selection behavior

#### Scenario: Artist header action follows the visible grouping
- **WHEN** the user invokes an artist-header action on a settled grouped view
- **THEN** the action operates on exactly the albums shown under that header in the settled snapshot

### Requirement: Stable redraw behavior
For an unchanged settled grouped snapshot, repeated terminal redraws SHALL reuse its grouping and ordering without starting artist metadata resolution work.

#### Scenario: Repeated redraw without music data changes
- **WHEN** the terminal redraws a settled grouped view and its albums, grouping metadata, and selection have not changed
- **THEN** the displayed grouping remains identical and no additional artist lookup is initiated by the redraw
