# queue-artwork-title-overlay Specification

## Purpose

Draws the now-playing title onto the queue column's artwork when the terminal shows real images,
so the header row can drop to a plain label, and defines the one rule that decides whether the
artwork or the header row carries the title.

## Requirements

### Requirement: The title is drawn onto live queue artwork

While the queue column's visual slot shows real artwork through the configured Kitty, Sixel or
iTerm2 image protocol and local or watched playback is active (paused included), the now-playing title SHALL be drawn onto the
artwork itself, over a scrim. A title with no context part SHALL be drawn as one row at the top
of the artwork, unless it splits across two rows under the one-part split rule. A title with a context part (artist and song, show and episode) SHALL be drawn
as two rows: the context part at the top and the title part at the bottom. Each row SHALL be a
single line over its own flat translucent scrim, SHALL be horizontally centred within the side
padding, and SHALL NOT wrap. The overlay SHALL use the same title and context text the header row
would carry. A context part SHALL paint yellow; a title part beneath a context part SHALL paint
cream (`mbv_theme::TEXT_EMPHASIS`); and a one-part title SHALL remain yellow in both of its rows. Paused
playback keeps the overlay. Overlay text SHALL be painted over a soft dark drop shadow so it remains legible on light or busy artwork.

#### Scenario: One-part title

- **WHEN** a movie, track without context, or other one-part title that fits its row at the nominal size plays with the overlay live
- **THEN** the title is drawn in a single scrim row at the top of the artwork in yellow
- **AND** the row's text is horizontally centred within the side padding
- **AND** nothing is drawn at the bottom of the artwork

#### Scenario: Two-part title

- **WHEN** an item with a context part plays with the overlay live
- **THEN** the context part is drawn in a scrim row at the top of the artwork
- **AND** the title part is drawn in a scrim row at the bottom of the artwork
- **AND** both rows' text is horizontally centred within the side padding

#### Scenario: Artwork outside the rows is untouched

- **WHEN** the overlay is drawn
- **THEN** the artwork between the scrim rows is not modified

### Requirement: Overlay text is fitted to its row

Each overlay row SHALL fit its text to the artwork width. A one-part title wider than its row at
the nominal size SHALL first split under the one-part split rule. Text wider than the row SHALL shrink
down to a minimum legible size; text still wider at that size SHALL end with an ellipsis. The
overlay SHALL NOT scroll. The nominal text size SHALL be defined by the terminal's cell grid —
one overlay row is one terminal cell row tall — and SHALL NOT scale with the artwork's size, so
the overlay text is as tall as the terminal's own text at every artwork size.

#### Scenario: Long title shrinks first

- **WHEN** a context part or a title part beneath a context part is wider than its row at the nominal size but fits at a smaller allowed size
- **THEN** the row draws the whole text at the smaller size with no ellipsis

#### Scenario: Text size follows the cell grid, not the artwork

- **WHEN** the same title is drawn on a small artwork box and on a large one
- **THEN** the text height is the same number of cell rows on both
- **AND** the larger box fits more characters at the nominal size

#### Scenario: Very long title is ellipsised

- **WHEN** a row's text does not fit the row even at the minimum size
- **THEN** the row ends with an ellipsis and the text stays within the artwork width

### Requirement: The overlay is composed once per track and box

The overlay SHALL be composed when the playing track, the artwork's rendered box, the active
image protocol, or the title text changes, and SHALL NOT be recomposed or retransmitted on
playback ticks. No time-varying content (position, remaining time, status chips) SHALL be drawn
into the artwork. The artwork cache entry the overlay lives in SHALL be distinct from the plain
artwork entry other consumers (MPRIS art, Library surfaces) read.

#### Scenario: Playback ticks do not rebuild the overlay

- **WHEN** playback advances within a track with no change of title, box or protocol
- **THEN** the overlay artwork is not recomposed or re-encoded

#### Scenario: Track change recomposes

- **WHEN** the playing track changes
- **THEN** the overlay is composed for the new title and artwork

#### Scenario: Plain artwork stays plain

- **WHEN** the overlay artwork is live for a track
- **THEN** the plain cached artwork for the same item carries no overlay text

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

- **WHEN** a movie plays on a watched remote Emby session with no local queue slot, and
  the card shows that item's artwork
- **THEN** the title is drawn onto the artwork as a one-part title from the session's now-playing
  name
- **AND** the header row reads `Now Playing` once it has been painted

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
sized relative to the terminal cell, not the artwork. An item with no logo, a logo still loading
or failed, and items that are not Emby movies or episodes SHALL keep the text row.

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

### Requirement: The queue card key and fetch chain are item-type specific

The Queue card's artwork key and fetch chain SHALL depend on the item type. A Movie SHALL use the
landscape key `{id}:QB` and fetch `Backdrop` then `Primary`, without `Logo`. An Episode SHALL
keep the portrait key `{id}:P` and fetch `Primary`, `Thumb`, `Backdrop`, `Logo`, so its card shows
the episode's own still and never the series' art. A watched Session's now-playing item held
without an item record SHALL choose key and chain by its item type with the same split. MPRIS art
SHALL fall back to the landscape key when the portrait key is not cached.

#### Scenario: Movie card uses the landscape key

- **WHEN** a Movie is the card's item
- **THEN** its artwork is cached and read under `{id}:QB`
- **AND** its fetch chain does not request `Logo`

#### Scenario: Episode card keeps its own still

- **WHEN** an Episode is the card's item
- **THEN** its artwork is cached under `{id}:P` and fetched `Primary` first

#### Scenario: MPRIS finds a Movie's art

- **WHEN** a Movie plays and only `{id}:QB` is cached
- **THEN** MPRIS publishes that image as the track art

### Requirement: A long one-part title splits across the top and bottom rows

When a one-part title is drawn as text (no logo replaces the top row) and is wider than its row at
the nominal size, it SHALL split at the space nearest the middle of the title, and the earlier space SHALL win a tie. The text before
that space SHALL be drawn in the top row and the text after it in the bottom row, both yellow,
each over its own scrim and centred within the side padding. Each half SHALL then be fitted to
its row by the shrink-then-ellipsis rule. A one-part title with no space SHALL NOT split and SHALL
be drawn in the top row only.

#### Scenario: Long one-part title splits before shrinking

- **WHEN** a one-part title such as a home video name is wider than its row at the nominal size and contains a space
- **THEN** the words before the space nearest the middle are drawn in the top row
- **AND** the remaining words are drawn in the bottom row
- **AND** both rows paint yellow

#### Scenario: Split half still too wide

- **WHEN** a half of a split one-part title is wider than its row at the nominal size
- **THEN** that row shrinks and then ellipsises its half under the fit rule

#### Scenario: One-part title with no space

- **WHEN** a one-part title with no space is wider than its row at the nominal size
- **THEN** it is drawn in the top row only, shrunk and ellipsised as needed
- **AND** nothing is drawn at the bottom of the artwork

#### Scenario: Logo keeps a one-part title as the logo alone

- **WHEN** a one-part title has a loaded logo
- **THEN** the logo alone is drawn and no text row is drawn, whatever the title's length
