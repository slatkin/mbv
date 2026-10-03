# Spec Delta

## MODIFIED Requirements

### Requirement: One rule decides whether the artwork or the header carries the title

The title SHALL be carried by at most one site at a time. The artwork SHALL carry it only while
the overlay for the current title has been painted through a non-halfblock configured protocol.
The header row SHALL carry it in every state where the overlay cannot arrive: halfblock protocol
or an otherwise incapable image protocol, images disabled, visualizer shown, visual slot hidden
(idle, or hidden by the user), or a title containing a character the embedded font
cannot render.

While the overlay can arrive but has not yet been painted — a track change before the overlay
composes, artwork still loading or showing its placeholder (the former "no slot area" condition
of this rule: no projected artwork yet), or a column resize temporarily reverting the art to
plain — NEITHER site carries the title: no header row is painted and the title is drawn nowhere
until the overlay lands. The header does not return for that window. If the playing item's
artwork never arrives, the title remains unshown for that track.

The forced-halfblock rendering used under a dimmed backdrop SHALL NOT change which site carries
the title.

#### Scenario: Overlay not yet painted

- **WHEN** a track starts and its overlay has not yet been painted
- **THEN** neither site carries the title: no header row is painted and nothing is drawn on the
  artwork, until the overlay has been painted once

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
- **AND** no header row is painted while the artwork carries the title

#### Scenario: Artwork never arrives

- **WHEN** the playing item's artwork never loads on a setup whose protocol can carry the overlay
- **THEN** the title is not shown on either site for that track and the header does not return

#### Scenario: Dimmed backdrop does not flip the site

- **WHEN** an overlay dialog that dims its backdrop opens while the artwork carries the title,
  for an Emby or an Audiobookshelf item
- **THEN** the artwork continues to carry the title and no header row is painted
