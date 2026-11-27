# Spec Delta

## MODIFIED Requirements

### Requirement: Shows selects one view

The GUI Shows destination's list header SHALL paint one dropdown with exactly four single-choice entries: All, New, Upcoming, and Favourites, in that order. All SHALL be the default. A press on one entry SHALL replace the active entry and close the dropdown. The choice SHALL be a view of the destination and SHALL NOT be a collection set, a collection target, a collection filter, or a key binding. The saved launch state SHALL keep All, New, and Upcoming and SHALL restore Favourites as All. The TUI Client's Shows modes SHALL NOT change.

#### Scenario: Shows opens on All

- **WHEN** the user opens a Shows destination with no saved view
- **THEN** the list shows the library's series sorted by name with the letter rail, the Hero and Workspace are present, and the dropdown marks All

#### Scenario: One view replaces another

- **GIVEN** All is active and the view dropdown is open
- **WHEN** the user presses New
- **THEN** New is the only marked entry and the dropdown closes

#### Scenario: Favourites is the fourth entry

- **WHEN** the user opens the Shows view dropdown
- **THEN** it lists All, New, Upcoming, and Favourites, in that order

#### Scenario: Favourites does not survive a restart

- **GIVEN** Favourites is the active Shows view when mbv quits
- **WHEN** mbv starts again and opens the Shows destination
- **THEN** the dropdown marks All

### Requirement: A collection dropdown selects the collections

Music, Podcasts, Feeds, and an Emby library in feed-group mode SHALL each show one collection dropdown in the list header while the destination's view is All. The dropdown SHALL offer All and one entry per collection: a Music group, a Podcast show, a Feeds subscription, or an Emby folder. The list SHALL show the de-duplicated union of the checked collections, or every collection while All is selected. No other destination SHALL show a collection dropdown.

#### Scenario: Collections per destination

- **WHEN** a destination with collections paints its list
- **THEN** the dropdown offers All and the Music groups, the Podcast shows, the Feeds subscriptions, or the folders of an Emby library in feed-group mode

#### Scenario: A destination without collections

- **WHEN** Shows, Audiobooks, or a plain Emby library paints its list
- **THEN** no collection dropdown shows
- **AND** Shows and a plain Emby library paint their single-choice view dropdown, which is not a collection filter

#### Scenario: Two collections are checked

- **GIVEN** Hip-Hop and Soul are checked in Music, and one artist has albums in both
- **WHEN** the list paints
- **THEN** the list shows the artists of Hip-Hop and Soul sorted as one list, and that artist shows one time

#### Scenario: Space over the trigger plays or pauses

- **GIVEN** the dropdown is closed and the pointer rests on its trigger
- **WHEN** the user presses Space
- **THEN** playback plays or pauses and the selection does not change

### Requirement: The collection dropdown sits in the list header and no pill row remains

The GUI Library panel SHALL show no pill row and no selector row between the top bar and the list in any destination. The collection dropdown trigger SHALL sit in the list header on the title's line and SHALL show that it opens a menu. Where the destination paints a view dropdown, the collection dropdown trigger SHALL sit directly to the left of the view dropdown trigger; otherwise it SHALL sit directly to the left of the sort control where one paints, and at the right end of the line where none does. The open dropdown SHALL open under its trigger, over the list.

#### Scenario: Podcasts header

- **WHEN** the Podcasts destination paints
- **THEN** the list header's title line shows the Service mark at the left, the refresh button, the dropdown trigger, and the sort control at the right end, and nothing else
- **AND** no row of pills or selector controls shows between the top bar and the list

#### Scenario: A Music header paints both triggers

- **WHEN** the Music destination paints its All view
- **THEN** the collection dropdown trigger sits directly to the left of the view dropdown trigger

#### Scenario: A feed-group header paints both triggers

- **WHEN** an Emby library in feed-group mode paints its All view
- **THEN** the collection dropdown trigger sits directly to the left of the view dropdown trigger, and the sort control is the line's rightmost item

#### Scenario: No destination paints a pill row

- **WHEN** any destination paints, Home, Shows, and Audiobooks included
- **THEN** no pill row shows above the list

#### Scenario: The dropdown opens under the trigger

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens directly under the trigger, over the list rows

### Requirement: Each flat Library list has a sort control

A plain Emby library, Audiobooks, Podcasts, Feeds, an Emby library in feed-group mode, and the Shows New view SHALL each show one single-choice sort control in the list header. Audiobooks SHALL offer Title, Author, Newest, and Oldest. Every other flat list SHALL offer Title, Newest, and Oldest. Home, Music, the Shows All and Upcoming views, Search, and the Playlists screen SHALL show no sort control. A Favourites view SHALL show no sort control.

#### Scenario: A Movies library offers three sorts

