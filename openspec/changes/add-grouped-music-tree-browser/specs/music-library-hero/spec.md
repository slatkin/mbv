## MODIFIED Requirements

### Requirement: Grouped Music uses responsive compositions

The grouped Music tree view SHALL use Wide hero when it meets the shared wide geometry conditions. Its right pane SHALL contain detail and a track Workspace for the selected artist root or album leaf, and its left rail SHALL contain the single-column tree browser. Otherwise an album leaf SHALL open its Library Hero overlay through its existing Enter activation, while Right on an already expanded artist root SHALL open the artist Library Hero overlay and focus its Workspace. Grouped Music SHALL NOT evaluate the breakpoint or minimum-height guard itself and SHALL NOT use a separate fallback.

Album-leaf detail SHALL retain the album title, metadata, album art, and album-track Workspace. Artist-root detail SHALL show artist artwork when stable Service identity and artwork are available, artist name, in-scope album count and year span, and an artist-track Workspace grouped by album. Fallback artist roots with no Service artist identity SHALL use the existing no-artwork presentation.

#### Scenario: Grouped Music below the breakpoint
- **WHEN** grouped Music does not meet the shared wide geometry conditions
- **THEN** group pills span the content width and the tree occupies the browser list slot
- **AND** Enter on an album leaf or Right on an already expanded artist root opens the Library Hero overlay with the corresponding Workspace

#### Scenario: Grouped Music at the breakpoint
- **WHEN** grouped Music meets the shared wide geometry conditions
- **THEN** it renders Wide hero with the selected artist or album detail and tracks in the right pane

#### Scenario: Grouped Music lacks sufficient height
- **WHEN** grouped Music meets the width breakpoint but fails the existing minimum-height guard
- **THEN** it uses the established non-Wide Library panel composition
- **AND** it does not pin detail above the browser or introduce a separate fallback

#### Scenario: Non-Music library at wide width
- **WHEN** another hero-bearing library meets the shared wide geometry conditions
- **THEN** it continues to render Wide hero with its one-column browser

### Requirement: Wide right pane persistently shows album detail and tracks

The wide grouped Music right pane SHALL show detail and tracks for the selected tree node. An album leaf SHALL show that album's title, metadata, artwork, and track list. An artist root SHALL show that artist's artwork when available, name, in-scope album count and year span, and all in-scope tracks grouped by album. The Workspace SHALL remain visible whether tree browsing or track selection has focus. Artwork SHALL yield vertical space before the Workspace disappears, and a present Workspace SHALL retain a visible track viewport whenever the content height can fit one.

#### Scenario: Album browsing is active
- **WHEN** an album leaf is selected in the wide tree and track selection is inactive
- **THEN** the right pane shows that album's large hero treatment and a readable, non-cursor track preview

#### Scenario: Artist root is focused
- **WHEN** an artist root is selected in the wide tree
- **THEN** the right pane immediately shows its text summary and artwork state
- **AND** its Workspace shows or loads only tracks from the root's in-scope albums

#### Scenario: Selected album changes
- **WHEN** the selected album leaf changes
- **THEN** the right-pane title, metadata, artwork, loading state, and tracks follow the newly selected album
- **AND** tracks from the prior album never paint under the new title

#### Scenario: Album tracks are loading
- **WHEN** the selected album's tracks are not cached yet
- **THEN** the Workspace shows a loading state and replaces it only with tracks for that album

#### Scenario: Content height is constrained
- **WHEN** the wide layout has limited vertical space
- **THEN** the artwork shrinks before the persistent track region is removed

### Requirement: Wide album browser occupies the left rail

In the wide grouped Music composition, the music-group pills SHALL render at the top of the left rail and the artist/album tree browser SHALL render below them. Artist roots and album leaves SHALL render one per row regardless of available left-rail width. Artist roots SHALL span the rail as focusable expandable nodes; album leaves SHALL show their hierarchy beneath them.

#### Scenario: Wide grouped Music renders
- **WHEN** grouped Music uses the horizontal composition
- **THEN** the left rail shows group pills followed by the one-column artist/album tree

#### Scenario: Artist group contains several albums
- **WHEN** an expanded artist root is visible in the wide left rail
- **THEN** each album occupies its own indented leaf row beneath that root

#### Scenario: Group pill changes selection
- **WHEN** the user selects another music-group pill in wide mode
- **THEN** the left rail loads that group's settled tree and returns focus to tree browsing
- **AND** the right pane follows the resulting selected node

### Requirement: Wide hero uses one focus treatment

The Wide hero arrangement SHALL apply one focused and unfocused surface treatment to every screen that uses it, including grouped Music and Home. During tree browsing the browser pane SHALL carry the focused treatment and the hero pane SHALL carry the resting treatment. During track selection those treatments SHALL reverse. When the Library panel itself is unfocused, both panes SHALL use the unfocused treatment. Grouped Music SHALL NOT define these colours itself.

