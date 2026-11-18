# Spec Delta

## ADDED Requirements

### Requirement: A collection dropdown selects the collections

Music, Podcasts, Feeds, and an Emby library in feed-group mode SHALL each show one collection dropdown in the list header. The dropdown SHALL offer All and one entry per collection: a Music group, a Podcast show, a Feeds subscription, or an Emby folder. The list SHALL show the de-duplicated union of the checked collections, or every collection while All is selected. No other destination SHALL show a collection dropdown.

#### Scenario: Collections per destination

- **WHEN** a destination with collections paints its list
- **THEN** the dropdown offers All and the Music groups, the Podcast shows, the Feeds subscriptions, or the folders of an Emby library in feed-group mode

#### Scenario: A destination without collections

- **WHEN** Shows, Audiobooks, or a plain Emby library paints its list
- **THEN** no collection dropdown shows
- **AND** Shows paints its own single-choice view dropdown, which is not a collection filter

#### Scenario: Two collections are checked

- **GIVEN** Hip-Hop and Soul are checked in Music, and one artist has albums in both
- **WHEN** the list paints
- **THEN** the list shows the artists of Hip-Hop and Soul sorted as one list, and that artist shows one time

#### Scenario: Space over the trigger plays or pauses

- **GIVEN** the dropdown is closed and the pointer rests on its trigger
- **WHEN** the user presses Space
- **THEN** playback plays or pauses and the selection does not change

### Requirement: The collection dropdown sits in the list header and no pill row remains

The GUI Library panel SHALL show no pill row and no selector row between the top bar and the list in any destination. The dropdown trigger SHALL sit in the list header on the title's line, at the right end of the line, and SHALL show that it opens a menu. The trigger is a rectangular button with the list filter field's corner radius and an outline in the accent colour, with a
chevron that points down while closed and up while open. The open dropdown SHALL open under its
trigger, over the list, with its right edge aligned with the trigger's right edge.

#### Scenario: Podcasts header

- **WHEN** the Podcasts destination paints
- **THEN** the list header's title line shows the list title at the left and the dropdown trigger at the right end, and nothing else
- **AND** no row of pills or selector controls shows between the top bar and the list

#### Scenario: No destination paints a pill row

- **WHEN** any destination paints, Home, Shows, and Audiobooks included
- **THEN** no pill row shows above the list

#### Scenario: The dropdown opens under the trigger

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens directly under the trigger, over the list rows, with its right edge aligned with the trigger's right edge

### Requirement: The selection is All or checked collections

The selection SHALL be All or a non-empty set of checked collections, never both. A press on an unchecked collection SHALL check it and clear All. A press on a checked collection SHALL uncheck it; when no collection stays checked, the selection SHALL become All. A press on All SHALL uncheck every collection. A destination SHALL start on All when it holds no saved selection.

#### Scenario: The first collection is checked

- **GIVEN** All is selected
- **WHEN** the user presses Tifo Football
- **THEN** only Tifo Football is checked and All is not

#### Scenario: A second collection is checked

- **GIVEN** Tifo Football is checked
- **WHEN** the user presses Off Menu
- **THEN** Tifo Football and Off Menu are checked

#### Scenario: The last collection is unchecked

- **GIVEN** only Tifo Football is checked
- **WHEN** the user presses Tifo Football
- **THEN** All is selected and no collection is checked

#### Scenario: All is pressed

- **GIVEN** Tifo Football and Off Menu are checked
- **WHEN** the user presses All
- **THEN** All is selected and no collection is checked

#### Scenario: A failed collection is fetched again on a press

- **GIVEN** a Music group whose albums failed to load
- **WHEN** the user presses All
- **THEN** the group's albums are requested again and the list shows every group's albums once they load
- **AND** a repaint alone does not request them again

#### Scenario: A checked collection no longer exists

- **GIVEN** Soul and Jazz are checked, and the library no longer has Soul
- **WHEN** the list paints
- **THEN** only Jazz is checked
- **AND** when the library has none of the checked collections, All is selected

### Requirement: The dropdown trigger names the selection

The closed dropdown SHALL show a trigger that names the selection: All, the one checked collection,
or the count of checked collections. A press on the trigger SHALL open the dropdown, or close it
while it is open.

#### Scenario: Two collections are checked

- **GIVEN** Tifo Football and Off Menu are checked
- **WHEN** the dropdown is closed
- **THEN** the trigger shows the count, for example "2 shows"