- **WHEN** a Movies library paints its list
- **THEN** the list header shows the sort control with Title, Newest, and Oldest, and Title is active

#### Scenario: Audiobooks offers four sorts

- **WHEN** an Audiobooks destination paints its list
- **THEN** the sort control shows Title, Author, Newest, and Oldest, and Author is active

#### Scenario: Shows All shows no sort control

- **WHEN** the Shows destination paints its All view or its Upcoming view
- **THEN** no sort control shows

#### Scenario: Shows New shows the sort control

- **WHEN** the Shows destination paints its New view
- **THEN** the sort control shows Title, Newest, and Oldest, and Newest is active

#### Scenario: A Favourites view shows no sort control

- **WHEN** a Movies library paints its Favourites view
- **THEN** no sort control shows

### Requirement: The sort control is the rightmost item of the title line

The sort control SHALL sit on the list header's title line as its rightmost item. Where the title line holds a view dropdown trigger, the sort control SHALL sit directly to the right of that trigger; where it holds only a collection dropdown trigger, the sort control SHALL sit directly to the right of that trigger. Where both dropdowns paint, the collection dropdown trigger SHALL sit directly to the left of the view dropdown trigger. The sort control SHALL NOT add a row between the top bar and the list.

#### Scenario: Podcasts title line

- **WHEN** the Podcasts destination paints
- **THEN** the title line shows the Service mark at the left, then the refresh button, then the collection dropdown trigger, then the sort control at the right end

#### Scenario: A plain library title line

- **WHEN** a Movies library paints
- **THEN** the title line shows the Service mark at the left, then the refresh button, then the view dropdown trigger, then the sort control at the right end

#### Scenario: A feed-group library title line

- **WHEN** an Emby library in feed-group mode paints its All view
- **THEN** the title line shows the Service mark at the left, then the refresh button, then the collection dropdown trigger, then the view dropdown trigger, then the sort control at the right end

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show letter headings and a letter rail from A to Z. A flat list SHALL count as sorted by name exactly while its active sort is Title or Author. A letter with no rows SHALL be disabled. A press on a letter SHALL scroll the list to the first row whose sort key starts with that letter; it SHALL pick and select nothing. The rail's keyboard path SHALL be the list's `/` filter. A list sorted by date SHALL show no letter headings and no rail. A Favourites view SHALL show no letter headings and no rail. An Emby library that is not a feed-group library SHALL show no collection dropdown: the rail replaces the TUI's letter-range pill row, as it does for Shows.

#### Scenario: A plain Emby library paints the rail and no collection dropdown

- **WHEN** a Movies library paints its list sorted by Title
- **THEN** the list is flat and sorted by name, it shows the letter rail, no collection dropdown shows, and the view dropdown marks All

#### Scenario: A date sort hides the rail

- **WHEN** the user sorts a Movies library by Newest
- **THEN** the list shows no letter headings and no rail

#### Scenario: A title sort shows the rail on a feed list

- **WHEN** the user sorts Feeds by Title
- **THEN** the list shows letter headings and the rail over the entry titles

#### Scenario: A letter is pressed

- **WHEN** the user presses the letter B on the rail of the Music artist list
- **THEN** the list scrolls so that the first artist whose sort name starts with B is fully visible
- **AND** the Hero does not change

#### Scenario: A letter without rows is pressed

- **WHEN** the user presses a disabled letter
- **THEN** the list does not change

#### Scenario: A Favourites view shows no rail

- **WHEN** a Movies library paints its Favourites view
- **THEN** the list shows no letter headings and no rail

### Requirement: A GUI tree list header shows Expand all and Collapse all

The list header of the Music All view and of the Shows All view SHALL show an Expand all button and a Collapse all button on its title line, directly to the left of the line's first dropdown trigger. The Shows New, Upcoming, and Favourites views, the Music Favourites view, and every other Library list, SHALL show neither button. The buttons SHALL NOT add a row between the top bar and the list.

#### Scenario: Music title line

- **WHEN** the Music destination paints its All view
- **THEN** the title line shows the Service mark at the left, then the refresh button, Expand all, Collapse all, the collection dropdown trigger, and the view dropdown trigger at the right end

#### Scenario: Shows All title line

- **WHEN** the Shows destination paints its All view
- **THEN** Expand all and Collapse all sit directly to the left of the view dropdown trigger

#### Scenario: Shows New shows no tree buttons

- **WHEN** the Shows destination paints its New view
- **THEN** the title line shows no Expand all and no Collapse all button

#### Scenario: A flat list shows no tree buttons

- **WHEN** the Podcasts destination paints
- **THEN** the title line shows no Expand all and no Collapse all button

#### Scenario: Music Favourites shows no tree buttons

- **WHEN** the Music destination paints its Favourites view
- **THEN** the title line shows no Expand all and no Collapse all button

