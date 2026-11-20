# Spec Delta

## ADDED Requirements

### Requirement: The Playlists control toggles the Playlists view in the Queue column

The top bar's Playlists control and the `playlists_open` action SHALL toggle what the Queue column shows between the queue list and the Playlists view. While the view shows, the top bar SHALL mark the Playlists control. Opening the view SHALL close the Settings screen, and opening the Settings screen SHALL close the view.

#### Scenario: The Playlists control opens the view

- **GIVEN** the Queue column shows the queue list
- **WHEN** the user presses the Playlists control
- **THEN** the Queue column shows the Playlists view in place of the queue list
- **AND** the top bar marks the Playlists control

#### Scenario: A second press shows the queue again

- **GIVEN** the Queue column shows the Playlists view
- **WHEN** the user presses the Playlists control
- **THEN** the Queue column shows the queue list again and the queue is unchanged

#### Scenario: The action opens the view from the Settings screen

- **GIVEN** the Settings screen is open
- **WHEN** the user runs `playlists_open`
- **THEN** the Settings screen closes and the Queue column shows the Playlists view

### Requirement: The playback panel stays above the Playlists view

While the Playlists view shows, the Queue column SHALL show the playback panel above the view as it does above the queue list, except the now-playing title block: the view's title block, which names the picked row, SHALL show in its place. Under the title block the view SHALL show the picked row's controls, then the playlist tree. The Queue footer SHALL stay under the view.

#### Scenario: An item plays while the view opens

- **GIVEN** an item plays and its art shows in the playback panel
- **WHEN** the user opens the Playlists view
- **THEN** the art and the transport stay in place
- **AND** the view's title block shows under the art in place of the now-playing title, followed by the controls and the tree

#### Scenario: Nothing plays

- **GIVEN** nothing plays and the playback panel shows no art and no title
- **WHEN** the user opens the Playlists view
- **THEN** the view's title block, controls, and tree show under what the playback panel shows

#### Scenario: The view closes

- **GIVEN** an item plays and the Playlists view shows
- **WHEN** the view closes
- **THEN** the now-playing title shows under the art again and the queue list shows under it

### Requirement: The Queue column keeps its states while the Playlists view shows

While the Playlists view is open, the Queue column SHALL keep its collapse, queue-fold, and compact-strip states and their toggles. In the queue-fold state the view SHALL show in place of the folded line. In the compact-strip state the view SHALL show under the strip. A press on the Playlists control or the `playlists_open` action while the column is collapsed to its rail SHALL expand the column and show the view.

#### Scenario: The queue is folded

- **GIVEN** the queue is folded to its one-line summary
- **WHEN** the user opens the Playlists view
- **THEN** the view shows in place of the folded line
- **AND** closing the view shows the folded line again

#### Scenario: The window is short

- **GIVEN** the Queue column shows the compact strip
- **WHEN** the user opens the Playlists view
- **THEN** the strip stays and the view shows under it

#### Scenario: The column is collapsed to its rail

- **GIVEN** the Queue column is collapsed to its rail and the Playlists view is closed
- **WHEN** the user presses the Playlists control
- **THEN** the column expands and shows the Playlists view

#### Scenario: The column was collapsed while the view was open

- **GIVEN** the Playlists view is open and the user collapsed the Queue column to its rail
- **WHEN** the user presses the Playlists control
- **THEN** the column expands and the Playlists view shows

### Requirement: Play, Shuffle, and Add to queue close the Playlists view

A Play, Shuffle, or Add to queue on a playlist or one of its items SHALL close the Playlists view, from its controls, its right-click menu, or an item's double-click, so the queue list shows. A Play or Shuffle SHALL close the view when the queue replacement commits; a cancelled replacement prompt SHALL leave the view open. Rename, Refresh, Delete, and opening or closing a playlist SHALL leave the view open.

#### Scenario: A playlist plays into an empty queue

- **GIVEN** the queue is empty and the Playlists view shows a picked playlist
- **WHEN** the user presses Play
- **THEN** the queue holds the playlist's playable items and the Queue column shows the queue list

#### Scenario: The replacement prompt is cancelled

- **GIVEN** the queue holds items and the Playlists view shows a picked playlist
- **WHEN** the user presses Shuffle and cancels the replacement prompt
- **THEN** the queue is unchanged and the Playlists view stays open

#### Scenario: An item is added to the queue

- **GIVEN** a playlist is open in the Playlists view
- **WHEN** the user picks Add to queue from an item's right-click menu
- **THEN** the item is appended to the queue and the Queue column shows the queue list

#### Scenario: An item is double-clicked

- **GIVEN** a playlist is open in the Playlists view
- **WHEN** the user double-clicks one of its items
- **THEN** the playlist plays from that item and the Queue column shows the queue list

#### Scenario: A playlist is renamed

- **WHEN** the user renames a playlist in the Playlists view
- **THEN** the Playlists view stays open

### Requirement: The Library panel stays beside the Playlists view

While the Playlists view shows, the Library panel SHALL show beside the Queue column as it does with the queue list, and its keys SHALL act as they do then. A press on a destination SHALL make that destination active and SHALL leave the Playlists view open. The top bar SHALL mark the active destination and the Playlists control together.

#### Scenario: A tab is pressed while the view shows

- **GIVEN** the arrangement is Narrow and the Playlists view shows
- **WHEN** the user presses the Music tab
- **THEN** Music shows in the Library panel and the Playlists view stays in the Queue column
- **AND** the top bar marks the Music tab and the Playlists control

