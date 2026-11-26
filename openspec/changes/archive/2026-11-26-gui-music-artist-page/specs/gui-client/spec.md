# Spec Delta

## ADDED Requirements

### Requirement: A picked Music artist shows an artist page

A press on a Music artist row SHALL pick the artist, and the Hero SHALL show the artist page for it. The press SHALL NOT open or close the row. The artist page SHALL paint in the Hero frame: its header band SHALL show the artist name and the plain facts (the selected collections, the artist's album count, the span of the artist's release years, and the artist's track count and total time once every album of the artist has loaded its tracks). Its detail block SHALL show the artist's artwork and the actions Play, Add to queue, and Shuffle artist. Its list band SHALL show the artist's tracks grouped by album, in the order of the albums in the list and then in disc and track order, with one label for each album that shows the album's name and release year. The artist page SHALL offer no More actions control.

Play and Add to queue SHALL act on the whole artist: every album of the artist that the list holds, in the list's album order. Shuffle artist SHALL act as it does on an album. A double-click on a track row of the page, its hover Play control, or Enter while the pointer rests on it SHALL replace the queue and start playback from that track through the rest of the tracks that the page shows, in the page's order, and it SHALL NOT stop at the end of the track's album. A track row's hover Add to queue control SHALL append only that track. A press on a track row, and a press on an album label, SHALL change nothing.

The artist page SHALL load the tracks of every album of the artist when the artist is picked, and SHALL show each album's group as its tracks arrive. An album whose tracks have not loaded SHALL show its label and no rows, and SHALL NOT be part of a played sequence. The artist's row SHALL stay marked in the list while the Hero shows its page. A press on an album row or a track row of the list SHALL end the artist page and show that album. The artist page SHALL NOT be saved in the launch location.

#### Scenario: An artist row is pressed

- **GIVEN** the Hero shows an album
- **WHEN** the user presses a Music artist row
- **THEN** the Hero shows the artist page for that artist, and the artist row is marked
- **AND** the artist row does not open or close

#### Scenario: The page fills in as the tracks load

- **GIVEN** the user presses an artist whose albums have not loaded their tracks
- **WHEN** the tracks of the albums arrive
- **THEN** each album's group shows its tracks as they arrive, and the track count and total time show in the header band once every album has loaded

#### Scenario: A track is played from the artist page

- **GIVEN** the artist page shows three loaded albums
- **WHEN** the user double-clicks the first track of the second album
- **THEN** the queue is replaced and playback starts at that track
- **AND** the queue holds that track, the rest of the second album, and every track of the third album, in order

#### Scenario: Enter plays the hovered artist page track

- **WHEN** the pointer rests on a track row of the artist page and the user presses Enter
- **THEN** the track plays as a double-click on the row plays it

#### Scenario: Play acts on the whole artist

- **WHEN** the artist page shows an artist with two albums and the user presses Play
- **THEN** the queue is replaced with every track of the first album followed by every track of the second album, and playback starts at the first track

#### Scenario: Add to queue acts on the whole artist

- **WHEN** the artist page shows an artist with two albums and the user presses Add to queue
- **THEN** every track of the first album followed by every track of the second album is appended to the queue, and the queue's current item does not change

#### Scenario: An album row ends the artist page

- **GIVEN** the Hero shows an artist page
- **WHEN** the user presses an album row of the list
- **THEN** the Hero shows that album

#### Scenario: A track row of the list ends the artist page

- **GIVEN** the Hero shows an artist page
- **WHEN** the user presses a track row of the list
- **THEN** the Hero shows the album of that track

#### Scenario: A selection that drops the artist

- **GIVEN** the Hero shows the artist page of an artist who is in the Hip-Hop group only
- **WHEN** the user checks only Soul
- **THEN** the Hero shows an album, as it does for any Music selection

#### Scenario: A selection that keeps the artist

- **GIVEN** the Hero shows the artist page of an artist who is in the Hip-Hop and Soul groups
- **WHEN** the user checks only Hip-Hop and then also checks Soul
- **THEN** the Hero still shows the artist page

#### Scenario: The artist page is not kept across a restart

- **GIVEN** the GUI Client quit while the Hero showed an artist page
- **WHEN** the GUI Client starts again and the user opens Music
- **THEN** the Hero shows the kept album of the group, and no artist page

#### Scenario: The overlay shows the artist page

- **GIVEN** the arrangement is Narrow
- **WHEN** the user presses a Music artist row and then chooses View Detail from the row's menu
- **THEN** the Hero overlay opens and shows the artist page

## MODIFIED Requirements

### Requirement: The Music list is the canvas tree

The Music list SHALL show the artists of the selected collection as a tree of artist rows, album rows, and track rows, with a letter heading above the first artist of each letter. A press on an artist row SHALL pick the artist and the Hero SHALL show its artist page, as the requirement "A picked Music artist shows an artist page" states. A press on an album row SHALL pick the album. Neither press SHALL open or close its row. A double-click on an artist or album row SHALL open or close it, as every GUI tree parent row does. A press on a track row SHALL pick its album. A double-click on a track row SHALL play that track as the Enter play does.

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

#### Scenario: An artist row is pressed

- **GIVEN** an artist row is closed
- **WHEN** the user presses the artist row once
- **THEN** the Hero shows the artist page
- **AND** the artist row stays closed

#### Scenario: The rows show their facts

- **WHEN** the list paints an album row and a track row
- **THEN** the album row shows the album's year and the track row shows the track number

#### Scenario: The list is loading or empty

- **WHEN** the selected collection is loading or holds no artists
- **THEN** the list paints a loading state or an empty text

### Requirement: The Hero shows the picked item with its actions

The Hero SHALL show the picked album with its artist header, its square artwork, and its tracklist, or, while an artist is picked, the artist page that the requirement "A picked Music artist shows an artist page" states. It SHALL offer Play, Add to queue with the key hint `a`, Shuffle artist, and More actions. Play SHALL replace the queue and start the item without the TUI's populated-queue replace-confirm. The saved-playlist save/discard prompt SHALL still present, through the GUI's confirmation overlay. More actions SHALL open the item's menu. A tracklist row SHALL show the hover controls that the requirement "GUI Library rows show hover controls" defines, and a double-click on a tracklist row SHALL play that track, as it does in every Workspace list (Shows episodes, Audiobooks chapters, Feeds entries). The tracklist row of the track playing at the playhead SHALL show a play glyph beside its title. A highlight marks focus only, so the playing row SHALL NOT paint a green fill.

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

The Hero of Music, Shows, Podcasts, Audiobooks, and Feeds SHALL paint one frame: a header band, a detail block beside or above the art, and a list band. The header band's title SHALL be the name of the largest unit: the artist, the series, the podcast, the feed, or, for a book, the book. The detail block SHALL show the selected smaller unit (the album, the season or episode, the episode, the entry) with its duration and progress, its Play and Add to queue actions, and its overview. The Music artist page SHALL be the one Hero whose detail block shows the largest unit: it has no smaller unit, so the block shows the artist's artwork and the actions. Plain facts with no role SHALL sit on one line under the header title. A destination SHALL map its facts onto this frame and SHALL NOT remove a band, add a layout, or paint its own copy of a part. Home and Movies are not yet on the frame.

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

### Requirement: Each Music group keeps its last picked album

Each Music group SHALL keep the album its Hero last showed while it was the only checked group, across a restart too. An artist page SHALL NOT replace the kept album: the group keeps the album last shown before the artist page, and a restart shows that album. When the selection becomes one group, the Hero SHALL show its kept album, else its first album. For All or several groups, the Hero SHALL keep its album while the union holds it, else show the union's first album. The Hero SHALL never be blank while the albums are loaded.

#### Scenario: A group reopens on its own album

- **GIVEN** the user picked Paul's Boutique with only Hip-Hop checked, and then checked only Soul
- **WHEN** the user checks only Hip-Hop again
- **THEN** the Hero shows Paul's Boutique

#### Scenario: A second group keeps the Hero

- **GIVEN** only Hip-Hop is checked and the Hero shows Paul's Boutique
- **WHEN** the user also checks Soul
- **THEN** the Hero still shows Paul's Boutique

#### Scenario: The kept albums survive a restart

- **GIVEN** the GUI Client quit with Paul's Boutique kept under Hip-Hop and only Hip-Hop checked
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Hip-Hop checked, and the Hero shows Paul's Boutique

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the Search field on the left, and the location on the right: the destination, All or the names of the checked collections, and the picked item. The Music location names the picked album's title, or, while the Hero shows the artist page, the artist's name. The bar SHALL show no key hints. While prefix mode is armed, the armed state SHALL paint after the Search field, and the field and the location SHALL stay. The bar SHALL show no toast.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop checked and the album Licensed To Ill picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Hip-Hop, Licensed To Ill, and no key hints

#### Scenario: Music shows the artist page

- **GIVEN** Music is active with Hip-Hop checked and the Hero shows the Beastie Boys artist page
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Hip-Hop, Beastie Boys, and no key hints

#### Scenario: Two collections are checked

- **GIVEN** Feeds is active with Tifo Football and Off Menu checked
- **WHEN** the Library Status bar paints
- **THEN** the location names Feeds, then both subscriptions, then the picked entry

#### Scenario: A toast arrives

- **WHEN** a toast is presented while the Library panel shows
- **THEN** the Library Status bar does not show it and does not change
