## ADDED Requirements

### Requirement: Artist-scoped selection block
In the music-group view, the system SHALL render one selection block around the complete focused artist group rather than around an individual album. The block SHALL render the resolved artist name once on its first content row, a pinned action-hint row second, and the artist's album region below it.

#### Scenario: Album target in a grouped view
- **WHEN** an album is focused in the music-group view
- **THEN** the selection frame encloses that album's artist header, hint row, and sibling albums
- **AND** the artist name appears only on the artist row

#### Scenario: Artist target in a grouped view
- **WHEN** the artist header is focused in the music-group view
- **THEN** the same artist-scoped block remains in place
- **AND** no separate artist-header-only block is rendered

#### Scenario: Marker crosses an artist boundary
- **WHEN** navigation moves the target from the last selectable row of one artist to the next artist
- **THEN** the selection block moves to enclose the newly targeted artist group

### Requirement: Action-target marker
The system SHALL reserve a fixed two-column gutter for every artist and album target row inside the selected group. The focused target SHALL render an AQUA `▌` marker and a bold white title in that gutter layout, and unfocused targets SHALL retain the same text alignment without a marker. The pinned hint text SHALL reflect whether the target is the artist or an album.

#### Scenario: Focus moves within an artist
- **WHEN** focus moves between the artist header and its albums
- **THEN** the marker and bold title move to the new target
- **AND** album title columns do not shift horizontally
- **AND** the hint row changes to the actions available for the new target type

### Requirement: Bounded inline album region
The selected artist block SHALL render every album belonging to the focused artist when the group has 12 or fewer albums. For larger groups, it SHALL render a derived 12-album inline window containing the focused album, and navigation SHALL shift that window without moving the outer artist block.

#### Scenario: Small discography is fully visible
- **WHEN** the artist-scoped block is rendered for the focused artist
- **THEN** every album entry for that artist is present in the block, in full

#### Scenario: Large discography scrolls inline
- **WHEN** the focused artist has more than 12 albums
- **THEN** the selected block renders a 12-album window containing the focused album
- **AND** moving focus at either window edge shifts the window to reveal the next album
- **AND** the outer artist block remains anchored

### Requirement: Stable outer viewport
The outer library viewport SHALL keep the selected artist block anchored while focus moves within that artist. For groups larger than 12 albums, the inline album window SHALL shift to reveal the focused album without scrolling the outer viewport through the block; expanded track tables retain their own internal cursor scrolling. The artist header SHALL not become sticky.

#### Scenario: Navigation within a fitting artist block
- **WHEN** focus moves between targets in one artist block and the target remains visible
- **THEN** the outer library viewport offset does not change

#### Scenario: Selected content exceeds the viewport
- **WHEN** expanded track content exceeds the viewport
- **THEN** the track table scrolls internally enough to reveal the target
- **AND** no sticky copy of the artist header is rendered

#### Scenario: Discography exceeds the inline window
- **WHEN** the focused artist has more than 12 albums and focus moves to an album outside the current window
- **THEN** the inline album window shifts to reveal that album
- **AND** the selected block's outer position does not change

### Requirement: Inline track expansion
When a focused album is expanded, the system SHALL append its loading state or track table below the album region inside the artist selection block. The sibling albums SHALL remain present, and the block height SHALL grow with the rendered track content.

#### Scenario: Album tracks are available
- **WHEN** the user expands a focused album whose tracks are loaded
- **THEN** the track table appears below the album region within the same frame
- **AND** the sibling album entries remain above it

#### Scenario: Track row is focused
- **WHEN** focus moves within an expanded album's track table
- **THEN** the artist-block marker remains on the expanded album
- **AND** the track table's own cursor identifies the focused track

#### Scenario: Album tracks are loading
- **WHEN** the user expands a focused album whose tracks are not loaded
- **THEN** a loading row appears below the album region within the same frame

### Requirement: Target-sensitive inline artwork
When inline images are enabled, the selected group SHALL show the artist collage while the artist row is targeted and the focused album cover while an album row is targeted. Artwork SHALL occupy a 12-row box anchored to the top of the block. The block SHALL retain enough continuation space to avoid cropping the art box for short discographies.

> Deferred: row-by-row width recalculation so text below the 12-row art box reclaims full width was considered and is intentionally out of scope here — see design.md's "Deferred" note. This change uses one constant narrowed width for the block, matching current behavior; rows below the art box keep the narrowed width rather than expanding.

#### Scenario: Artist row is targeted
- **WHEN** the artist row has the marker and inline images are enabled
- **THEN** the block renders the artist collage

#### Scenario: Album row is targeted
- **WHEN** an album row has the marker and inline images are enabled
- **THEN** the block renders that album's cover

#### Scenario: Expanded track row is focused
- **WHEN** the track-table cursor is active and inline images are enabled
- **THEN** the block marker remains on the expanded album
- **AND** the block continues to render that album's cover

### Requirement: Non-grouped album compatibility
Outside the music-group view, the system SHALL retain the existing per-album selection frame and SHALL not render a duplicated artist-name row inside it. Removing the duplicated row SHALL also apply to grouped rendering and SHALL not change album action semantics.

#### Scenario: Album selected in plain or search results
- **WHEN** an album is selected outside the music-group view
- **THEN** its existing per-album frame is rendered
- **AND** no separate album-artist row is inserted inside that frame

#### Scenario: Grouped album action target
- **WHEN** an album is the target inside an artist-scoped block
- **THEN** play, enqueue, shuffle, and track-expansion actions continue to target that album
