# Spec Delta

## MODIFIED Requirements

### Requirement: One rule decides whether the artwork or the header carries the title

The title SHALL be carried by exactly one site at a time. The artwork SHALL carry it only while
the overlay for the current title has been painted through a non-halfblock configured protocol.
In every other state — halfblock protocol, visualizer shown, placeholder or loading slot,
images disabled, visual slot hidden (idle, hidden by the user, or no slot area), no ready overlay yet, or a title containing a character
the embedded font cannot render — the header row SHALL carry it. Any uncertainty SHALL resolve
to the header carrying the title, so the title is never absent from both sites.

The forced-halfblock rendering used under a dimmed backdrop SHALL NOT change which site carries
the title.

#### Scenario: Overlay not yet painted

- **WHEN** a track starts and its overlay has not yet been painted
- **THEN** the header row carries the title until the overlay has been painted once

#### Scenario: Visualizer replaces artwork

- **WHEN** the user switches the visual slot from artwork to the visualizer
- **THEN** the header row carries the title

#### Scenario: Halfblock configured

- **WHEN** the configured image protocol is halfblocks
- **THEN** no title is drawn onto the artwork and the header row carries it

#### Scenario: Idle card art never carries a title

- **WHEN** no item is actively playing and the card shows the cursor row's artwork
- **THEN** no title is drawn onto that artwork

#### Scenario: Unsupported glyph

- **WHEN** the title contains a character the embedded font has no glyph for
- **THEN** no title is drawn onto the artwork for that track and the header row carries it

#### Scenario: Playing on a remote target

- **WHEN** an item plays on a watched remote Emby session with no local queue slot, and
  the card shows that item's artwork
- **THEN** the title is drawn onto the artwork with the same part split local playback draws:
  an episode whose session payload carries a series name draws the show name as the context part
  and the episode name as the title part, and any other item draws a one-part title from the
  session's now-playing name
- **AND** the header row reads `Now Playing` once it has been painted

#### Scenario: Remote session payload lacks the series name

- **WHEN** an episode plays on a watched remote Emby session with no local queue slot and its
  session payload carries no series name
- **THEN** the title is drawn onto the artwork as a one-part title from the session's
  now-playing name

#### Scenario: Dimmed backdrop does not flip the site

- **WHEN** an overlay dialog that dims its backdrop opens while the artwork carries the title,
  for an Emby or an Audiobookshelf item
- **THEN** the header row continues to read `Now Playing`

### Requirement: A logo replaces the top text row

While the overlay is live and the playing Emby item has a logo image that has been loaded, the
logo SHALL be drawn in the upper-left corner of the artwork in place of the top text row and
its scrim. A movie uses its own logo; an episode uses its show's logo. A two-part title SHALL keep
its bottom title row; a one-part title SHALL then be drawn as the logo alone. The logo SHALL be
scaled to fit, preserving its aspect ratio, within a box inset from the artwork's top-left and
sized relative to the terminal cell, not the artwork. The same rule SHALL apply to a watched
remote Emby session's now-playing item: an episode resolves its show's logo from the session's
series id, and a movie resolves its own logo when the session payload advertises a logo image.
An item with no logo, a logo still loading or failed, an item whose session payload carries no
resolvable logo reference, and items that are not Emby movies or episodes SHALL keep the text row.

#### Scenario: Episode with a show logo

- **WHEN** an episode plays and its show has a loaded logo
- **THEN** the show logo is drawn in the upper-left corner with no scrim behind it
- **AND** the episode title is drawn in the bottom row

#### Scenario: Movie with a logo

- **WHEN** a movie with a loaded logo plays
- **THEN** only the logo is drawn, in the upper-left corner, and no title text is drawn

#### Scenario: Logo arrives after the text overlay

- **WHEN** a logo finishes loading while the text overlay is already painted
- **THEN** the artwork is recomposed with the logo in place of the top text row

#### Scenario: No logo

- **WHEN** the item has no logo, or the logo fails to load
- **THEN** the text overlay is unchanged

#### Scenario: Remote episode uses its show's logo

- **WHEN** an episode plays on a watched remote Emby session with no local queue slot and its
  session payload carries a series id
- **THEN** the show's logo is fetched and drawn in the upper-left corner in place of the top
  text row, exactly as local playback draws it
- **AND** the episode title is drawn in the bottom row

#### Scenario: Remote movie with an advertised logo

- **WHEN** a movie plays on a watched remote Emby session with no local queue slot and its
  session payload advertises a logo image
- **THEN** only that logo is drawn, in the upper-left corner, and no title text is drawn

#### Scenario: Remote item without a resolvable logo reference

- **WHEN** the now-playing item of a watched remote Emby session carries no series id (episode)
  or no advertised logo image (movie), or the logo fetch fails
- **THEN** the text overlay is unchanged
