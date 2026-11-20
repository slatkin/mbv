# Spec Delta

## MODIFIED Requirements

### Requirement: The Music list is the canvas tree

The Music list SHALL show the artists of the selected collection as a tree of artist rows, album rows, and track rows, with a letter heading above the first artist of each letter. A press on an artist or album row SHALL pick it and SHALL NOT open or close it. A double-click on an artist or album row SHALL open or close it, as every GUI tree parent row does. A press on a track row SHALL pick its album. A double-click on a track row SHALL play that track as the Enter play does.

#### Scenario: A track is double-clicked

- **WHEN** the user double-clicks a track row of the list
- **THEN** the queue is replaced and the track plays
- **AND** with autoload on the queue holds that track's album in disc and track order starting at the track, and with autoload off the track alone

#### Scenario: An album is opened

- **WHEN** the user double-clicks an album row whose tracks are not loaded
- **THEN** the album opens, its tracks appear when they load, and the Hero shows the album

#### Scenario: An album is pressed

- **GIVEN** an album row is closed
- **WHEN** the user presses the album row once
- **THEN** the Hero shows the album
- **AND** the album row stays closed

#### Scenario: A track row is pressed

- **WHEN** the user presses a track row
- **THEN** the Hero shows the track's album
- **AND** no row becomes selected

#### Scenario: The rows show their facts

- **WHEN** the list paints an album row and a track row
- **THEN** the album row shows the album's year and the track row shows the track number

#### Scenario: The list is loading or empty

- **WHEN** the selected collection is loading or holds no artists
- **THEN** the list paints a loading state or an empty text

## ADDED Requirements

### Requirement: GUI tree parent rows open and close on double-click

A GUI tree parent row is a row that opens to child rows in the same list: a Music artist or album, a Shows series or season in the All view, and a Playlists screen playlist. On every screen, a press on a parent row SHALL pick it and SHALL NOT open or close it. A double-click on a parent row SHALL open it when closed and close it when open, and SHALL NOT play it.

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

- **GIVEN** the Playlists screen shows a closed playlist row and the queue holds other items
- **WHEN** the user double-clicks the playlist row
- **THEN** the playlist opens and its item rows show under it
- **AND** the queue does not change and nothing starts to play

#### Scenario: A playlist plays from its Play control

- **GIVEN** the Playlists screen has a playlist row picked
- **WHEN** the user presses the playlist's Play control or the Play entry of its right-click menu
- **THEN** the playlist plays

#### Scenario: A node the filter opened is double-clicked

- **GIVEN** the Library filter shows an artist row open because one of its albums matches
- **WHEN** the user double-clicks the artist row
- **THEN** the artist closes while the filter text stays

### Requirement: A GUI tree parent row shows a double-click hint on hover

While the pointer rests on a GUI tree parent row, the row SHALL show a hint at its trailing end that names the double-click and what it does to the row now: expand when the row is closed, collapse when it is open. The hint SHALL take the place of the row's trailing text while it shows. A row that is not a parent row SHALL show no hint.

#### Scenario: The pointer rests on a closed album

- **WHEN** the pointer rests on a closed album row
- **THEN** the row shows the hint to double-click to expand, in place of the album's year

#### Scenario: The pointer rests on an open series

- **WHEN** the pointer rests on an open series row
- **THEN** the row shows the hint to double-click to collapse

#### Scenario: The pointer leaves the row

- **GIVEN** a parent row shows its hint
- **WHEN** the pointer leaves the row
- **THEN** the hint goes and the row's trailing text shows again

#### Scenario: A leaf row shows no hint

- **WHEN** the pointer rests on a track, episode, or playlist item row
- **THEN** the row shows no hint