#### Scenario: The trigger opens the dropdown

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens with All and every collection, each with its check state

### Requirement: The open dropdown filters its collections and closes

The open dropdown SHALL show a filter field. Text in the field SHALL narrow the collection entries to the ones whose name matches, by the Library filter's match rule; All SHALL stay shown. A check change SHALL leave the dropdown open. A press outside the dropdown, a destination change, or Esc SHALL close it and clear the filter text. The dropdown SHALL bind no other key.

#### Scenario: The filter narrows the entries

- **GIVEN** the dropdown is open
- **WHEN** the user types `tifo`
- **THEN** the dropdown shows All and Tifo Football, and no other collection

#### Scenario: A check leaves the dropdown open

- **GIVEN** the dropdown is open
- **WHEN** the user presses a collection
- **THEN** its check state changes and the dropdown stays open

#### Scenario: A press outside closes the dropdown

- **GIVEN** the dropdown is open with the filter text `tifo`
- **WHEN** the user presses a Library row outside the dropdown
- **THEN** the dropdown closes and the next open shows an empty filter

### Requirement: Feed-based lists paint newest first

Podcasts, Feeds, and an Emby library in feed-group mode SHALL paint their lists with the most recent
row first; undated rows SHALL stay last. The GUI SHALL show no date order control; the core keeps
its `DateOrder` command.

#### Scenario: A folder paints newest first

- **WHEN** the user checks one folder of an Emby library in feed-group mode
- **THEN** the list shows that folder's items newest first

#### Scenario: No destination shows a date order control

- **WHEN** any destination paints its list header
- **THEN** no date order control shows

### Requirement: The collection selection persists per destination

Each destination SHALL keep its own selection while the GUI Client runs. Every collection dropdown selection and its date order, where one is held, SHALL persist across a restart through the same launch-location entry, keyed by collection target: Music groups, Podcast shows, Feeds subscriptions, or Emby feed-group folders. An entry written before targets existed SHALL load as Music. A destination with no saved selection SHALL start on All.

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

### Requirement: Each Music group keeps its last picked album

Each Music group SHALL keep the album its Hero last showed while it was the only checked group, across a restart too. When the selection becomes one group, the Hero SHALL show its kept album, else its first album. For All or several groups, the Hero SHALL keep its album while the union holds it, else show the union's first album. The Hero SHALL never be blank while the albums are loaded.

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

### Requirement: Shows selects one view

The GUI Shows destination's list header SHALL paint one dropdown with exactly three single-choice entries: All, New, and Upcoming, in that order. All SHALL be the default. A press on one entry SHALL replace the active entry and close the dropdown. The choice SHALL be a view of the destination and SHALL NOT be a collection set, a collection target, a collection filter, or a key binding. The TUI Client's Shows modes SHALL NOT change.

#### Scenario: Shows opens on All

- **WHEN** the user opens a Shows destination with no saved view
- **THEN** the list shows the library's series sorted by name with the letter rail, the Hero and Workspace are present, and the dropdown marks All

#### Scenario: One view replaces another

- **GIVEN** All is active and the view dropdown is open
- **WHEN** the user presses New
- **THEN** New is the only marked entry and the dropdown closes

### Requirement: Shows paints the New and Upcoming lists

Under New, the GUI Shows destination SHALL show the old Shows Latest episodes list, merged across every Emby TV library, newest first and de-duplicated by item id. A New row SHALL select into the Hero, play, and enqueue as a Recently added Emby row does. Under Upcoming, it SHALL show the old Shows Upcoming premieres list, merged across every Emby TV library, soonest first. An empty list SHALL show no rows. GUI Home SHALL NOT show either list. The TUI Client's Home SHALL NOT change.

#### Scenario: Two TV libraries have new episodes

- **GIVEN** two Emby TV libraries each have recent episodes
- **WHEN** the Shows destination paints New
- **THEN** the list shows the episodes of both libraries as one list, newest first

#### Scenario: A New row plays

- **WHEN** the user plays a New row
- **THEN** that episode plays as a Recently added episode row plays

#### Scenario: Upcoming has no premieres

- **WHEN** no Emby TV library has an upcoming premiere
- **THEN** the Shows destination's Upcoming view shows no rows

#### Scenario: Home has no relocated sections

- **WHEN** GUI Home paints
- **THEN** it shows only Continue watching and Recently added sections, with no Latest episodes or Upcoming section

