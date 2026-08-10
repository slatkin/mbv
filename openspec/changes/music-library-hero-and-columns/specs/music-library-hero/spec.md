## Purpose

Provides a responsive grouped Music layout that preserves the current narrow hero and presents a persistent album-and-track workspace beside a one-column album browser at wide widths.

## ADDED Requirements

### Requirement: Grouped Music uses responsive compositions

The grouped Music album view SHALL use the existing shared wide-layout breakpoint to choose its composition. Below the breakpoint it SHALL preserve the current pills-above, hero-above-list, one-column layout. At or above the breakpoint it SHALL render a Music-specific horizontal split with album detail and tracks on the left and album browsing on the right. Other library types SHALL NOT change composition because of this requirement.

#### Scenario: Grouped Music below the breakpoint
- **WHEN** the grouped Music content area is narrower than the shared wide-layout breakpoint
- **THEN** group pills span the content width, the album hero renders above the list, and albums render one per row as they do before this change

#### Scenario: Grouped Music at the breakpoint
- **WHEN** the grouped Music content area reaches the shared wide-layout breakpoint
- **THEN** it switches to the horizontal Music composition

#### Scenario: Non-Music library at wide width
- **WHEN** any non-Music library is rendered at or above the breakpoint
- **THEN** its existing layout remains unchanged

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

### Requirement: Wide Music focus uses the Home visual language

The wide grouped Music view SHALL use the same focused and unfocused surface treatment as the Home wide split. During album browsing the right rail SHALL be the focused green surface and the left track workspace SHALL use the playback-panel surface. During track selection those treatments SHALL reverse. When the Library panel itself is unfocused, both Music panes SHALL use the normal dimmed library treatment.

#### Scenario: Album browser has focus
- **WHEN** track selection is inactive and the Library panel is focused
- **THEN** the right rail has Home's focused treatment and the left workspace remains a readable preview

#### Scenario: Track selection has focus
- **WHEN** track selection is active and the Library panel is focused
- **THEN** the left track workspace has Home's focused treatment and the right rail is visibly dimmed while retaining the selected album marker

#### Scenario: Queue has focus
- **WHEN** the Queue panel has focus
- **THEN** both Music panes use the existing unfocused library styling

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
