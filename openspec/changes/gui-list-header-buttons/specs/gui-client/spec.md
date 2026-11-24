# Spec Delta

## ADDED Requirements

### Requirement: A GUI tree list header shows Expand all and Collapse all

The list header of the Music list and of the Shows All view SHALL show an Expand all button and a Collapse all button on its title line, directly to the left of the line's dropdown trigger. The Shows New and Upcoming views, and every other Library list, SHALL show neither button. The buttons SHALL NOT add a row between the top bar and the list.

#### Scenario: Music title line

- **WHEN** the Music destination paints
- **THEN** the title line shows the Service mark at the left, then the refresh button, Expand all, Collapse all, and the collection dropdown trigger at the right end

#### Scenario: Shows All title line

- **WHEN** the Shows destination paints its All view
- **THEN** Expand all and Collapse all sit directly to the left of the view dropdown trigger

#### Scenario: Shows New shows no tree buttons

- **WHEN** the Shows destination paints its New view
- **THEN** the title line shows no Expand all and no Collapse all button

#### Scenario: A flat list shows no tree buttons

- **WHEN** the Podcasts destination paints
- **THEN** the title line shows no Expand all and no Collapse all button

### Requirement: Expand all opens every top-level parent row the list shows

A press on Expand all SHALL open every top-level parent row that the list shows: every artist row in Music, every series row in Shows. It SHALL NOT open or close an album or a season row, and SHALL NOT pick a row or change the Hero. While the Library filter narrows the list, it SHALL act on the rows the filter keeps, and an expanded row SHALL show the children the filter keeps.

#### Scenario: Expand all in Music

- **GIVEN** the Music list shows closed artist rows and one open album
- **WHEN** the user presses Expand all
- **THEN** every artist row opens and shows its album rows
- **AND** the open album stays open and every closed album stays closed
- **AND** the Hero does not change

#### Scenario: Expand all under a filter

- **GIVEN** the Library filter narrows the Music list and the user closed an artist row that holds a match
- **WHEN** the user presses Expand all
- **THEN** that artist row opens again and shows its matching albums
- **AND** after the filter is cleared, every artist row that the filter kept is still open

#### Scenario: Expand all in Shows

- **GIVEN** the Shows All view shows closed series rows
- **WHEN** the user presses Expand all
- **THEN** every series row opens
- **AND** the series rows in view show their season rows when their seasons load, and no season row opens

### Requirement: Collapse all closes every open parent row

A press on Collapse all SHALL close every open parent row of the list at every level: artists and albums in Music, series and seasons in Shows. It SHALL NOT pick a row or change the Hero. While the Library filter narrows the list, every parent row the filter opened SHALL close too, and SHALL stay closed after the filter is cleared.

#### Scenario: Collapse all in Music

- **GIVEN** the Music list shows open artists and an open album
- **WHEN** the user presses Collapse all
- **THEN** only artist rows and letter headings show
- **AND** a later double-click on an artist shows its albums closed

#### Scenario: Collapse all under a filter

- **GIVEN** the Library filter opened an artist row because one of its albums matches
- **WHEN** the user presses Collapse all
- **THEN** the artist row closes while the filter text stays

### Requirement: An open Shows row in view loads its children

In the Shows All view, an open series row in view whose seasons are not loaded SHALL load them, and an open season row in view whose episodes are not loaded SHALL load them. The loads SHALL start without a pick and SHALL NOT change the Hero. Only a few such loads SHALL run at one time; the rest SHALL wait, and a row that leaves the view before its load starts SHALL NOT load.

#### Scenario: Scrolling after Expand all

- **GIVEN** the user pressed Expand all in a Shows library with many series
- **WHEN** the user scrolls the list
- **THEN** the open series rows that come into view show their seasons when the seasons load
- **AND** no load starts for a series that never came into view

#### Scenario: A row scrolled past

- **GIVEN** an open series row is waiting for its seasons to load
- **WHEN** the user scrolls the row out of view before its load starts
- **THEN** no load starts for that series

### Requirement: Every GUI Library list header shows a refresh button

