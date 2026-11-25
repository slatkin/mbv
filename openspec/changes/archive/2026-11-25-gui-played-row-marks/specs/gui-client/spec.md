# Spec Delta

## MODIFIED Requirements

### Requirement: The queue shows a row menu and drag reorder

The queue header SHALL hold only what the requirement "The queue header holds the chooser and its controls" (`gui-queue-chooser`) names. Every row SHALL have the same fixed height and SHALL carry its queue ordinal, its title, and a right column. The right column SHALL show the item's duration in every row state, except while the row shows its hover Play control in its place. A row with progress (partly played, or now playing) SHALL show a progress ring after its title; the ring SHALL be the row's only progress mark and SHALL NOT add height to the row. The now-playing row's ring SHALL follow the playhead. Music is a no-progress media type: a music row SHALL show no ring, whether it is playing or not. A row SHALL show no time-left text and no start mark. A played row and an unplayed row SHALL each show its played or unplayed mark before the duration in the right column, as the requirement "Rows mark their played state" states; a partly played row is an unplayed row. The now-playing row SHALL keep its own look and SHALL show neither mark, and no row SHALL be struck through. Every queue edit SHALL be a request to the Player owner of the shown queue. The window content SHALL NOT change the canonical queue.

#### Scenario: Rows show their state

- **WHEN** the queue holds a watched row, the active row, and a row with partial progress
- **THEN** the watched row shows the played mark and the played look, and its title is not struck through
- **AND** the active row is filled and shows no played or unplayed mark
- **AND** the partial row shows the unplayed mark and keeps its progress ring
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
- **THEN** the row paints the canvas's hovered row: it keeps its ordinal, and the hover Play control takes the place of the duration and the played or unplayed mark at the row's trailing end

#### Scenario: The row menu opens

- **WHEN** the user right-clicks the row or its hover Play control
- **THEN** the menu opens with Play now (`⏎`), Mark watched (`w`), Go to episode (`g`), Shuffle, Remove from queue (`dd`), and Clear queue (`c`)
- **AND** the menu shows no Play next entry

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

The Library's pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every Library list row and every Playlists screen row SHALL paint through one of two row parts, the flat row or the tree row, and no list SHALL paint a row of its own. A row SHALL have one of three levels, and every row of one level SHALL look the same in every list, with the look of that level in the Music list:

- A top-level row (the Music artist row's look) is every row of a flat list and the top-level row of every tree list (a Music artist, a Shows series, a Playlists playlist). It SHALL paint through the flat row, with its art thumb (`library-row-artwork`) when the item has art.
- A nested parent row (the Music album row's look) is a Music album or a Shows season. It SHALL paint through the tree row.
- A leaf row (the Music track row's look) is a Music track, a Shows episode, or a Playlists item. It SHALL paint through the tree row.

A destination SHALL supply the row's data, its state, its hook prefix, and its handlers, and SHALL NOT supply or restate the row's look. The list row parts and the Workspace table rows SHALL carry one play state, and the played title tone and the resume percent SHALL come from one derivation shared by both; no destination SHALL convert the play state into a second state of its own. A row whose item carries a play state SHALL show its played or unplayed mark and look, as the requirement "Rows mark their played state" states; a row whose item carries no play state SHALL show neither. A Workspace table SHALL paint a header row, a number or play-glyph cell, a title, a duration, and a progress run, and the row that plays at the playhead SHALL keep its own playing mark beside its title. An Audiobooks chapter row SHALL show its number.

#### Scenario: A list row looks the same in every destination

- **WHEN** a Home, Emby, Music artist, Shows series, Shows New, Shows Upcoming, Audiobooks, Podcasts, Feeds, or Playlists playlist row paints
- **THEN** it paints through the flat row with the same height, title, subtitle, trailing text, and selection fill as a Music artist row

#### Scenario: Continue watching shows the time left

- **WHEN** a Home Continue watching row paints
- **THEN** its trailing text is the time left in that item

#### Scenario: A nested row keeps its look

- **WHEN** a Shows season row, a Shows episode row, or a Playlists item row paints
- **THEN** it paints through the tree row, without a thumb
- **AND** a season looks as a Music album row looks, and an episode or a Playlists item looks as a Music track row looks

#### Scenario: A played row mutes its title

- **GIVEN** a list row's item carries a play state
- **WHEN** the item is played
- **THEN** the row shows the played mark and the played look

#### Scenario: A row with no play state shows no mark

- **GIVEN** a Music track row, an Emby library row, or a Playlists item row, whose item carries no play state
- **WHEN** the row paints
- **THEN** the row shows no played or unplayed mark and keeps its present look

#### Scenario: A Workspace row carries the shared play state

- **GIVEN** a Workspace destination carries a play state on its rows, as the Shows episodes Workspace and the Feeds entries Workspace do
- **WHEN** a Workspace row paints
- **THEN** the row carries the play state the list rows carry, and no second state or percent beside it
- **AND** a played row shows the played mark and look, a partly played row shows the unplayed mark and paints its progress run at its resume percent, and an ordinary row shows the unplayed mark and paints no progress run

#### Scenario: The playing Workspace row keeps its own mark

- **GIVEN** a Workspace row is the one playing at the playhead
- **WHEN** the row paints
- **THEN** its title keeps the playing row's tone and its play glyph paints beside the title
- **AND** the row's carried play state does not change that tone, and the row shows no played or unplayed mark

#### Scenario: A partly played row with no percent stays ordinary

- **GIVEN** a Workspace row is partly played and carries no resume percent
- **WHEN** the row paints
- **THEN** the row paints no progress run and shows the unplayed mark


## ADDED Requirements

### Requirement: Rows mark their played state

A GUI row whose item carries a play state SHALL show its played or unplayed state by its look and by a mark. A played row SHALL look weaker than an unplayed row in its title, its secondary text, its trailing text, and its thumb. A partly played row SHALL be an unplayed row and SHALL keep its progress mark. Each mark SHALL carry an accessible name for its state. Every row of a list that carries a play state SHALL keep one same-size place for the mark.

#### Scenario: An unplayed row

- **GIVEN** a Home Recently added row whose item is not played
- **WHEN** the row paints
- **THEN** the row shows the unplayed mark before its trailing text
- **AND** the mark's accessible name names the unplayed state

#### Scenario: A played row

- **GIVEN** a Podcasts episode row whose item is played
- **WHEN** the row paints
- **THEN** the row shows the played mark in the unplayed mark's place
- **AND** its title, subtitle, trailing text, and thumb paint weaker than an unplayed row's
- **AND** its title is not struck through
- **AND** the mark's accessible name names the played state

#### Scenario: A partly played row

- **GIVEN** a Home Continue watching row at 40%
- **WHEN** the row paints
- **THEN** the row shows the unplayed mark and the unplayed look
- **AND** its thumb shows 40% progress along its bottom edge, as before

#### Scenario: The state changes in place

- **GIVEN** a Shows episode row that is not played
- **WHEN** the user marks the episode played and the row paints again
- **THEN** the played mark takes the unplayed mark's place
- **AND** the row's title and trailing text keep their positions

#### Scenario: A row with no trailing text

- **GIVEN** a Home row whose item carries a play state and no trailing text
- **WHEN** the row paints
- **THEN** the mark shows at the row's trailing end

#### Scenario: The hover controls cover the trailing end

- **GIVEN** a Feeds entry row that offers hover controls
- **WHEN** the pointer rests on the row
- **THEN** the mark hides with the trailing text while the hover controls show
