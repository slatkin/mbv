# Spec Delta

## ADDED Requirements

### Requirement: Each flat Library list has a sort control

A plain Emby library, Audiobooks, Podcasts, Feeds, an Emby library in feed-group mode, and the Shows New view SHALL each show one single-choice sort control in the list header. Audiobooks SHALL offer Title, Author, Newest, and Oldest. Every other flat list SHALL offer Title, Newest, and Oldest. Home, Music, the Shows All and Upcoming views, Search, and the Playlists screen SHALL show no sort control.

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

### Requirement: The sort control is the rightmost item of the title line

The sort control SHALL sit on the list header's title line as its rightmost item. Where the title line holds a dropdown trigger, the collection dropdown or the Shows view dropdown, the sort control SHALL sit directly to the right of that trigger. The sort control SHALL NOT add a row between the top bar and the list.

#### Scenario: Podcasts title line

- **WHEN** the Podcasts destination paints
- **THEN** the title line shows the Service mark at the left, then the collection dropdown trigger, then the sort control at the right end

#### Scenario: A plain library title line

- **WHEN** a Movies library paints
- **THEN** the title line shows the Service mark at the left and the sort control at the right end

### Requirement: A press selects one sort

A press on a sort choice SHALL make it the only active choice and SHALL re-sort the list. The control SHALL bind no key. A sort change SHALL keep the Hero's pick and the filter text, and SHALL scroll the list to its top.

#### Scenario: Newest is pressed on a Movies library

- **GIVEN** a Movies library sorted by Title with a movie picked
- **WHEN** the user presses Newest
- **THEN** Newest is the only active choice, the list shows the most recently added movie first, and the Hero still shows the picked movie

#### Scenario: The filter survives a sort change

- **GIVEN** the list filter holds text
- **WHEN** the user presses another sort choice
- **THEN** the filter keeps its text and the re-sorted list shows only the rows that match it

### Requirement: Each sort orders rows by one key

Title SHALL order rows by title ascending: the Emby sort name, the book title, the episode or entry title, and on a Shows New row the show name and then the episode. Author SHALL order books by the author sort key and then the title. Newest SHALL put the latest date first and Oldest the earliest first. Emby rows and books SHALL use the date they were added to the server. Podcast episodes and Feeds entries SHALL use their publish date. A row with no date SHALL stay last in both date sorts.

#### Scenario: Oldest on a feed-group folder

- **WHEN** the user sorts an Emby feed-group folder by Oldest
- **THEN** the list shows the earliest added video first, and a video with no date shows last

#### Scenario: Newest on Audiobooks

- **WHEN** the user sorts Audiobooks by Newest
- **THEN** the list shows the most recently added book first

#### Scenario: Title on Podcasts

- **WHEN** the user sorts Podcasts by Title
- **THEN** the list shows the checked shows' episodes ordered by episode title

### Requirement: The list sort persists per destination

Each destination SHALL keep its own sort while the GUI Client runs and across a restart, keyed by the destination's stable identity. Two libraries of the same kind SHALL keep separate sorts. A destination with no saved sort SHALL open on its default: Title for a plain Emby library, Author for Audiobooks, and Newest for the others. A launch-location file from before this change SHALL load a saved feed-based date order as that destination's sort.

#### Scenario: Two Movies libraries keep separate sorts

- **GIVEN** the user sorts one Movies library by Newest and leaves a second Movies library on Title
- **WHEN** the user opens the second library
- **THEN** it shows Title active

#### Scenario: The GUI starts again

- **GIVEN** the GUI Client quit with a YouTube library sorted by Title
- **WHEN** the GUI Client starts again and the user opens that library
- **THEN** it shows Title active and the list is sorted by title

#### Scenario: An old file holds an Oldest date order

- **GIVEN** a launch-location file written before this change holds Oldest for a Podcasts library
- **WHEN** the GUI Client starts
- **THEN** that Podcasts library opens sorted by Oldest

## MODIFIED Requirements

### Requirement: The collection selection persists per destination

Each destination SHALL keep its own selection while the GUI Client runs. Every collection dropdown selection SHALL persist across a restart through the same launch-location entry, keyed by collection target: Music groups, Podcast shows, Feeds subscriptions, or Emby feed-group folders. An entry written before targets existed SHALL load as Music. A destination with no saved selection SHALL start on All. The list's sort SHALL persist by the requirement "The list sort persists per destination", not through this entry.

#### Scenario: The user changes destination and returns

- **GIVEN** Soul and Jazz are checked in Music
- **WHEN** the user opens another destination and then Music again
- **THEN** Soul and Jazz are still checked

#### Scenario: The GUI starts again

- **GIVEN** the GUI Client quit with Soul and Jazz checked in Music
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Soul and Jazz checked

#### Scenario: A saved collection no longer exists

- **GIVEN** the GUI Client quit with only Soul checked, and the library no longer has Soul
- **WHEN** the GUI Client starts again
- **THEN** Music opens on All

