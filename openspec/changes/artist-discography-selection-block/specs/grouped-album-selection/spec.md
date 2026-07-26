## ADDED Requirements

### Requirement: Artist-scoped selection block
In the music-group view, the system SHALL render one selection block around the complete focused artist group rather than around an individual album. The block SHALL render the resolved artist name once on its first content row, a pinned action-hint row second, and the artist's album region below it.

#### Scenario: Album target in a grouped view
- **WHEN** an album is focused in the music-group view
- **THEN** the selection frame encloses that album's artist header, hint row, and visible sibling albums
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

### Requirement: Bounded album region
The selected artist block SHALL allocate at most eight physical terminal rows to album entries. Wrapped album titles SHALL consume rows from that limit, so the region can contain fewer than eight albums. The system SHALL derive a canonical visible window from the focused target without persistent inner-scroll state, keep the focused album visible, and advance by the minimum number of rendered rows needed to reveal each newly focused complete album.

#### Scenario: Discography fits in the region
- **WHEN** all album entries for the selected artist occupy eight or fewer rendered rows
- **THEN** every album is visible and no album entry is clipped

#### Scenario: Focus advances beyond the lower edge
- **WHEN** the focused album is at the lower edge of an overflowing album region and focus moves to the next album
- **THEN** the region advances by the minimum rendered rows needed to show the new album
- **AND** the new focused album remains at the lower edge where possible

#### Scenario: Wrapped titles consume the row budget
- **WHEN** one or more album titles wrap across multiple terminal rows
- **THEN** each wrapped row counts toward the eight-row limit
- **AND** the window shifts by enough rows to keep the newly focused album complete
- **AND** only complete neighboring album entries that fit with the focused album are included

#### Scenario: Focused album exceeds the row budget
- **WHEN** the focused album title wraps to more than eight terminal rows
- **THEN** the region is dedicated to that album
- **AND** its first eight wrapped lines are visible, beginning with the marker-bearing first line
- **AND** wrapped lines after the eighth are clipped

### Requirement: Album overflow feedback
When an artist has albums outside the visible album window, the pinned hint row SHALL show the one-based visible album range and total album count in the form `first-last/total`. The range SHALL update from the derived window as focus moves and SHALL be absent when the full discography is visible.

#### Scenario: Discography overflows
- **WHEN** albums 3 through 8 of a 20-album artist are visible
- **THEN** the hint row displays `3-8/20`

#### Scenario: Entire discography is visible
- **WHEN** every album for the selected artist is in the visible window
- **THEN** the hint row does not display an overflow range

### Requirement: Stable outer viewport
The outer library viewport SHALL keep the selected artist block anchored while focus moves within that artist whenever the target remains visible. If the selected block or expanded track target cannot fit in the viewport, the outer viewport SHALL follow the active cursor sufficiently to keep the target visible; the artist header SHALL not become sticky.

#### Scenario: Navigation within a fitting artist block
- **WHEN** focus moves between targets in one artist block and the target remains visible
- **THEN** the outer library viewport offset does not change

#### Scenario: Selected content exceeds the viewport
- **WHEN** expanded content or terminal height places the active target outside the viewport
- **THEN** the outer viewport scrolls enough to reveal the target
- **AND** no sticky copy of the artist header is rendered

### Requirement: Inline track expansion
When a focused album is expanded, the system SHALL append its loading state or track table below the bounded album region inside the artist selection block. The visible sibling albums SHALL remain present, and the block height SHALL grow with the rendered track content.

#### Scenario: Album tracks are available
- **WHEN** the user expands a focused album whose tracks are loaded
- **THEN** the track table appears below the album region within the same frame
- **AND** the visible sibling album entries remain above it

#### Scenario: Track row is focused
- **WHEN** focus moves within an expanded album's track table
- **THEN** the artist-block marker remains on the expanded album
- **AND** the track table's own cursor identifies the focused track

#### Scenario: Album tracks are loading
- **WHEN** the user expands a focused album whose tracks are not loaded
- **THEN** a loading row appears below the album region within the same frame

### Requirement: Target-sensitive inline artwork
When inline images are enabled, the selected group SHALL show the artist collage while the artist row is targeted and the focused album cover while an album row is targeted. Artwork SHALL occupy a 12-row box anchored to the top of the block; only text rows that vertically overlap that box SHALL use the narrowed wrap width, and rows below the box SHALL use the full content width. The block SHALL retain enough continuation space to avoid cropping the art box for short discographies.

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

#### Scenario: Text continues below artwork
- **WHEN** an album entry begins or wraps below the 12-row artwork box
- **THEN** those non-overlapping rendered rows use the full block content width

### Requirement: Non-grouped album compatibility
Outside the music-group view, the system SHALL retain the existing per-album selection frame and SHALL not render a duplicated artist-name row inside it. Removing the duplicated row SHALL also apply to grouped rendering and SHALL not change album action semantics.

#### Scenario: Album selected in plain or search results
- **WHEN** an album is selected outside the music-group view
- **THEN** its existing per-album frame is rendered
- **AND** no separate album-artist row is inserted inside that frame

#### Scenario: Grouped album action target
- **WHEN** an album is the target inside an artist-scoped block
- **THEN** play, enqueue, shuffle, and track-expansion actions continue to target that album
