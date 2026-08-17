# music-library-hero Specification

## Purpose
Provides a responsive grouped Music layout that preserves the current narrow hero and presents a persistent album-and-track workspace beside a one-column album browser at wide widths.
## Requirements
### Requirement: Grouped Music uses responsive compositions

The grouped Music album view SHALL use the hero-on-left arrangement. Below the shared breakpoint it
SHALL fall back to hero-on-top with a single list column. At or above the breakpoint it SHALL render
hero-on-left, with album detail and tracks in the hero pane and album browsing in the list pane. The
grouped Music view SHALL NOT evaluate the breakpoint itself. Screens assigned hero-on-top SHALL NOT
change arrangement because of this requirement.

#### Scenario: Grouped Music below the breakpoint

- **WHEN** the grouped Music content area is narrower than the shared breakpoint
- **THEN** group pills span the content width, the album hero renders above the list, and albums
  render one per row

#### Scenario: Grouped Music at the breakpoint

- **WHEN** the grouped Music content area reaches the shared breakpoint
- **THEN** it renders the hero-on-left arrangement

#### Scenario: Non-Music library at wide width

- **WHEN** a library assigned hero-on-top is rendered at or above the breakpoint
- **THEN** it renders hero-on-top with a two-column list and does not adopt hero-on-left

### Requirement: Wide left pane persistently shows album detail and tracks

The wide grouped Music left pane SHALL show the selected album's title, metadata, large artwork, and track list. The track list SHALL remain visible whether album browsing or track selection has focus. Artwork SHALL yield vertical space before the track list disappears, and a present track list SHALL retain a visible track viewport whenever the content height can fit one.

#### Scenario: Album browsing is active
- **WHEN** an album is selected in the wide right rail and track selection is inactive
- **THEN** the left pane shows that album's large hero treatment and a readable, non-cursor track preview

#### Scenario: Selected album changes
- **WHEN** the album cursor moves to another album
- **THEN** the left title, metadata, artwork, loading state, and tracks update to the newly selected album without showing tracks from the previous album under the new title

#### Scenario: Album tracks are loading
- **WHEN** the selected wide-mode album's tracks are not cached yet
- **THEN** the left track region shows a loading state and replaces it with that album's tracks when available

#### Scenario: Content height is constrained
- **WHEN** the wide layout has limited vertical space
- **THEN** the artwork shrinks before the persistent track region is removed

### Requirement: Wide album browser occupies the right rail

In the wide grouped Music composition, the music-group pills SHALL render at the top of the right rail and the artist-grouped album browser SHALL render below them. Albums SHALL render one per row regardless of available right-rail width. Artist headers SHALL span the rail as non-selectable grouping labels.

#### Scenario: Wide grouped Music renders
- **WHEN** grouped Music uses the horizontal composition
- **THEN** the right rail shows group pills followed by a one-column artist-grouped album list

#### Scenario: Artist group contains several albums
- **WHEN** an artist group is visible in the wide right rail
- **THEN** each album occupies its own row beneath the artist header

#### Scenario: Group pill changes selection
- **WHEN** the user selects another music-group pill in wide mode
- **THEN** the right rail loads that group's albums, returns focus to album browsing, and the left pane follows the resulting album selection

### Requirement: Hero-on-left uses one focus treatment

The hero-on-left arrangement SHALL apply one focused and unfocused surface treatment to every screen
that uses it, including grouped Music and Home. During album browsing the list pane SHALL carry the
focused treatment and the hero pane SHALL carry the resting treatment. During track selection those
treatments SHALL reverse. When the Library panel itself is unfocused, both panes SHALL use the
unfocused treatment. Grouped Music SHALL NOT define these colours itself.

#### Scenario: Album browser has focus

- **WHEN** track selection is inactive and the Library panel is focused
- **THEN** the list pane has the arrangement's focused treatment and the hero pane remains a
  readable preview

#### Scenario: Track selection has focus

- **WHEN** track selection is active and the Library panel is focused
- **THEN** the hero pane has the arrangement's focused treatment and the list pane is visibly dimmed
  while retaining the selected album marker

#### Scenario: Queue has focus

- **WHEN** the Queue panel has focus
- **THEN** both Music panes use the arrangement's unfocused treatment

#### Scenario: The focused treatment is changed

- **WHEN** the hero-on-left focused treatment is changed in its single definition
- **THEN** grouped Music, Home, and audiobooks all render the change

### Requirement: Wide track selection preserves keyboard behavior

Enter on the selected wide-mode album SHALL activate the track cursor in the already-visible left track list. Existing track movement, playback, current-item scope, and Escape or Backspace exit behavior SHALL remain unchanged. Entering and exiting track selection SHALL NOT change the wide layout geometry.

#### Scenario: Enter track selection
- **WHEN** the user presses Enter on the selected album in wide mode
- **THEN** the track cursor activates at the existing initial position and visual focus shifts left without moving either pane

#### Scenario: Play focused track
- **WHEN** the user presses Enter with a track focused
- **THEN** playback starts from that track and track selection remains active

#### Scenario: Exit track selection
- **WHEN** the user presses Escape or Backspace during wide track selection
- **THEN** the track cursor clears, album browsing regains visual focus, and the persistent track preview remains visible

### Requirement: Wide tracks support direct mouse interaction

Each visible wide-mode track SHALL have a logical mouse target covering all of its wrapped physical rows. A single click SHALL select that track and activate track selection. A double-click SHALL select and play that track. Clicking an album or music-group pill SHALL clear track selection and return focus to the right rail. Artwork and blank hero space SHALL NOT activate track selection or playback.

#### Scenario: Click a visible track
- **WHEN** the user single-clicks any visible physical row belonging to a track
- **THEN** that logical track becomes selected and visual focus shifts left

#### Scenario: Double-click a visible track
- **WHEN** the user double-clicks any visible physical row belonging to a track
- **THEN** that track becomes selected and playback starts from it

#### Scenario: Click an album while tracks have focus
- **WHEN** the user single-clicks an album in the right rail during track selection
- **THEN** track selection clears, that album becomes selected, and visual focus returns right

#### Scenario: Click artwork
- **WHEN** the user clicks album artwork or blank space in the wide left hero
- **THEN** no track is selected and no playback action is invoked