#### Scenario: Album browser has focus
- **WHEN** track selection is inactive and the Library panel is focused
- **THEN** the tree pane has the arrangement's focused treatment and the hero pane remains a readable preview

#### Scenario: Track selection has focus
- **WHEN** track selection is active and the Library panel is focused
- **THEN** the hero pane has the arrangement's focused treatment and the tree pane is visibly dimmed while retaining its selected-node treatment

#### Scenario: Queue has focus
- **WHEN** the Queue panel has focus
- **THEN** both Music panes use the arrangement's unfocused treatment

#### Scenario: The focused treatment is changed
- **WHEN** the Wide hero focused treatment is changed in its single definition
- **THEN** grouped Music, Home, and audiobooks all render the change

### Requirement: Wide track selection preserves keyboard behavior

Enter on a selected wide-mode album leaf SHALL activate the track cursor in the already-visible album-track Workspace. Enter on a selected artist root SHALL toggle expansion and SHALL NOT enter its track Workspace. Right on a collapsed artist root SHALL expand it; Right on an already expanded artist root SHALL activate the cursor in its artist-track Workspace. Once either track Workspace has focus, existing track movement, playback, current-item scope, and Escape or Backspace exit behavior SHALL remain unchanged. Entering and exiting track selection SHALL NOT change the wide layout geometry.

#### Scenario: Enter track selection
- **WHEN** the user presses Enter on a selected album leaf in wide mode
- **THEN** the track cursor activates at the existing initial position and visual focus shifts right without moving either pane

#### Scenario: Enter toggles an artist root
- **WHEN** the user presses Enter on a selected artist root
- **THEN** that root toggles expansion
- **AND** track selection remains inactive

#### Scenario: Right enters an expanded artist Workspace
- **WHEN** the user presses Right on a selected artist root that is already expanded
- **THEN** the artist-track Workspace cursor activates at its existing initial position
- **AND** visual focus shifts right without moving either pane

#### Scenario: Play focused track
- **WHEN** the user presses Enter with an album or artist Workspace track focused
- **THEN** playback starts from that track and track selection remains active

#### Scenario: Exit track selection
- **WHEN** the user presses Escape or Backspace during track selection
- **THEN** the track cursor clears, tree browsing regains visual focus, and the persistent track preview remains visible

### Requirement: Wide tracks support direct mouse interaction

Each visible wide-mode Workspace track SHALL have a logical mouse target covering its full painted row. A single click SHALL select that track and activate track selection. A double-click SHALL select and play that track. Clicking an artist root, album leaf, or music-group pill SHALL clear track selection and return focus to the tree rail. Artwork and blank hero space SHALL NOT activate track selection or playback.

#### Scenario: Click a visible track
- **WHEN** the user single-clicks a visible track in an album or artist Workspace
- **THEN** that logical track becomes selected and visual focus shifts right

#### Scenario: Double-click a visible track
- **WHEN** the user double-clicks a visible track in an album or artist Workspace
- **THEN** that track becomes selected and playback starts from it

#### Scenario: Click an album while tracks have focus
- **WHEN** the user single-clicks an artist root or album leaf during track selection
- **THEN** track selection clears, that tree node becomes selected, and visual focus returns left

#### Scenario: Click artwork
- **WHEN** the user clicks artist or album artwork or blank space in the wide right hero
- **THEN** no track is selected and no playback action is invoked

### Requirement: Grouped Music pre-warms neighbour album artwork

While grouped Music is visible and image fetching is idle-gated open, the system SHALL initiate artwork fetches for album leaves neighbouring the painted selected album leaf in visible tree order: up to one behind and up to three ahead, skipping the selected album itself. This SHALL apply in both non-Wide and Wide presentations. The neighbour window SHALL be keyed off the visible projection actually being painted, not a separately resolved cursor. Artist-root focus and active in-place tree filtering SHALL NOT initiate neighbour album prefetch.

#### Scenario: Scrolling narrow grouped albums warms neighbours
- **WHEN** the user moves focus to an album leaf in the non-Wide grouped Music tree while image fetches are idle-allowed and no filter is active
- **THEN** artwork fetches are initiated for neighbouring album leaves in the visible ±3-ahead/±1-behind window

#### Scenario: Scrolling the wide right rail warms neighbours
- **WHEN** the user moves focus to an album leaf in the wide grouped Music tree while image fetches are idle-allowed and no filter is active
- **THEN** artwork fetches are initiated for neighbouring album leaves in the same visible window

#### Scenario: Rapid navigation suppresses prefetch
- **WHEN** the user is actively navigating and image fetches are idle-gated closed
- **THEN** no neighbour artwork fetches are initiated

#### Scenario: Search grid suppresses prefetch
- **WHEN** an artist root is focused or the in-place Grouped Music tree filter is active
- **THEN** no neighbour album-artwork prefetch is initiated from the tree projection
