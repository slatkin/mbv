# Spec Delta

## ADDED Requirements

### Requirement: GUI Library rows show hover controls

While the pointer rests on a GUI Library row, the row SHALL show its hover controls at its trailing end, and they SHALL take the place of the row's trailing text. This holds for every top-level list row, every tree child row, every Hero tracklist row, every Workspace table row, and every Playlists screen row. When the pointer leaves the row, the controls SHALL go and the trailing text SHALL show again. A row's number SHALL stay while the controls show.

#### Scenario: The pointer rests on an album row

- **WHEN** the pointer rests on a Music album row
- **THEN** the row shows its hover controls in place of the album's year

#### Scenario: The pointer leaves the row

- **GIVEN** a row shows its hover controls
- **WHEN** the pointer leaves the row
- **THEN** the controls go and the row's trailing text shows again

#### Scenario: A tracklist row keeps its number

- **WHEN** the pointer rests on a Hero tracklist row
- **THEN** the row shows its track number and its hover controls, and the controls take the place of its duration

### Requirement: A row's hover controls follow its kind

A collection row SHALL show Play, Add to queue, and Shuffle. A collection row is an Emby folder or boxset, a Music artist or album, a Shows series or season, a Home row that names a collection, or a saved playlist. A media row SHALL show Play and Add to queue. An Audiobooks chapter row SHALL show Play only. A row that cannot play, such as a Shows Upcoming row, SHALL show no hover controls. No row SHALL show a Play next control.

#### Scenario: A series row's controls

- **WHEN** the pointer rests on a Shows series row
- **THEN** the row shows Play, Add to queue, and Shuffle

#### Scenario: An episode row's controls

- **WHEN** the pointer rests on a Shows episode row of the Workspace table
- **THEN** the row shows Play and Add to queue, and no Shuffle

#### Scenario: A playlist item row's controls

- **WHEN** the pointer rests on an item row of an open saved playlist
- **THEN** the row shows Play and Add to queue, and no Shuffle

#### Scenario: A chapter row's controls

- **WHEN** the pointer rests on an Audiobooks chapter row
- **THEN** the row shows Play only

#### Scenario: An Upcoming row shows no controls

- **WHEN** the pointer rests on a Shows Upcoming row
- **THEN** the row shows no hover controls and keeps its trailing text

### Requirement: A hover control does what the row's menu entry does

Play, Add to queue, and Shuffle SHALL each have the same effect as the entry of the same name in the row's right-click menu. A row with no right-click menu, such as an Audiobooks chapter row, SHALL play as its double-click plays it. A press on a hover control SHALL NOT also count as a press or a double-click on the row.

#### Scenario: Add to queue from an album row

- **GIVEN** the queue holds two items
- **WHEN** the user presses the Add to queue control of a Music album row
- **THEN** the album's tracks are appended after the two items, as the album's right-click Add to Queue entry appends them

#### Scenario: Shuffle from a folder row

- **WHEN** the user presses the Shuffle control of an Emby folder row
- **THEN** the folder's items play in shuffled order, as the folder's right-click Shuffle entry plays them

#### Scenario: Play from a tracklist row

- **WHEN** the user presses the Play control of a Hero tracklist row
- **THEN** the track plays as a double-click on the row plays it

#### Scenario: Play from a chapter row

- **WHEN** the user presses the Play control of an Audiobooks chapter row
- **THEN** the book plays from that chapter, as a double-click on the row plays it

#### Scenario: A control press does not pick the row

- **GIVEN** the Hero shows an album
- **WHEN** the user presses the Add to queue control of another album's row
- **THEN** the Hero still shows the first album

### Requirement: A GUI tree parent row shows an expand control on hover

A GUI tree parent row SHALL show an Expand/collapse control with its other hover controls. A press on the control SHALL open the node when it is closed and close it when it is open, as a double-click on the row does. The control SHALL show which of the two a press does now. A row that is not a parent row SHALL show no Expand/collapse control.

#### Scenario: A closed artist is expanded from its control

- **GIVEN** a Music artist row is closed
- **WHEN** the user presses the row's Expand/collapse control
- **THEN** the artist opens and its album rows show under it
- **AND** nothing plays and the queue does not change

