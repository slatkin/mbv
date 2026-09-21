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
When a settled grouped snapshot is replaced, the system SHALL preserve the current artist-root or album-leaf selection by stable identity when that node remains present. It SHALL preserve surviving artist expansion and multi-selection state. When the selected node survives, the viewport SHALL retain its prior screen row when projection bounds permit; otherwise it SHALL scroll only enough to keep the node visible and clamp at projection bounds. Artist roots SHALL remain stable grouping and action targets across the replacement.

#### Scenario: Selected album survives a replacement
- **WHEN** a replacement snapshot contains the album leaf selected in the prior snapshot
- **THEN** that album remains selected and remains visible after the replacement is committed

#### Scenario: Selected artist survives a replacement
- **WHEN** a replacement snapshot contains the artist root selected in the prior snapshot
- **THEN** that artist remains selected with its persistent expansion state preserved

#### Scenario: Selected album is absent from a replacement
- **WHEN** the selected artist or album node is not present in a replacement
- **THEN** the system selects a valid visible node using its normal fallback selection behavior

#### Scenario: Artist grouping survives a replacement
- **WHEN** a replacement snapshot is committed
- **THEN** its settled artist roots continue to group the visible album leaves with stable identity and expansion

#### Scenario: Artist header action follows the visible grouping
- **WHEN** the user invokes an artist-root action on a settled grouped view
- **THEN** the action operates on exactly that root's in-scope albums in the settled snapshot

### Requirement: Stable redraw behavior
For an unchanged settled grouped snapshot, repeated terminal redraws SHALL reuse its grouping and ordering without starting artist metadata resolution work.

#### Scenario: Repeated redraw without music data changes
- **WHEN** the terminal redraws a settled grouped view and its albums, grouping metadata, and selection have not changed
- **THEN** the displayed grouping remains identical and no additional artist lookup is initiated by the redraw

### Requirement: Album navigation remains visible across artist groups
Tree navigation in the grouped Music view SHALL keep the selected artist root or album leaf visible while crossing artist-group boundaries. Artist roots SHALL contribute to scroll geometry and SHALL be cursor targets; collapsed album leaves SHALL not contribute to the visible projection or hit geometry.

#### Scenario: Cursor crosses an artist boundary
- **WHEN** tree navigation moves selection from one artist group to an adjacent group
- **THEN** the destination visible node is selected
- **AND** the viewport adjusts as needed to keep it visible

#### Scenario: Artist is collapsed
- **WHEN** an expanded artist root is collapsed
- **THEN** its album leaves leave the visible projection
- **AND** viewport clamping keeps the focused root visible

### Requirement: Responsive grouped-view continuity

The non-Wide and Wide compositions SHALL consume the same settled grouped snapshot and tree owner. Changing composition SHALL NOT restart artist metadata resolution, publish a different grouping for the same snapshot, or replace the selected artist root or album leaf when it remains available.

#### Scenario: Grouped Music crosses the responsive breakpoint
- **WHEN** terminal resizing switches grouped Music between its non-Wide and Wide compositions
- **THEN** the same settled grouping, selected tree node, expansion state, and multi-selection remain in use
- **AND** the active tree viewport is clamped around that selection

#### Scenario: Responsive composition redraws
- **WHEN** either responsive composition redraws without a changed album snapshot
- **THEN** it reuses the existing settled grouping without starting artist metadata resolution work

### Requirement: Artist roots are focusable grouping targets
The grouped Music album view SHALL present focusable artist roots in the shallow tree. Artist roots SHALL receive keyboard and mouse selection, expansion, current-item scope, playback and queue actions, and context actions as specified by `grouped-music-tree-browser`. Album leaves SHALL remain independently selectable targets within each artist root. This SHALL NOT change canonical Group headings in any other list or in the artist track Workspace; those remain non-selectable visual labels.

#### Scenario: Keyboard navigation crosses an artist boundary
- **WHEN** the user moves through the grouped Music tree across an artist boundary
- **THEN** focus can land on the artist root and its visible album leaves

#### Scenario: Artist root is clicked
- **WHEN** the user clicks a painted artist root
- **THEN** that root receives focus and becomes the current tree action scope

#### Scenario: Grouped music action is invoked
- **WHEN** the user invokes a playback, queue, or context action with an artist root focused
- **THEN** the action resolves the root to its in-scope album leaves
- **AND** no artist identity crosses the effect boundary as a playable target

### Requirement: Music grouping metadata is warmed at startup

For a configured music library, the system SHALL begin resolving the artist
metadata used for album grouping in the background once its Service is
connected, without waiting for a grouped music view to be opened. Warm-up
SHALL NOT delay application startup or the availability of any other Service
or feature, and a warm-up failure SHALL leave ordinary grouped browsing
usable through the existing settle-and-fallback behavior.

#### Scenario: Warm-up begins after the Service connects

- **WHEN** mbv has started and the music library's Service becomes connected
- **THEN** the system begins resolving grouping artist metadata for the
  library's music grouping levels in the background, before any grouped
  music view is opened

#### Scenario: Grouped view opens from warmed metadata

- **WHEN** the user opens a grouped music album level whose background
  metadata resolution has already completed
- **THEN** the settled artist-grouped ordering is published without an
  organizing wait for artist lookups

#### Scenario: Warm-up is incomplete or fails

- **WHEN** the user opens a grouped music album level whose background
  metadata resolution has not completed or could not obtain metadata
- **THEN** the level settles through the existing organizing state and
  deterministic fallback within the grouping resolution window, and browsing
  remains available

#### Scenario: Warm-up does not gate startup

- **WHEN** background grouping-metadata resolution is still running or has
  failed
- **THEN** the TUI, other Services, and non-music browsing remain fully
  available and unaffected

### Requirement: Grouped music levels omit folders without contents

When a grouped music view lists a level's children, a folder item with no
child items SHALL NOT be enumerated as a row of that level. Items whose
child count is unknown SHALL be enumerated, and non-folder items SHALL
never be treated as empty. Background artist warm-up SHALL NOT spend
resolution work on folders that are not enumerated.

#### Scenario: Empty folder is not enumerated

- **WHEN** a grouped music level is listed and its children include a
  folder with no child items (for example a `Downloads` folder at the
  music library root)
- **THEN** that folder does not appear as a row of the level

#### Scenario: Unknown child count still enumerates

- **WHEN** a grouped music level is listed and a folder item's payload
  does not carry a child count
- **THEN** that folder is enumerated like any other folder

#### Scenario: Non-folder items are never treated as empty

- **WHEN** a grouped music level is listed and a non-folder item's payload
  carries a child count of zero
- **THEN** that item is still enumerated

#### Scenario: Warm-up skips empty folders

- **WHEN** background grouping-artist warm-up lists a music library's
  group-level children
- **THEN** folders with no child items receive no grouping-artist
  resolution work
