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
of the artwork. A title with a context part (artist and song, show and episode) SHALL be drawn
as two rows: the context part at the top and the title part at the bottom. Each row SHALL be a
single line over its own scrim that fades into the unmodified artwork, and SHALL NOT wrap. The
overlay SHALL use the same title and context text the header row would carry. A context part SHALL
paint in the playback panel's context role (yellow), a title part beneath a context part in its
title role (aqua), and a one-part title in the context role (yellow), matching the header. Paused
playback keeps the overlay.

#### Scenario: One-part title

- **WHEN** a movie, track without context, or other one-part title plays with the overlay live
- **THEN** the title is drawn in a single scrim row at the top of the artwork
- **AND** nothing is drawn at the bottom of the artwork

#### Scenario: Two-part title

- **WHEN** an item with a context part plays with the overlay live
- **THEN** the context part is drawn in a scrim row at the top of the artwork
- **AND** the title part is drawn in a scrim row at the bottom of the artwork

#### Scenario: Artwork outside the rows is untouched

- **WHEN** the overlay is drawn
- **THEN** the artwork between the scrim rows is not modified

### Requirement: Overlay text is fitted to its row

Each overlay row SHALL fit its text to the artwork width. Text wider than the row SHALL shrink
down to a minimum legible size; text still wider at that size SHALL end with an ellipsis. The
overlay SHALL NOT scroll. The nominal text size SHALL be defined by the terminal's cell grid —
one overlay row is one terminal cell row tall — and SHALL NOT scale with the artwork's size, so
the overlay text is as tall as the terminal's own text at every artwork size.

#### Scenario: Long title shrinks first

- **WHEN** a title is wider than its row at the nominal size but fits at a smaller allowed size
- **THEN** the row draws the whole title at the smaller size with no ellipsis

#### Scenario: Text size follows the cell grid, not the artwork

- **WHEN** the same title is drawn on a small artwork box and on a large one
- **THEN** the text height is the same number of cell rows on both
- **AND** the larger box fits more characters at the nominal size

#### Scenario: Very long title is ellipsised

- **WHEN** a title does not fit its row even at the minimum size
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

#### Scenario: Dimmed backdrop does not flip the site

- **WHEN** an overlay dialog that dims its backdrop opens while the artwork carries the title,
  for an Emby or an Audiobookshelf item
- **THEN** the header row continues to read `Now Playing`
