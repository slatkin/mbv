# Spec Delta

## MODIFIED Requirements

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every Library list row and every Playlists screen row SHALL paint through one of two row parts, the flat row or the tree row, and no list SHALL paint a row of its own. A row SHALL have one of three levels, and every row of one level SHALL look the same in every list, with the look of that level in the Music list:

- A top-level row (the Music artist row's look) is every row of a flat list and the top-level row of every tree list (a Music artist, a Shows series, a Playlists playlist). It SHALL paint through the flat row, with its art thumb (`library-row-artwork`) when the item has art.
- A nested parent row (the Music album row's look) is a Music album or a Shows season. It SHALL paint through the tree row.
- A leaf row (the Music track row's look) is a Music track, a Shows episode, or a Playlists item. It SHALL paint through the tree row.

A destination SHALL supply the row's data, its state, its hook prefix, and its handlers, and SHALL NOT supply or restate the row's look. The list row parts and the Workspace table rows SHALL carry one play state, and the played title tone and the resume percent SHALL come from one derivation shared by both; no destination SHALL convert the play state into a second state of its own. A plain Emby library's row SHALL carry the play state its item carries, derived by the one shared Emby derivation the TUI's rows use, in plain mode and in feed-group mode alike. A row whose item carries a play state SHALL show its played or unplayed mark and look, as the requirement "Rows mark their played state" states; a row whose item carries no play state SHALL show neither. A Workspace table SHALL paint a header row, a number or play-glyph cell, a title, a duration, and a progress run, and the row that plays at the playhead SHALL keep its own playing mark beside its title. An Audiobooks chapter row SHALL show its number.

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
- **THEN** its title keeps the playing row's tone and its play glyph paints beside the title
- **AND** the row's carried play state does not change that tone, and the row shows no played or unplayed mark

#### Scenario: A partly played row with no percent stays ordinary

- **GIVEN** a Workspace row is partly played and carries no resume percent
- **WHEN** the row paints
- **THEN** the row paints no progress run and shows the unplayed mark
