# Spec Delta

## MODIFIED Requirements

### Requirement: The Hero shows the picked item with its actions

The Hero SHALL show the picked album with its artist header, its square artwork, and its tracklist, or, while an artist is picked, the artist page that the requirement "A picked Music artist shows an artist page" states. The album Hero's header band SHALL show the artist and the selected collections, and SHALL NOT show the artist's album count or the artist's track count and total time: those facts belong to the artist page. The album's detail block SHALL show the album's track count and total time as fact pills. It SHALL offer Play, Add to queue with the key hint `a`, Shuffle artist, and More actions. Play SHALL replace the queue and start the item without the TUI's populated-queue replace-confirm. The saved-playlist save/discard prompt SHALL still present, through the GUI's confirmation overlay. More actions SHALL open the item's menu. A tracklist row SHALL show the hover controls that the requirement "GUI Library rows show hover controls" defines, and a double-click on a tracklist row SHALL play that track, as it does in every Workspace list (Shows episodes, Audiobooks chapters, Feeds entries). The tracklist row of the track playing at the playhead SHALL show as the playing row, as the requirement "Library lists and Workspace tables paint through shared parts" states.

#### Scenario: The Hero paints a picked album

- **WHEN** the user picks the album Licensed to Ill
- **THEN** the artist header shows the artist and the selected collections, and no album count, artist track count, or artist total time
- **AND** the album block shows the year, the title, the track count and the time as fact pills, and the square artwork, above the tracklist

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

The Hero of Music, Shows, Podcasts, Audiobooks, Feeds, Home, and every plain Emby library (Movies included) SHALL paint one frame: a header band, a detail block beside or above the art, and the bands below it. The header band SHALL show the Hero's kind label, its title, and, except for the Music album totals this requirement names, its short facts as a row of fact pills. The header band's title SHALL be the name of the largest unit the item belongs to: the artist for Music, the series for a series or one of its episodes, the podcast for a podcast episode, the feed for a feed entry, the book for an audiobook, or the item itself for a movie or another item without a containing unit. A blurred glow made from the Hero's own artwork SHALL show behind the header band, and a Hero with no artwork SHALL show no glow. The detail block SHALL show the artwork and, beside it or under it when the Hero is narrow, the selected smaller unit's name when a larger unit took the title, its Play and Add to queue actions, and its labelled facts as label and value rows. The Music artist page SHALL be the one Hero whose detail block shows the largest unit: it has no smaller unit, so the block shows the artist's artwork and the actions.

Each fact SHALL show in one place only. The short facts (a year, a genre, a runtime or duration, a count, an external link) SHALL be fact pills, and the facts that need a label (a director, a full release date, a studio or network, an author, a publish date) SHALL be label and value rows. The Music album Hero is the exception to the header-facts rule: its track count and total time SHALL be fact pills in the detail block, not in the header. A full release date row MAY show while a pill shows the year. A fact pill that names an external link SHALL open that link when pressed.

Below the detail block, each present band SHALL have a titled section header: the overview when the item has one, the cast when the item has one, and the destination's Hero list when one is defined. Music, a Shows series, Audiobooks, and Feeds SHALL show their Hero list bands. Home, Podcasts, plain Emby libraries, and a flat Shows episode Hero SHALL NOT show a list band. The cast section SHALL show each person who is not a director with the person's photo from the Service, or the person's initials when the Service has no photo, and the person's name and role. A director SHALL show in a director row and not in the cast.

No Hero SHALL show playback progress: no progress bar, no resume position, and no remaining time. The queue handles playback, and Play SHALL still resume from the stored position.

A destination SHALL map its facts onto this frame and SHALL NOT remove a band this requirement gives it, add a layout, or paint its own copy of a part.

#### Scenario: A Podcasts episode's Hero

- **WHEN** the Hero shows an episode of a podcast
- **THEN** the header title is the podcast's name
- **AND** the detail block shows the episode's name, its actions, and its publish date row, and the header band shows its duration as a fact pill
- **AND** no Episodes list band shows in the Hero

#### Scenario: A flat episode uses its series or show as the Hero title