#### Scenario: An open playlist is collapsed from its control

- **GIVEN** a saved playlist row is open on the Playlists screen
- **WHEN** the user presses the row's Expand/collapse control
- **THEN** the playlist closes and its item rows no longer show

#### Scenario: A leaf row has no expand control

- **WHEN** the pointer rests on a track, episode, or playlist item row
- **THEN** the row shows no Expand/collapse control

### Requirement: A queue row shows a Play control on hover

While the pointer rests on a queue row, the row SHALL show a Play control in its right column, in place of the item's duration, at the row's trailing end. A press on the control SHALL play that slot, as a double-click on the row does. A double-click on the control SHALL NOT play the row a second time.

#### Scenario: A hovered queue row plays from its control

- **GIVEN** the queue holds five items and the second plays
- **WHEN** the user presses the Play control of the fourth row
- **THEN** the fourth slot plays and the queue order does not change

### Requirement: The GUI Client offers no Play next

The GUI Client SHALL offer no action that admits an item directly after the active slot, and no action that moves a queued slot to play next. A drag or a move key moves a queued slot.

#### Scenario: No Play next in the Hero or the row menus

- **WHEN** the Hero, a Library right-click menu, or the queue row menu shows
- **THEN** it shows no Play next action

#### Scenario: `n` over the Library is not a Library key

- **GIVEN** the pointer rests on a Library row
- **WHEN** the user presses `n`
- **THEN** the queue does not change, and the key goes to the Client core's keyboard router

## MODIFIED Requirements

### Requirement: The queue shows a row menu and drag reorder

The queue header SHALL hold only what the requirement "The queue header holds the chooser and its controls" (`gui-queue-chooser`) names. Every row SHALL have the same fixed height and SHALL carry its queue ordinal, its title, and a right column. The right column SHALL show the item's duration in every row state, except while the row shows its hover Play control in its place. A row with progress (partly played, or now playing) SHALL show a progress ring after its title; the ring SHALL be the row's only progress mark and SHALL NOT add height to the row. The now-playing row's ring SHALL follow the playhead. Music is a no-progress media type: a music row SHALL show no ring, whether it is playing or not. A row SHALL show no time-left text, no start mark, and no state glyph in the right column. Every queue edit SHALL be a request to the Player owner of the shown queue. The window content SHALL NOT change the canonical queue.

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
- **THEN** the row paints the canvas's hovered row: it keeps its ordinal, and the hover Play control takes the place of the duration at the row's trailing end

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

### Requirement: The Library keys act on the hovered row

While the pointer is over the Library column, the window content SHALL take the Library keys. `/` opens the list filter. Enter plays the hovered row and `a` adds it to the queue; with no hovered row they do nothing. While the pointer is over the destination's list, the arrow, Page Up, Page Down, Home, and End keys scroll that list. Every key that the window content does not take SHALL go to the Client core's keyboard router.

#### Scenario: Enter plays the hovered row

- **WHEN** the pointer rests on a track row of the Music list and the user presses Enter
- **THEN** the track plays as a double-click on the row plays it

#### Scenario: The Library keys reach the Library under the pointer

- **GIVEN** the pointer rests over the Library list
- **WHEN** the user presses `/`, types `bo`, presses Esc, and then presses Space
- **THEN** `/` opens the Library filter, not the Queue filter, the list shows only the rows whose names contain `bo`, Esc closes the filter, and Space plays or pauses

#### Scenario: The filter picks nothing

- **GIVEN** the Hero shows an album
- **WHEN** the Library filter narrows the list so that the album's row is hidden
- **THEN** the Hero still shows the album

#### Scenario: The Queue keeps its keys under the pointer

- **GIVEN** the pointer rests over the Queue list box
- **WHEN** the user presses `/`, then Esc, then `a`
- **THEN** `/` opens the Queue filter and `a` opens the audio popover

#### Scenario: An arrow key scrolls the list

- **GIVEN** the pointer rests over the Music list
- **WHEN** the user presses Down
- **THEN** the list scrolls by one row
- **AND** no row is picked and the Hero does not change

#### Scenario: Enter with no hovered row

