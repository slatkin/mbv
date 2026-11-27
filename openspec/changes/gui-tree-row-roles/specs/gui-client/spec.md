# Spec Delta

## MODIFIED Requirements

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, the Hero's fact pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every Library list row and every Playlists screen row SHALL paint through one of three row parts, the flat row, the branch row, or the leaf row, and no list SHALL paint a row of its own. A row SHALL have one of three levels, and every row of one level SHALL look the same in every list, with the look of that level in the Music list:

- A top-level row (the Music artist row's look) is every row of a flat list and the top-level row of every tree list (a Music artist, a Shows series, a Playlists playlist). It SHALL paint through the flat row, with its art thumb (`library-row-artwork`) when the item has art.
- A branch row (the Music album row's look) is a Music album or a Shows season. It SHALL paint through the branch row.
- A leaf row (the Music track row's look) is a Music track, a Shows episode, or a Playlists item. It SHALL paint through the leaf row.

A destination SHALL supply the row's data, its state, its hook prefix, and its handlers, and SHALL NOT supply or restate the row's look. The list row parts and the Workspace table rows SHALL carry one play state, and the played title tone and the resume percent SHALL come from one derivation shared by both; no destination SHALL convert the play state into a second state of its own. A plain Emby library's row SHALL carry the play state its item carries, derived by the one shared Emby derivation the TUI's rows use, in plain mode and in feed-group mode alike. A row whose item carries a play state SHALL show its played or unplayed mark and look, as the requirement "Rows mark their played state" states; a row whose item carries no play state SHALL show neither. A Workspace table SHALL paint a header row, a number cell, a title, a duration, and a progress run. The row that plays at the playhead SHALL show as the playing row over its whole width, with its title emphasised and a playing mark in place of its number. A highlight that is not the playing row SHALL still mark focus only. An Audiobooks chapter row SHALL show its number when it is not the playing row.

#### Scenario: A list row looks the same in every destination

- **WHEN** a Home, Emby, Music artist, Shows series, Shows New, Shows Upcoming, Audiobooks, Podcasts, Feeds, or Playlists playlist row paints
- **THEN** it paints through the flat row with the same height, title, subtitle, trailing text, and selection fill as a Music artist row

#### Scenario: Continue watching shows the time left

- **WHEN** a Home Continue watching row paints
- **THEN** its trailing text is the time left in that item

#### Scenario: A nested row keeps its look

- **WHEN** a Shows season row, a Shows episode row, or a Playlists item row paints
- **THEN** a season paints through the branch row and an episode or a Playlists item paints through the leaf row, each without a thumb
- **AND** a season looks as a Music album row looks, and an episode or a Playlists item looks as a Music track row looks

#### Scenario: A played row mutes its title

- **GIVEN** a list row's item carries a play state
- **WHEN** the item is played
- **THEN** the row shows the played mark and the played look

#### Scenario: An Emby library row carries the shared play state

- **GIVEN** a plain Emby library row whose item is played, partly played with a resumable position, or not played
- **WHEN** the row paints, in plain mode or in feed-group mode
- **THEN** a played item's row shows the played mark and the played look
- **AND** a partly played item's row shows the unplayed mark and the unplayed look
- **AND** a row of an item that is not played shows the unplayed mark
- **AND** a collection row (a folder, a boxset, or a series) carries the same state by the same derivation
- **AND** a music item's row is ordinary, whatever its stored played or resume facts

#### Scenario: A row with no play state shows no mark

- **GIVEN** a Music track row or a Playlists item row, whose item carries no play state
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

## ADDED Requirements

### Requirement: An open GUI tree parent joins its child rows as one group

A GUI tree root row (a Music artist, a Shows series in the All view, a Playlists playlist) that is open SHALL show that it is open. When it shows at least one child row, those rows SHALL show as one group joined to the root, from its first child row to the last row of its subtree. A closed root, and the rows after a group, SHALL show as ordinary top-level rows.

#### Scenario: An open artist shows its group

- **GIVEN** the Music list shows an open artist with three album rows, one of them open with its track rows
- **WHEN** the list paints
- **THEN** the artist row shows that it is open
- **AND** the album rows and the track rows show as one group joined to the artist row, which ends at the last track or album row of the artist
- **AND** the next artist row shows as an ordinary top-level row

#### Scenario: A scrolled list keeps the group whole

- **GIVEN** an open series whose group is taller than the list
- **WHEN** the user scrolls so that only rows inside the group are in view
- **THEN** every row in view still shows as part of the group, with no break between rows

#### Scenario: An open parent with no child rows shows no group

- **GIVEN** an open album whose tracklist has not loaded, or an open playlist with no items
- **WHEN** the list paints
- **THEN** the parent row shows that it is open and no group shows under it

#### Scenario: A flat list shows no group

- **WHEN** a Home, Emby, Audiobooks, Podcasts, Feeds, or Shows New or Upcoming list paints
- **THEN** no row shows as open and no group shows

### Requirement: The leaf rows of one parent sit in one well

The leaf rows that one open parent shows SHALL sit together in one recessed well inside the parent's group, apart from the branch rows around it, with a separator between two leaf rows and none above the first or below the last. In a tree with no branch level (the Playlists screen), the well SHALL sit directly under the open root. The well and its rows SHALL follow the list column's width, and a leaf row's text SHALL stay on one line.

#### Scenario: An open album's tracks sit in one well

- **GIVEN** an open album shows twelve track rows
- **WHEN** the list paints
- **THEN** the twelve track rows show inside one well under the album row
- **AND** a separator shows between each two track rows

#### Scenario: An open playlist's items sit in a well under the playlist

- **GIVEN** the Playlists screen shows an open playlist with items
- **WHEN** the screen paints
- **THEN** the item rows show inside one well directly under the playlist row, in the playlist's group

#### Scenario: The list column is narrowed

- **GIVEN** an open album shows a track whose title is longer than the well
- **WHEN** the user narrows the list column
- **THEN** the well narrows with the column and the track title stays on one line

### Requirement: Sibling branch rows are told apart by a swatch

Each branch row SHALL show a Branch swatch. The swatch's colour SHALL follow the row's position among the branch rows that the list shows under the same parent, so two adjacent sibling rows never show one colour. The colour SHALL NOT change when the list scrolls. A change of sort or filter MAY change it. The swatch SHALL carry no meaning of its own, and a branch row SHALL load no artwork. Root and leaf rows SHALL show no swatch.

#### Scenario: Adjacent albums show different swatches

- **GIVEN** an open artist shows four album rows
- **WHEN** the list paints
- **THEN** each album row shows a swatch, and no two adjacent album rows show the same swatch colour

#### Scenario: A scroll keeps the swatch colours

- **GIVEN** an open series shows twelve season rows and the list shows only some of them
- **WHEN** the user scrolls the list
- **THEN** each season row keeps the swatch colour it showed before the scroll

#### Scenario: A playlist tree shows no swatch

- **WHEN** the Playlists screen shows an open playlist
- **THEN** no row of the playlist's group shows a swatch

### Requirement: An open branch row shows that it is open

An open branch row SHALL look different from a closed branch row. A branch row SHALL show its secondary facts under its title: a Music album its year, and an open Music album also its track count and total time once its tracklist has loaded; an open Shows season its episode count once its episodes have loaded. The branch row the Hero shows SHALL stay marked while it is open.

#### Scenario: An album is opened

- **GIVEN** a closed album row whose tracklist has loaded
- **WHEN** the user double-clicks the album row
- **THEN** the album row shows that it is open
- **AND** its secondary facts show the year, the track count, and the total time

#### Scenario: An open album is the Hero's subject

- **GIVEN** the Hero shows an album and that album's row is open
- **WHEN** the list paints
- **THEN** the album row shows both that it is open and that it is the Hero's item

#### Scenario: An open season shows its episode count

- **GIVEN** an open season whose episodes have loaded
- **WHEN** the list paints
- **THEN** the season row's secondary facts show the episode count

## REMOVED Requirements

### Requirement: An open GUI tree parent shows an indent guide

**Reason**: The group of an open parent (requirement "An open GUI tree parent joins its child rows as one group") and the leaf well now show where a parent's rows start and stop. A vertical guide beside them repeats that fact.

**Migration**: None for users. The guide parts and their canvas tokens are deleted; the group and well requirements carry the scroll-whole scenario.

### Requirement: A nested parent's guide stands apart from a top-level guide

**Reason**: No guide remains. An open branch row shows its own open look (requirement "An open branch row shows that it is open").

**Migration**: None.

### Requirement: GUI tree row content sits clear of the guides

**Reason**: No guide remains to cross a row's text.

**Migration**: None.
