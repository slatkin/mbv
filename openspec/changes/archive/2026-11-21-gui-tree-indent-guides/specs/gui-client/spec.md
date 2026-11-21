# Spec Delta

## ADDED Requirements

### Requirement: An open GUI tree parent shows an indent guide

A GUI tree parent row (a Music artist or album, a Shows series or season in the All view, a Playlists playlist) that is open and shows at least one child row SHALL show one vertical indent guide. The guide SHALL start under the parent row's leading element and SHALL run without a break through every row of the parent's subtree, to the bottom of its last row. A closed parent, and an open parent that shows no child row, SHALL show no guide.

#### Scenario: An open season shows its guide

- **GIVEN** the Shows All view shows an open series with an open season that shows episode rows
- **WHEN** the list paints
- **THEN** a guide starts under the series row's art and runs through every season and episode row of the series
- **AND** a second guide starts under the open season's label and runs through its episode rows, ending at the bottom of its last episode row

#### Scenario: A closed child shows only its parent's guide

- **GIVEN** an artist row is open and one of its album rows is closed
- **WHEN** the list paints
- **THEN** the closed album row carries the artist's guide and starts no guide of its own

#### Scenario: An open album whose tracks are loading

- **GIVEN** an album row is open and its tracks have not loaded
- **WHEN** the list paints
- **THEN** the album row starts no guide
- **AND** its guide shows when its first track row paints

#### Scenario: An open playlist shows its guide

- **GIVEN** the Playlists tree shows an open playlist with item rows
- **WHEN** the tree paints
- **THEN** a guide starts under the playlist row's label and runs through its item rows

#### Scenario: A flat list shows no guide

- **WHEN** a Home, Emby, Audiobooks, Podcasts, Feeds, Shows New, or Shows Upcoming list paints
- **THEN** no row shows an indent guide

#### Scenario: A scrolled list keeps the guide whole

- **GIVEN** an open series' subtree is longer than the list's viewport
- **WHEN** the user scrolls the list so the series row is out of view
- **THEN** every season and episode row in view still shows the series' guide without a break

### Requirement: A nested parent's guide stands apart from a top-level guide

A guide under a nested parent row (a Music album, a Shows season) SHALL paint in the accent tone. A guide under a top-level parent row (a Music artist, a Shows series, a Playlists playlist) SHALL paint in a neutral tone that does not draw the eye.

#### Scenario: Two levels of guides

- **GIVEN** an open artist with an open album that shows track rows
- **WHEN** the list paints a track row
- **THEN** the track row shows the artist's guide in the neutral tone and the album's guide in the accent tone

#### Scenario: A playlist guide

- **WHEN** an open playlist shows its item rows
- **THEN** the playlist's guide paints in the neutral tone

### Requirement: GUI tree row content sits clear of the guides

A GUI tree child row SHALL place its content (the number, the label, the trailing text) to the right of every guide that passes through it. No guide SHALL cross a row's text. A guide SHALL stay visible over a picked or hovered row's fill.

#### Scenario: An episode row under two guides

- **WHEN** an episode row of an open season paints
- **THEN** its number, title, and runtime sit to the right of both guides

#### Scenario: A selected episode row

- **GIVEN** an episode row under an open season is the selected row
- **WHEN** the list paints
- **THEN** both guides show over the episode row's fill

#### Scenario: A hovered track row

- **WHEN** the pointer rests on a track row under an open album
- **THEN** both guides show over the row's hover look
