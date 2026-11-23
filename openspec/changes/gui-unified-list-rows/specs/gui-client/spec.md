# Spec Delta

## MODIFIED Requirements

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. The Library lists and the Playlists screen SHALL have two row types, a flat row and a tree row, and each type SHALL have one painter. Every top-level row of every Library list, flat or tree, Home included, SHALL paint through the flat row with its art thumb (`library-row-artwork`). Every nested child row of a tree list (the Music album and track rows, the Shows season and episode rows) and every Playlists screen row SHALL paint through the tree row. No list SHALL paint a row of its own. A list SHALL supply the row's data, its state, its hook prefix, and its handlers, and SHALL NOT restate or change the row type's look. A Workspace table SHALL paint a header row, a number or play-glyph cell, a title, a duration, and a progress run. An Audiobooks chapter row SHALL show its number.

#### Scenario: A list row looks the same in every destination

- **WHEN** a Home, Emby, Music artist, Shows series, Shows New, Shows Upcoming, Audiobooks, Podcasts, or Feeds top-level row paints
- **THEN** it paints through the flat row with the same thumb, title, subtitle, and trailing text

#### Scenario: Continue watching shows the time left

- **WHEN** a Home Continue watching row paints
- **THEN** its trailing text is the time left in that item

#### Scenario: A nested row keeps its look

- **WHEN** an open series' season and episode rows, an open artist's album and track rows, or the Playlists screen's playlist and item rows paint
- **THEN** they paint through the tree row, without a thumb, with the same title and trailing text look

## ADDED Requirements

### Requirement: Every GUI list row shows its item's play state the same way

Every flat row and tree row whose item tracks a play state SHALL show that state (unplayed, partly played, or played), and two rows in the same state SHALL look the same in every list. A played row SHALL be visibly dimmer than an unplayed row and SHALL show a played mark; an unplayed row SHALL show an unplayed mark. A partly played row SHALL look like an unplayed row and SHALL also show its progress. A row whose item tracks no play state SHALL show no mark and SHALL look like an unplayed row.

#### Scenario: Played and unplayed rows differ in every list

- **GIVEN** a Home, Podcasts, Feeds, Emby, Audiobooks, Shows, or Playlists list shows one played item and one unplayed item
- **WHEN** the list paints
- **THEN** the played row shows the played mark and the dimmed look
- **AND** the unplayed row shows the unplayed mark

#### Scenario: A partly played row keeps its progress

- **WHEN** a row of a partly played item paints
- **THEN** it shows the unplayed mark and the unplayed look
- **AND** it shows its progress

### Requirement: A GUI list row's play mark keeps the row's layout

Each play mark SHALL carry an accessible label that names its state and SHALL sit before the trailing text in a slot of one fixed width, which a row with no mark keeps empty.

#### Scenario: The trailing text does not move

- **WHEN** a row's item changes from unplayed to played
- **THEN** the row's trailing text stays where it was

#### Scenario: A collection row shows no mark

- **WHEN** a Music artist, Music album, Shows series, Shows season, Emby folder, or Playlists playlist row paints
- **THEN** it shows no played or unplayed mark

### Requirement: A GUI list row shows its item's current play state

A played or progress change to an item SHALL reach every GUI list row that shows that item, Home's Recently added rows included, without a new fetch.

#### Scenario: A Recently added row follows a later play

- **GIVEN** Home's Recently added section shows an unplayed item
- **WHEN** the item is played to its end, or marked played
- **THEN** its Recently added row shows the played state without a new fetch