#### Scenario: Non-Music selections survive a restart

- **GIVEN** the GUI Client quit with a show checked in Podcasts, a subscription checked in Feeds, and a folder checked in an Emby feed-group library
- **WHEN** the GUI Client starts again
- **THEN** Podcasts, Feeds, and that Emby library open with those collections checked

#### Scenario: Saved ids restore to their own dropdown

- **GIVEN** a launch entry names the Feeds target with a saved subscription
- **WHEN** the GUI Client starts again
- **THEN** Feeds opens with that subscription checked and no other dropdown reads it

#### Scenario: An old launch entry loads as Music

- **GIVEN** the GUI Client quit with Soul and Jazz saved as Music groups before launch entries named a collection target
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Soul and Jazz checked

#### Scenario: A destination with no saved selection starts on All

- **GIVEN** the GUI Client quit with no saved Podcasts or Feeds selection
- **WHEN** the GUI Client starts again
- **THEN** Podcasts and Feeds open on All

#### Scenario: The Shows view survives a restart

- **GIVEN** the GUI Client quit with Upcoming selected in a Shows destination
- **WHEN** the GUI Client starts again
- **THEN** that Shows destination opens with Upcoming selected

#### Scenario: An old launch file has no Shows view

- **GIVEN** the GUI Client quit before the launch file held a Shows view
- **WHEN** the GUI Client starts again
- **THEN** Shows opens on All

### Requirement: The collection dropdown sits in the list header and no pill row remains

The GUI Library panel SHALL show no pill row and no selector row between the top bar and the list in any destination. The dropdown trigger SHALL sit in the list header on the title's line, at the right end of the line, and SHALL show that it opens a menu. Where the destination shows a sort control, the trigger SHALL sit directly to the left of it, and the sort control SHALL be the line's rightmost item. The open dropdown SHALL open under its trigger, over the list.

#### Scenario: Podcasts header

- **WHEN** the Podcasts destination paints
- **THEN** the list header's title line shows the Service mark at the left, the dropdown trigger, and the sort control at the right end, and nothing else
- **AND** no row of pills or selector controls shows between the top bar and the list

#### Scenario: No destination paints a pill row

- **WHEN** any destination paints, Home, Shows, and Audiobooks included
- **THEN** no pill row shows above the list

#### Scenario: The dropdown opens under the trigger

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens directly under the trigger, over the list rows

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show letter headings and a letter rail from A to Z. A flat list SHALL count as sorted by name exactly while its active sort is Title or Author. A letter with no rows SHALL be disabled. A press on a letter SHALL scroll the list to the first row whose sort key starts with that letter; it SHALL pick and select nothing. The rail's keyboard path SHALL be the list's `/` filter. A list sorted by date SHALL show no letter headings and no rail. An Emby library that is not a feed-group library SHALL show no collection dropdown: the rail replaces the TUI's letter-range pill row, as it does for Shows.

#### Scenario: A plain Emby library paints the rail and no collection dropdown

- **WHEN** a Movies library paints its list sorted by Title
- **THEN** the list is flat and sorted by name, it shows the letter rail, and no collection dropdown shows

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

### Requirement: Shows paints the New and Upcoming lists

Under New, the GUI Shows destination SHALL show the old Shows Latest episodes list, merged across every Emby TV library and de-duplicated by item id, in the order of its active sort; its default sort is Newest. A New row SHALL select into the Hero, play, and enqueue as a Recently added Emby row does. Under Upcoming, it SHALL show the old Shows Upcoming premieres list, merged across every Emby TV library, soonest first. An empty list SHALL show no rows. GUI Home SHALL NOT show either list. The TUI Client's Home SHALL NOT change.

#### Scenario: Two TV libraries have new episodes

- **GIVEN** two Emby TV libraries each have recent episodes
- **WHEN** the Shows destination paints New sorted by Newest
- **THEN** the list shows the episodes of both libraries as one list, newest first

#### Scenario: New sorted by Title

- **WHEN** the user sorts the New view by Title
- **THEN** the list orders the episodes by show name and then by episode

#### Scenario: A New row plays

- **WHEN** the user plays a New row
- **THEN** that episode plays as a Recently added episode row plays

#### Scenario: Upcoming has no premieres

- **WHEN** no Emby TV library has an upcoming premiere
- **THEN** the Shows destination's Upcoming view shows no rows

#### Scenario: Home has no relocated sections

- **WHEN** GUI Home paints
- **THEN** it shows only Continue watching and Recently added sections, with no Latest episodes or Upcoming section

## REMOVED Requirements

### Requirement: Feed-based lists paint newest first

**Reason**: The GUI now shows a sort control on every flat list. Newest stays the default of the feed-based lists, and the date sorts keep undated rows last.
**Migration**: See "Each flat Library list has a sort control", "Each sort orders rows by one key", and "The list sort persists per destination".