### Requirement: Shows Upcoming rows are display-only

An Upcoming row in the Shows destination SHALL NOT select, play, or enqueue. A press on it SHALL change nothing, a play or enqueue key over it SHALL do nothing, and the Hero SHALL keep its item.

#### Scenario: An Upcoming row is pressed

- **GIVEN** the Shows Hero shows a series
- **WHEN** the user presses an Upcoming row
- **THEN** the Hero still shows that series and nothing plays

#### Scenario: Enter over an Upcoming row

- **WHEN** the pointer rests on an Upcoming row and the user presses Enter
- **THEN** nothing plays and the queue does not change

## MODIFIED Requirements

### Requirement: Open text fields and menus keep the keys

While the Search field, the Library filter, the Queue filter, or the collection dropdown's filter field is open for typing, it SHALL take the keys wherever the pointer is. While a core overlay is open or prefix mode is armed, every key SHALL go to the Client core. Esc SHALL close the open Library row menu, the item menu, the Hero overlay, the collection dropdown, or the Search results wherever the pointer is.

#### Scenario: The pointer moves while the user types

- **GIVEN** the Library filter is open
- **WHEN** the pointer moves over the Queue list box and the user types `bo`
- **THEN** the Library filter holds `bo`
- **AND** the Queue does not change

#### Scenario: Esc closes the row menu away from the pointer

- **GIVEN** a Library row's menu is open
- **WHEN** the pointer rests over the Queue list box and the user presses Esc
- **THEN** the row menu closes
- **AND** the Queue does not change

#### Scenario: The dropdown filter keeps the keys

- **GIVEN** the collection dropdown is open
- **WHEN** the pointer moves over the Queue list box and the user types `ti`
- **THEN** the dropdown filter holds `ti`
- **AND** the Queue does not change

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show a letter rail from A to Z. A letter with no rows SHALL be disabled. A press on a letter SHALL scroll the list to the first row whose sort name starts with that letter; it SHALL pick and select nothing. The rail's keyboard path SHALL be the list's `/` filter. A list sorted by date SHALL show no rail. An Emby library that is not a feed-group library SHALL show the rail and no collection dropdown: the rail replaces the TUI's letter-range pill row, as it does for Shows.

#### Scenario: A plain Emby library paints the rail and no pills

- **WHEN** a Movies library paints its list
- **THEN** the list is flat and sorted by name, it shows the letter rail, and no collection dropdown shows

#### Scenario: A letter is pressed

- **WHEN** the user presses the letter B on the rail of the Music artist list
- **THEN** the list scrolls so that the first artist whose sort name starts with B is fully visible
- **AND** the Hero does not change

#### Scenario: A letter without rows is pressed

- **WHEN** the user presses a disabled letter
- **THEN** the list does not change

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the key hints for the filter, play, play next, add to queue, and destination keys on the left, and the location on the right: the destination, All or the names of the checked collections, and the picked item. The hints SHALL name no pane or focus key. While prefix mode is armed or a toast is active, that SHALL replace the key hints, and the location SHALL stay.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop checked and a Beastie Boys album picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the key hints and the location Music, Hip-Hop, Beastie Boys

#### Scenario: Two collections are checked

- **GIVEN** Feeds is active with Tifo Football and Off Menu checked
- **WHEN** the Library Status bar paints
- **THEN** the location names Feeds, then both subscriptions, then the picked entry

#### Scenario: A toast arrives

- **WHEN** the shared Status bar projection holds a toast while the Library panel shows
- **THEN** the toast replaces the key hints until it ends, and the location stays

## REMOVED Requirements

### Requirement: One collection pill is selected at a time

**Reason**: The collection dropdown replaces the pill row, and its selection is All or several checked collections.
**Migration**: See "A collection dropdown selects the collections" and "The selection is All or checked collections". The Podcasts and Feeds Latest pills become All; the Podcasts state pills and the Emby feed-group New pill go away in the GUI. The Shows mode pills become the separate three-choice view dropdown of "Shows selects one view".

### Requirement: The pill selection persists per destination

**Reason**: Replaced by "The collection selection persists per destination", which saves the checked set of every collection target.
**Migration**: The GUI launch location's saved list holds each target's checked collections; an empty list means All, and an entry without a target loads as Music.

### Requirement: Each collection pill keeps its last picked album

**Reason**: Replaced by "Each Music group keeps its last picked album", which also covers All and several checked groups.
**Migration**: The saved picks keep their format.
