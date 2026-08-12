# stable-music-library-grouping Specification

## Purpose

Provide a settled, artist-grouped music album view that opens predictably and remains visually stable while metadata is resolved in the background.
## Requirements
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

#### Scenario: Artist header action follows the visible grouping

- **WHEN** the user invokes an artist-header action on a settled grouped view
- **THEN** the action operates on exactly the albums shown under that header in the settled snapshot

### Requirement: Stable redraw behavior
For an unchanged settled grouped snapshot, repeated terminal redraws SHALL reuse its grouping and ordering without starting artist metadata resolution work.

#### Scenario: Repeated redraw without music data changes
- **WHEN** the terminal redraws a settled grouped view and its albums, grouping metadata, and selection have not changed
- **THEN** the displayed grouping remains identical and no additional artist lookup is initiated by the redraw

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

### Requirement: Responsive grouped-view continuity

The narrow hero-above-list composition and wide side-hero composition SHALL consume the same settled grouped snapshot and album selection. Changing composition SHALL NOT restart artist metadata resolution, publish a different grouping for the same snapshot, or replace the selected album when it remains available.

#### Scenario: Grouped Music crosses the responsive breakpoint
- **WHEN** terminal resizing switches grouped Music between its narrow and wide compositions
- **THEN** the same settled grouping and selected album remain in use and the active album viewport is clamped around that selection

#### Scenario: Responsive composition redraws
- **WHEN** either responsive composition redraws without a changed album snapshot
- **THEN** it reuses the existing settled grouping without starting artist metadata resolution work