- **GIVEN** the pointer rests over the Library column but on no row
- **WHEN** the user presses Enter or `a`
- **THEN** nothing plays and the queue does not change

#### Scenario: Enter plays an Emby library's folder

- **WHEN** the pointer rests on a folder or boxset row of an Emby library and the user presses Enter
- **THEN** the folder plays through the TUI's folder-play rule, the queue is replaced by its items, and no nested list opens

#### Scenario: Prefix mode passes the Library

- **GIVEN** the pointer rests over the Library column and prefix mode is armed
- **WHEN** the user presses a key
- **THEN** the key resolves in the prefix namespace, and the Library does not take it

### Requirement: The Hero shows the picked item with its actions

The Hero SHALL show the picked album with its artist header, its square artwork, and its tracklist. It SHALL offer Play, Add to queue with the key hint `a`, Shuffle artist, and More actions. Play SHALL replace the queue and start the item without the TUI's populated-queue replace-confirm. The saved-playlist save/discard prompt SHALL still present, through the GUI's confirmation overlay. More actions SHALL open the item's menu. A tracklist row SHALL show the hover controls that the requirement "GUI Library rows show hover controls" defines, and a double-click on a tracklist row SHALL play that track, as it does in every Workspace list (Shows episodes, Audiobooks chapters, Feeds entries). The tracklist row of the track playing at the playhead SHALL show a play glyph beside its title. A highlight marks focus only, so the playing row SHALL NOT paint a green fill.

#### Scenario: The Hero paints a picked album

- **WHEN** the user picks the album Licensed to Ill
- **THEN** the artist header shows the artist, the selected collections, the album count, and the track count and total time
- **AND** the album block shows the year, the title, the track count, the time, and the square artwork, above the tracklist

#### Scenario: Play next from the Hero

- **WHEN** the Hero shows a picked album
- **THEN** it offers no Play next control, and the `n` key admits nothing

#### Scenario: A tracklist row's hover controls

- **WHEN** the pointer is over a tracklist row and the user presses its Add to queue control
- **THEN** that track is appended at the end of the queue

#### Scenario: A tracklist row is played

- **GIVEN** the pointer is over a tracklist row
- **WHEN** the user presses the row's Play control, or double-clicks the row
- **THEN** the track plays: the queue is replaced, as a double-clicked list track is

#### Scenario: More actions opens the item menu

- **WHEN** the user presses the Hero's More actions control
- **THEN** the item's menu opens in the row-menu look, and each entry runs its action for that item

### Requirement: Every Hero is one frame with the largest unit as its title

The Hero of Music, Shows, Podcasts, Audiobooks, and Feeds SHALL paint one frame: a header band, a detail block beside or above the art, and a list band. The header band's title SHALL be the name of the largest unit: the artist, the series, the podcast, the feed, or, for a book, the book. The detail block SHALL show the selected smaller unit (the album, the season or episode, the episode, the entry) with its duration and progress, its Play and Add to queue actions, and its overview. Plain facts with no role SHALL sit on one line under the header title. A destination SHALL map its facts onto this frame and SHALL NOT remove a band, add a layout, or paint its own copy of a part. Home and Movies are not yet on the frame.

#### Scenario: A Podcasts episode's Hero

- **WHEN** the Hero shows an episode of a podcast
- **THEN** the header title is the podcast's name
- **AND** the detail block shows the episode's name, its duration, and its actions

#### Scenario: A Feeds entry's Hero

- **WHEN** the Hero shows an entry of a feed
- **THEN** the header title is the feed's name
- **AND** the list band holds that feed's entries, newest first, with the selected entry marked

#### Scenario: A TV series' actions act on the selected season

- **WHEN** a series is selected and the user presses Play or Add to queue
- **THEN** the selected season is played or appended

## REMOVED Requirements

### Requirement: A GUI tree parent row shows a double-click hint on hover

**Reason**: The Expand/collapse hover control replaces the hint. A visible control makes the gesture discoverable and also performs it.

**Migration**: The requirement "A GUI tree parent row shows an expand control on hover" covers tree parent rows. The double-click still opens and closes the node ("GUI tree parent rows open and close on double-click" does not change).
