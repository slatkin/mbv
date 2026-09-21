# Spec Delta

## MODIFIED Requirements

### Requirement: Grouped Music uses responsive compositions

The grouped Music tree view SHALL use Wide hero when it meets the shared wide geometry conditions. Its right pane SHALL contain detail and a track Workspace for the selected artist root or album leaf, and its left rail SHALL contain the single-column tree browser. Otherwise an album leaf, or an artist root with no tree filter active, SHALL open its Library Hero overlay through its existing Enter activation, and Right on an already expanded artist root SHALL also open the artist Library Hero overlay and focus its Workspace. Grouped Music SHALL NOT evaluate the breakpoint or minimum-height guard itself and SHALL NOT use a separate fallback.

Album-leaf detail SHALL retain the album title, metadata, album art, and album-track Workspace. In every composition, an album leaf's release year SHALL paint once in the canonical fixed six-column right gutter using the green `STATUS_AVAILABLE` role; artist roots and yearless leaves SHALL reserve no year gutter. Artist-root detail SHALL show artist artwork when stable Service identity and artwork are available, artist name, in-scope album count and year span, and an artist-track Workspace grouped by album. Fallback artist roots with no Service artist identity SHALL use the existing no-artwork presentation.

#### Scenario: Grouped Music below the breakpoint
- **WHEN** grouped Music does not meet the shared wide geometry conditions
- **THEN** group pills span the content width and the tree occupies the browser list slot
- **AND** Enter on an album leaf or an unfiltered artist root, or Right on an already expanded artist root, opens the Library Hero overlay with the corresponding Workspace

#### Scenario: Grouped Music at the breakpoint
- **WHEN** grouped Music meets the shared wide geometry conditions
- **THEN** it renders Wide hero with the selected artist or album detail and tracks in the right pane

#### Scenario: Album year uses the canonical metadata gutter
- **WHEN** an album leaf with a release year paints in any Grouped Music composition
- **THEN** the year paints once, right-aligned in the fixed six-column gutter using `STATUS_AVAILABLE`
- **AND** the title shrinks around that gutter without an inline or duplicate year

#### Scenario: Grouped Music lacks sufficient height
- **WHEN** grouped Music meets the width breakpoint but fails the existing minimum-height guard
- **THEN** it uses the established non-Wide Library panel composition
- **AND** it does not pin detail above the browser or introduce a separate fallback

#### Scenario: Non-Music library at wide width
- **WHEN** another hero-bearing library meets the shared wide geometry conditions
- **THEN** it continues to render Wide hero with its one-column browser

### Requirement: Wide tracks support direct mouse interaction

Each visible wide-mode Workspace track SHALL have a logical mouse target covering its full painted row. A single click SHALL select that track and activate track selection. A double-click SHALL select and play that track: in an artist Workspace it SHALL start playback from that track through the remainder of the artist's in-scope discography, and in an album Workspace it SHALL keep the existing single-album behavior. Clicking an artist root, album leaf, or music-group pill SHALL clear track selection and return focus to the tree rail. Artwork and blank hero space SHALL NOT activate track selection or playback.

#### Scenario: Click a visible track
- **WHEN** the user single-clicks a visible track in an album or artist Workspace
- **THEN** that logical track becomes selected and visual focus shifts right

#### Scenario: Double-click a visible track
- **WHEN** the user double-clicks a visible track in an album Workspace
- **THEN** that track becomes selected and playback starts from its album

#### Scenario: Double-click a visible artist Workspace track
- **WHEN** the user double-clicks a visible track in an artist Workspace
- **THEN** that track becomes selected and playback starts from it through the remainder of the artist's in-scope discography

#### Scenario: Click an album while tracks have focus
- **WHEN** the user single-clicks an artist root or album leaf during track selection
- **THEN** track selection clears, that tree node becomes selected, and visual focus returns left

#### Scenario: Click artwork
- **WHEN** the user clicks artist or album artwork or blank space in the wide right hero
- **THEN** no track is selected and no playback action is invoked

## REMOVED Requirements

### Requirement: Wide track selection preserves keyboard behavior

**Reason**: Enter on an artist root now opens that root's Hero instead of toggling its expansion, and Enter on an artist-Workspace track now plays the artist's remaining discography instead of one album. The requirement's scenarios named the superseded behaviors, so the whole block is replaced rather than partially rewritten.

**Migration**: Replaced by "Wide track selection and artist Hero entry", which keeps the album-leaf and track-movement behaviors and states the artist-root and discography behavior.

## ADDED Requirements

### Requirement: Wide track selection and artist Hero entry

Enter on a selected wide-mode album leaf SHALL activate the track cursor in the already-visible album-track Workspace. With no tree filter active, Enter on a selected artist root SHALL activate the track cursor in its artist-track Workspace, the same entry the Right chord uses on an already expanded root, and SHALL leave the root's expansion state unchanged. While the tree filter is active, Enter on an artist root SHALL retain the filter's local expansion behavior and SHALL NOT enter the Workspace. Right on a collapsed artist root SHALL expand it; Right on an already expanded artist root SHALL activate the cursor in its artist-track Workspace. Once either track Workspace has focus, existing track movement, current-item scope, and Escape or Backspace exit behavior SHALL remain unchanged. Every focused artist-Workspace activation route — keyboard Enter, Hero activation, and row double-click — SHALL start playback from that track through the remainder of the artist's in-scope discography in disc/track order. The corresponding album-Workspace routes SHALL keep their existing single-album behavior. Entering and exiting track selection SHALL NOT change the wide layout geometry.

#### Scenario: Enter enters an unfiltered artist root's Workspace
- **WHEN** the user presses Enter on a selected artist root in wide mode with no tree filter active
- **THEN** the artist-track Workspace cursor activates at its existing initial position
- **AND** visual focus shifts right without moving either pane
- **AND** the root's expansion state is unchanged

#### Scenario: Right enters an expanded artist Workspace
- **WHEN** the user presses Right on a selected artist root that is already expanded
- **THEN** the artist-track Workspace cursor activates at its existing initial position
- **AND** visual focus shifts right without moving either pane

#### Scenario: Play focused artist track
- **WHEN** the user activates a focused artist Workspace track through keyboard Enter, Hero activation, or row double-click
- **THEN** playback starts from that track through the remainder of the artist's in-scope discography in disc/track order
- **AND** track selection remains active

#### Scenario: Play focused album track
- **WHEN** the user presses Enter with an album Workspace track focused
- **THEN** playback starts from that track's album and track selection remains active

#### Scenario: Exit track selection
- **WHEN** the user presses Escape or Backspace during track selection
- **THEN** the track cursor clears, tree browsing regains visual focus, and the persistent track preview remains visible
