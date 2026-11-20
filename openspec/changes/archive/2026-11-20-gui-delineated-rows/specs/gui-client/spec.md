# Spec Delta

## MODIFIED Requirements

### Requirement: The queue shows a row menu and drag reorder

The queue header SHALL hold only what the requirement "The queue header holds the chooser and its controls" (`gui-queue-chooser`) names. Every row SHALL have the same fixed height and SHALL carry its queue ordinal, its title, and a right column. The right column SHALL show the item's duration in every row state. A row with progress (partly played, or now playing) SHALL show a progress ring after its title; the ring SHALL be the row's only progress mark and SHALL NOT add height to the row. The now-playing row's ring SHALL follow the playhead. Music is a no-progress media type: a music row SHALL show no ring, whether it is playing or not. A row SHALL show no time-left text, no start mark, and no state glyph in the right column. Every queue edit SHALL be a request to the Player owner of the shown queue. The window content SHALL NOT change the canonical queue.

#### Scenario: Rows show their state

- **WHEN** the queue holds a watched row, the active row, and a row with partial progress
- **THEN** the watched row is struck through, the active row is filled, and the partial row shows a progress ring after its title
- **AND** every row has the same height, the partial row included
- **AND** each row carries its 1-based queue ordinal as the canvas paints it

#### Scenario: The right column is switched

- **WHEN** the queue holds a new row, a partly played row, a watched row, and the now-playing row
- **THEN** each row's right column shows that item's duration
- **AND** the header shows no right-column switch, and no row shows a time left or an end time
- **AND** no GUI key switches the right column

#### Scenario: The now-playing ring follows the playhead

- **WHEN** the now-playing item plays on
- **THEN** its row's ring advances with the playhead, and its right column keeps the item's duration

#### Scenario: A music row shows no progress ring

- **WHEN** the queue holds a music item, playing or not
- **THEN** its row shows no progress ring, and its right column keeps the item's duration

#### Scenario: The queue header counts the queue

- **WHEN** the queue header paints
- **THEN** it shows no remaining count and no queue finish clock

#### Scenario: A stopped queue marks its start slot

- **GIVEN** the shown queue is stopped with its third slot as its start slot
- **WHEN** the queue rows paint
- **THEN** no row carries a start mark, and no row is the now-playing row

#### Scenario: A peer queue marks its playing row

- **GIVEN** the GUI Client shows a peer that reports no start slot and plays its second slot
- **WHEN** the queue rows paint
- **THEN** the second row is the now-playing row, and no row carries a start mark

#### Scenario: Play turns the start mark into the playing mark

- **GIVEN** the shown queue is stopped with its third slot as its start slot
- **WHEN** the user presses Play
- **THEN** the third row becomes the now-playing row, and no row carries a start mark

#### Scenario: A row is hovered

- **WHEN** the pointer is over a queue row
- **THEN** the row paints the canvas's hovered row: the grip replaces the row's ordinal and the row-actions button shows at the row's end

#### Scenario: The row menu opens

- **WHEN** the row-actions button is pressed
- **THEN** the menu opens with Play now (`⏎`), Play next (`n`), Mark watched (`w`), Go to episode (`g`), and Remove from queue (`dd`)

#### Scenario: The destructive entry is distinct

- **WHEN** the row menu paints
- **THEN** Remove from queue paints in the canvas's destructive colour, separated from the other entries by the menu divider

#### Scenario: A row is removed from the menu

- **WHEN** the user chooses Remove from queue
- **THEN** the Player owner of the shown queue removes that slot from it

#### Scenario: A row is dragged

- **WHEN** the user drags a row to a new position and releases
- **THEN** the queue shows an insertion line and a drag ghost during the drag
- **AND** the Player owner of the shown queue moves the slot to the new position

#### Scenario: A row is moved by key

- **WHEN** the user presses `Alt+J` on a row
- **THEN** the row moves one place down, as the drag would do

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, flat list rows, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every top-level row of every destination, Home included, SHALL paint through the one top-level row part with its art thumb (`library-row-artwork`); nested rows SHALL keep their own part. A destination SHALL supply its data, its hook prefix, and its handlers, and SHALL NOT restate the part's look. A Workspace table SHALL paint a header row, a number or play-glyph cell, a title, a duration, and a progress run. An Audiobooks chapter row SHALL show its number.

#### Scenario: A list row looks the same in every destination

- **WHEN** a Home, Emby, Music artist, Shows series, Shows New, Shows Upcoming, Audiobooks, Podcasts, or Feeds top-level row paints
- **THEN** it paints through the shared top-level row with the same thumb, title, subtitle, and trailing text

#### Scenario: Continue watching shows the time left

- **WHEN** a Home Continue watching row paints
- **THEN** its trailing text is the time left in that item

#### Scenario: A nested row keeps its look

- **WHEN** an open series' season and episode rows, or an open artist's album and track rows, paint
- **THEN** they paint without a thumb, as before this change