#### Scenario: Library keys act while the view shows

- **GIVEN** the Playlists view shows and the pointer is over a Library list
- **WHEN** the user presses a Library list key
- **THEN** the Library list acts on it as it does while the queue list shows

### Requirement: Queue row keys do not act on the hidden queue list

While the Playlists view shows in place of the queue list, a key pressed with the pointer over the view SHALL NOT act on a queue row.

#### Scenario: A row key over the view

- **GIVEN** the Playlists view shows and the pointer is over its tree
- **WHEN** the user presses the key that removes the selected queue row
- **THEN** the queue is unchanged

### Requirement: Opening the Playlists view in Mini shows the Queue column

In the Mini arrangement, opening the Playlists view SHALL show the Queue column. A press on the Playlists control while Mini shows the Library and the view is open SHALL show the Queue column and SHALL leave the view open.

#### Scenario: The view opens in Mini

- **GIVEN** the arrangement is Mini and the Library shows
- **WHEN** the user presses the Playlists control while the view is closed
- **THEN** the Queue column shows with the Playlists view

#### Scenario: The view is open behind the Library in Mini

- **GIVEN** the arrangement is Mini, the Playlists view is open, and the user pressed a destination tab so the Library shows
- **WHEN** the user presses the Playlists control
- **THEN** the Queue column shows with the Playlists view still open

### Requirement: The Playlists view offers the TUI's playlist actions

The Playlists view SHALL list the saved Emby playlists as a tree, SHALL let the user open one playlist at a time to its items, and SHALL let the user play, shuffle, or add a playlist or one item, rename and delete a playlist, and refresh, with the same queue effects as the TUI's Playlists sidebar. A play that replaces a populated queue SHALL ask as the TUI's does. A delete SHALL ask for confirmation.

#### Scenario: Playing a playlist

- **WHEN** the user plays a playlist
- **THEN** the queue is replaced with the playlist's playable items, after the TUI's populated-queue prompt where it applies

#### Scenario: Renaming a playlist

- **WHEN** the user renames a playlist and confirms the new name
- **THEN** the playlist has the new name on the server and in the tree

#### Scenario: Deleting a playlist

- **WHEN** the user presses Delete on a playlist
- **THEN** a confirmation shows, and the playlist is deleted only when the user confirms

## MODIFIED Requirements

### Requirement: The list header names a Service destination by its Service mark

The GUI Library list header of a Service destination SHALL show the destination's Service mark at the left of its title line and SHALL show no text title. Emby destinations SHALL show the Emby mark. Audiobookshelf destinations SHALL show the Audiobookshelf mark. Home SHALL keep its text title. The mark SHALL NOT change the rest of the header.

#### Scenario: A plain Emby library shows the Emby mark

- **WHEN** the user opens a Movies library tab
- **THEN** the list header shows the Emby mark and no "Movies" title text

#### Scenario: An Emby homevideos feed view shows the Emby mark

- **WHEN** the user opens an Emby library tab in feed-group mode whose content comes from YouTube
- **THEN** the list header shows the Emby mark, not the YouTube mark

#### Scenario: Music shows the Emby mark

- **WHEN** the user opens a Music destination
- **THEN** the list header shows the Emby mark and no "Artists" title text, and the collection dropdown stays at the right end of the line

#### Scenario: Shows shows the Emby mark under every view

- **WHEN** the user picks New in the Shows view dropdown
- **THEN** the list header shows the Emby mark, no title text repeats the view, and the view dropdown stays at the right end of the line

#### Scenario: Audiobookshelf destinations show the Audiobookshelf mark

- **WHEN** the user opens an Audiobooks or a Podcasts destination
- **THEN** the list header shows the Audiobookshelf mark and no title text

#### Scenario: Home keeps its title

- **WHEN** the user opens Home
- **THEN** the list header shows the "Home" title and no Service mark

### Requirement: GUI tree parent rows open and close on double-click

A GUI tree parent row is a row that opens to child rows in the same list: a Music artist or album, a Shows series or season in the All view, and a Playlists view playlist. On every screen, a press on a parent row SHALL pick it and SHALL NOT open or close it. A double-click on a parent row SHALL open it when closed and close it when open, and SHALL NOT play it.

#### Scenario: A series is double-clicked

- **GIVEN** the Shows list shows the All view with a closed series row
- **WHEN** the user double-clicks the series row
- **THEN** the series opens and its season rows show under it
- **AND** the Hero shows the series

#### Scenario: A series is pressed

- **GIVEN** a series row is open
- **WHEN** the user presses the series row once
- **THEN** the series stays open and the Hero shows the series

#### Scenario: An open artist is double-clicked

- **GIVEN** an artist row is open
- **WHEN** the user double-clicks the artist row
- **THEN** the artist closes and its album rows no longer show

#### Scenario: A playlist is double-clicked

- **GIVEN** the Playlists view shows a closed playlist row and the queue holds other items
- **WHEN** the user double-clicks the playlist row
- **THEN** the playlist opens and its item rows show under it
- **AND** the queue does not change and nothing starts to play

#### Scenario: A playlist plays from its Play control

- **GIVEN** the Playlists view has a playlist row picked
- **WHEN** the user presses the playlist's Play control or the Play entry of its right-click menu
- **THEN** the playlist plays

#### Scenario: A node the filter opened is double-clicked

- **GIVEN** the Library filter shows an artist row open because one of its albums matches
- **WHEN** the user double-clicks the artist row
- **THEN** the artist closes while the filter text stays