- **WHEN** a Home or flat Shows Hero shows an episode whose payload names a series or show
- **THEN** the header band's title is the series or show name
- **AND** the detail block shows the episode's name

#### Scenario: A Home feed entry uses its feed as the Hero title

- **WHEN** a Home Hero shows a feed entry whose payload names its feed
- **THEN** the header band's title is the feed name
- **AND** the detail block shows the entry's name

#### Scenario: A Feeds entry's Hero

- **WHEN** the Hero shows an entry of a feed
- **THEN** the header title is the feed's name
- **AND** the list band holds that feed's entries, newest first, with the selected entry marked

#### Scenario: A TV series' actions act on the selected season

- **WHEN** a series is selected and the user presses Play or Add to queue
- **THEN** the selected season is played or appended

#### Scenario: A movie's Hero

- **GIVEN** a movie with a year, two genres, a runtime, an IMDb link, a director, a release date, a studio, and a cast
- **WHEN** the Hero shows the movie
- **THEN** the header band shows the movie's title and fact pills for the year, each genre, the runtime, and the IMDb link
- **AND** the detail block shows the artwork, Play and Add to queue, and rows for the director, the release date, and the studio, and no runtime row
- **AND** an Overview section and a Cast section follow, and the cast does not hold the director

#### Scenario: A cast member with no photo

- **GIVEN** the Service holds no photo for one person of a movie's cast
- **WHEN** the Hero shows the movie
- **THEN** that person shows the person's initials where a photo would show, and every other person shows the person's photo

#### Scenario: A partly played item shows no progress

- **GIVEN** a movie or an audiobook was played to half its length
- **WHEN** the Hero shows it
- **THEN** the Hero shows no progress bar, no resume position, and no remaining time
- **AND** a press on Play starts the item from its stored position

#### Scenario: The Hero's artwork makes its glow

- **WHEN** the Hero shows an item whose artwork has loaded
- **THEN** a blurred glow made from that artwork shows behind the header band

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, the Hero's fact pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every Library list row and every Playlists screen row SHALL paint through one of two row parts, the flat row or the tree row, and no list SHALL paint a row of its own. A row SHALL have one of three levels, and every row of one level SHALL look the same in every list, with the look of that level in the Music list:

- A top-level row (the Music artist row's look) is every row of a flat list and the top-level row of every tree list (a Music artist, a Shows series, a Playlists playlist). It SHALL paint through the flat row, with its art thumb (`library-row-artwork`) when the item has art.
- A nested parent row (the Music album row's look) is a Music album or a Shows season. It SHALL paint through the tree row.
- A leaf row (the Music track row's look) is a Music track, a Shows episode, or a Playlists item. It SHALL paint through the tree row.

A destination SHALL supply the row's data, its state, its hook prefix, and its handlers, and SHALL NOT supply or restate the row's look. The list row parts and the Workspace table rows SHALL carry one play state, and the played title tone and the resume percent SHALL come from one derivation shared by both; no destination SHALL convert the play state into a second state of its own. A row whose item carries a play state SHALL show its played or unplayed mark and look, as the requirement "Rows mark their played state" states; a row whose item carries no play state SHALL show neither. A Workspace table SHALL paint a header row, a number cell, a title, a duration, and a progress run. The row that plays at the playhead SHALL show as the playing row over its whole width, with its title emphasised and a playing mark in place of its number. A highlight that is not the playing row SHALL still mark focus only. An Audiobooks chapter row SHALL show its number when it is not the playing row.

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
- **THEN** the whole row shows as the playing row, its title is emphasised, and the playing mark shows in place of its number
- **AND** the row's carried play state does not change that look, and the row shows no played or unplayed mark

#### Scenario: A playing episode or entry shows the playing mark

- **GIVEN** a Shows episode or Feeds entry in a Hero list is the one playing at the playhead
- **WHEN** its Workspace row paints
- **THEN** the row shows the playing mark in place of its number

#### Scenario: A partly played row with no percent stays ordinary

- **GIVEN** a Workspace row is partly played and carries no resume percent
- **WHEN** the row paints
- **THEN** the row paints no progress run and shows the unplayed mark