### Requirement: The list header names a Service destination by its Service mark

The GUI Library list header of a Service destination SHALL show the destination's Service mark at the left of its title line and SHALL show no text title. Emby destinations SHALL show the Emby mark. Audiobookshelf destinations SHALL show the Audiobookshelf mark. Home and the Playlists screen SHALL keep their text title. The mark SHALL NOT change the rest of the header.

#### Scenario: A plain Emby library shows the Emby mark

- **WHEN** the user opens a Movies library tab
- **THEN** the list header shows the Emby mark and no "Movies" title text

#### Scenario: An Emby homevideos feed view shows the Emby mark

- **WHEN** the user opens an Emby library tab in feed-group mode whose content comes from YouTube
- **THEN** the list header shows the Emby mark, not the YouTube mark

#### Scenario: Music shows the Emby mark

- **WHEN** the user opens a Music destination
- **THEN** the list header shows the Emby mark and no "Artists" title text, and the view dropdown stays at the right end of the line

#### Scenario: Shows shows the Emby mark under every view

- **WHEN** the user picks New in the Shows view dropdown
- **THEN** the list header shows the Emby mark, no title text repeats the view, and the view dropdown stays at the right end of the line

#### Scenario: Audiobookshelf destinations show the Audiobookshelf mark

- **WHEN** the user opens an Audiobooks or a Podcasts destination
- **THEN** the list header shows the Audiobookshelf mark and no title text

#### Scenario: Home keeps its title

- **WHEN** the user opens Home
- **THEN** the list header shows the "Home" title and no Service mark

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the Search field on the left, and the location on the right: the destination, All or the names of the checked collections, and the picked item. While a library's view is Favourites, the location SHALL name Favourites in place of the collection names. The Music location names the picked album's title, or, while the Hero shows the artist page, the artist's name. The bar SHALL show no key hints. While prefix mode is armed, the armed state SHALL paint after the Search field, and the field and the location SHALL stay. The bar SHALL show no toast.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop checked and the album Licensed To Ill picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Hip-Hop, Licensed To Ill, and no key hints

#### Scenario: Music shows the artist page

- **GIVEN** Music is active with Hip-Hop checked and the Hero shows the Beastie Boys artist page
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Hip-Hop, Beastie Boys, and no key hints

#### Scenario: A Favourites view names Favourites

- **GIVEN** Music is active with Hip-Hop checked and its Favourites view painting
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Favourites, and the picked favourite, and no collection name

#### Scenario: Two collections are checked

- **GIVEN** Feeds is active with Tifo Football and Off Menu checked
- **WHEN** the Library Status bar paints
- **THEN** the location names Feeds, then both subscriptions, then the picked entry

#### Scenario: A toast arrives

- **WHEN** a toast is presented while the Library panel shows
- **THEN** the Library Status bar does not show it and does not change

## ADDED Requirements

### Requirement: Every other Emby library destination offers a Favourites view

The GUI destination of every Emby library other than a Shows library, meaning a plain library such
as Movies and a Music library, SHALL paint a view dropdown in its list header with exactly two
single-choice entries: All and Favourites, in that order. All SHALL be the default and the choice
SHALL NOT be saved across a restart. Audiobookshelf and Feeds destinations SHALL paint no such
dropdown.

#### Scenario: Movies library

- **WHEN** the user opens a Movies destination
- **THEN** its list header shows a view dropdown that marks All and offers Favourites

#### Scenario: Music library

- **WHEN** the user opens a Music destination and presses Favourites
- **THEN** the list shows that library's favourite artists, albums, and songs and the dropdown marks Favourites

#### Scenario: Audiobookshelf library

- **WHEN** the user opens an Audiobookshelf destination
- **THEN** its list header shows no view dropdown

#### Scenario: Collection dropdown beside the view dropdown

- **GIVEN** a Music destination whose header paints a collection dropdown
- **WHEN** the user presses Favourites
- **THEN** the view dropdown marks Favourites and the collection dropdown is not painted
- **WHEN** the user presses All
- **THEN** both dropdowns paint and the earlier collection pick is kept

### Requirement: A Favourites view replaces the destination's list column

While a library's view is Favourites, the destination SHALL paint the Favourites list in the list
column in place of that destination's All list, and the Hero SHALL show the selected favourite.
Pressing All SHALL restore the All list with the selection and scroll it had before.

#### Scenario: Return to All

- **GIVEN** the user scrolled the Movies All list, then pressed Favourites
- **WHEN** the user presses All
- **THEN** the Movies list shows the same row selected at the same position as before

#### Scenario: Hero follows a favourite

- **WHEN** the user selects a favourite row
- **THEN** the Hero shows that item

#### Scenario: Hero follows a favourite series

- **WHEN** the user selects a favourite series row
- **THEN** the Hero shows that series
