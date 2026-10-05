## MODIFIED Requirements

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

## ADDED Requirements

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
