# Spec Delta

## MODIFIED Requirements

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every Library list row and every Playlists screen row SHALL paint through one of two row parts, the flat row or the tree row, and no list SHALL paint a row of its own. A row SHALL have one of three levels, and every row of one level SHALL look the same in every list, with the look of that level in the Music list:

- A top-level row (the Music artist row's look) is every row of a flat list and the top-level row of every tree list (a Music artist, a Shows series, a Playlists playlist). It SHALL paint through the flat row, with its art thumb (`library-row-artwork`) when the item has art.
- A nested parent row (the Music album row's look) is a Music album or a Shows season. It SHALL paint through the tree row.
- A leaf row (the Music track row's look) is a Music track, a Shows episode, or a Playlists item. It SHALL paint through the tree row.

A destination SHALL supply the row's data, its state, its hook prefix, and its handlers, and SHALL NOT supply or restate the row's look. A row whose item carries a play state SHALL paint its title in the muted tone when the item is played, and SHALL show no other played or unplayed mark. A Workspace table SHALL paint a header row, a number or play-glyph cell, a title, a duration, and a progress run. An Audiobooks chapter row SHALL show its number.

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
- **THEN** the row's title paints in the muted tone
- **AND** the row shows no played or unplayed mark