The list header of every GUI Library destination SHALL show a refresh button on its title line: Home, Music, Shows in all three views, each plain Emby library, an Emby library in feed-group mode, Audiobooks, Podcasts, and Feeds. On a tree list it SHALL sit directly to the left of Expand all. On another list it SHALL sit directly to the left of the dropdown trigger, or of the sort control where there is no trigger, and it SHALL be the line's rightmost item where there is neither.

#### Scenario: Home title line

- **WHEN** Home paints
- **THEN** the refresh button is the rightmost item of the title line

#### Scenario: Shows Upcoming title line

- **WHEN** the Shows destination paints its Upcoming view
- **THEN** the refresh button sits directly to the left of the view dropdown trigger

### Requirement: A refresh reloads the list and keeps its rows until the new ones arrive

A press on the refresh button SHALL reload the shown destination's list from its Service. The list SHALL keep its rows until the new rows arrive. A failed refresh SHALL keep the rows the list showed. A refresh SHALL NOT change the pick, the filter, the scroll position, or which rows are open.

#### Scenario: A refresh brings a new item

- **GIVEN** a Movies library shows its rows and a movie was added on the server
- **WHEN** the user presses the refresh button
- **THEN** the rows stay on screen while the refresh runs
- **AND** the new movie shows when the refresh lands

#### Scenario: A failed refresh

- **GIVEN** a list shows its rows and the Service cannot be reached
- **WHEN** the user presses the refresh button
- **THEN** the rows stay on screen

#### Scenario: Shows New refresh

- **WHEN** the user presses the refresh button in the Shows New view
- **THEN** the latest-episodes list reloads and the view stays New

### Requirement: A tree refresh reloads the children of the open rows in view

In a tree list, a refresh SHALL also reload the children of each open parent row in view: the tracks of an open Music album, the seasons of an open Shows series, and the episodes of an open Shows season. A refresh SHALL start no load for the children of an open parent row that is out of view.

#### Scenario: A refresh of an open album in view

- **GIVEN** the Music list shows an open album with its tracks in view
- **WHEN** the user presses the refresh button
- **THEN** the album stays open and its tracks reload in place

#### Scenario: An open album out of view

- **GIVEN** the Music list holds an open album that is scrolled out of view
- **WHEN** the user presses the refresh button
- **THEN** no track load starts for that album

#### Scenario: A refresh of an open series in view

- **GIVEN** the Shows All view shows an open series with its seasons in view
- **WHEN** the user presses the refresh button
- **THEN** the series stays open and its seasons reload in place

## MODIFIED Requirements

### Requirement: The collection dropdown sits in the list header and no pill row remains

The GUI Library panel SHALL show no pill row and no selector row between the top bar and the list in any destination. The dropdown trigger SHALL sit in the list header on the title's line, at the right end of the line, and SHALL show that it opens a menu. Where the destination shows a sort control, the trigger SHALL sit directly to the left of it, and the sort control SHALL be the line's rightmost item. The open dropdown SHALL open under its trigger, over the list.

#### Scenario: Podcasts header

- **WHEN** the Podcasts destination paints
- **THEN** the list header's title line shows the Service mark at the left, the refresh button, the dropdown trigger, and the sort control at the right end, and nothing else
- **AND** no row of pills or selector controls shows between the top bar and the list

#### Scenario: No destination paints a pill row

- **WHEN** any destination paints, Home, Shows, and Audiobooks included
- **THEN** no pill row shows above the list

#### Scenario: The dropdown opens under the trigger

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens directly under the trigger, over the list rows

### Requirement: The sort control is the rightmost item of the title line

The sort control SHALL sit on the list header's title line as its rightmost item. Where the title line holds a dropdown trigger, the collection dropdown or the Shows view dropdown, the sort control SHALL sit directly to the right of that trigger. The sort control SHALL NOT add a row between the top bar and the list.

#### Scenario: Podcasts title line

- **WHEN** the Podcasts destination paints
- **THEN** the title line shows the Service mark at the left, then the refresh button, then the collection dropdown trigger, then the sort control at the right end

#### Scenario: A plain library title line

- **WHEN** a Movies library paints
- **THEN** the title line shows the Service mark at the left, then the refresh button, then the sort control at the right end
