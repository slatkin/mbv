# Spec Delta

## Purpose

Lets the user favourite and unfavourite Emby items and see each Emby library's favourites in that
library, following the behaviour of the official Emby client.

## ADDED Requirements

### Requirement: Supported Emby items offer a favourites entry

A library row or Hero for an Emby item SHALL offer one favourites entry when the item supports
favourites. An item supports favourites when it carries user data and its type is none of Season,
Folder, CollectionFolder, UserView, Channel, Program, Timer, SeriesTimer, Studio, or Tag. The entry
SHALL read "Add to Favourites" for an item that is not a favourite and "Remove from Favourites" for
one that is.

#### Scenario: Movie

- **WHEN** the user opens the menu on a movie that is not a favourite
- **THEN** the menu offers Add to Favourites

#### Scenario: Series is a collection

- **WHEN** the user opens the menu on a TV series that is a favourite
- **THEN** the menu offers Remove from Favourites

#### Scenario: Music track outside a Grouped Music library

- **WHEN** the user opens the menu on a music track in a non-grouped music library or a mixed plain library
- **THEN** the menu offers Add to Favourites even though it offers no played-state entry

#### Scenario: Season

- **WHEN** the user opens the menu on a TV season
- **THEN** the menu offers no favourites entry

### Requirement: The favourites entry is single-row and library-scoped

The favourites entry SHALL NOT appear in a multi-selection menu, on a Queue row, or on an
Audiobookshelf or Feeds row. No row of a Grouped Music library SHALL offer it — artist, album,
and track rows alike: the grouped Music destination is excluded from favourites everywhere.
A music row outside a Grouped Music library, in a non-grouped music library, a mixed plain
library, or Home, SHALL keep the entry the item's type qualifies for.

#### Scenario: Queue row

- **WHEN** the user opens the menu on a Queue row for an Emby movie
- **THEN** the menu offers no favourites entry

#### Scenario: Multi-selection

- **WHEN** two Emby movies are selected and the user opens the menu
- **THEN** the menu offers no favourites entry

#### Scenario: Music artist row

- **WHEN** the user opens the menu on a Grouped Music tree artist row
- **THEN** the menu offers no favourites entry

#### Scenario: Music album row in the grouped tree

- **WHEN** the user opens the menu on an album row of a Grouped Music library
- **THEN** the menu offers no favourites entry

#### Scenario: Music track row in the grouped tree

- **WHEN** the user opens the menu on a track row of a Grouped Music library
- **THEN** the menu offers no favourites entry

### Requirement: Choosing the entry changes the Emby favourite state

Choosing the entry SHALL set or clear the item's favourite state on the Emby server for the
signed-in user. On success the item's state in mbv SHALL show the new value and its entry SHALL
flip. On failure mbv SHALL leave the state unchanged and SHALL show an error toast.

#### Scenario: Add succeeds

- **WHEN** the user chooses Add to Favourites on an unfavourited movie and Emby accepts it
- **THEN** the movie is a favourite and its menu offers Remove from Favourites

#### Scenario: Emby rejects the write

- **WHEN** the user chooses Add to Favourites and Emby answers with an error
- **THEN** the movie stays unfavourited and an error toast shows

### Requirement: Each Emby library lists its own favourites

The GUI Favourites view of a plain Emby library SHALL list that library's favourite items and no
item of another library. It SHALL list collections first (Series, BoxSet, MusicArtist, MusicAlbum,
and other items that hold items) and media items after them (Episode, Movie, Audio, Video), each
group in Title order. A TV library SHALL list its favourite Series in its Favourite Shows view
and its favourite Episodes in its Favourite Episodes view, each in Title order, and neither view
SHALL list the other type. A non-grouped music library SHALL list favourite artists, albums, and
songs. A Grouped Music library SHALL have no Favourites view. An empty list SHALL show no rows.

#### Scenario: TV library Favourite Shows

- **GIVEN** a TV library with three favourite series and one favourite episode
- **WHEN** the user opens its Favourite Shows view
- **THEN** the list shows the three series in Title order and no episode

#### Scenario: TV library Favourite Episodes

- **GIVEN** a TV library with three favourite series and one favourite episode
- **WHEN** the user opens its Favourite Episodes view
- **THEN** the list shows the episode and no series

#### Scenario: Another library's favourite

- **GIVEN** a favourite movie in the Movies library
- **WHEN** the user opens the TV library's Favourite Shows view
- **THEN** the list does not show the movie

#### Scenario: No favourites

- **WHEN** a library has no favourites and the user opens its Favourites view
- **THEN** the list shows no rows

### Requirement: The Favourites view follows favourite changes

Opening a library's Favourites view SHALL fetch that library's favourites again. Removing a
favourite from a row of the open view SHALL remove that row. Adding a favourite elsewhere SHALL
show on the next opening of the view.

#### Scenario: Unfavourite from the list

- **GIVEN** the Favourites view shows a series
- **WHEN** the user chooses Remove from Favourites on that row and Emby accepts it
- **THEN** the row leaves the list

#### Scenario: Favourite added in another place

- **GIVEN** the user favourited a movie from its library's All view
- **WHEN** the user then opens the library's Favourites view
- **THEN** the list shows the movie

#### Scenario: Refresh

- **WHEN** the user presses refresh in a Favourites view
- **THEN** the favourites list is fetched again

### Requirement: A Favourites row acts as the library's own row

A Favourites row SHALL select into the Hero, play, enqueue, and open the item menu as the same item
does in the library's All view. A collection row plays and shuffles its items as a collection row
does.

#### Scenario: Favourite episode plays

- **WHEN** the user plays a favourite episode row
- **THEN** that episode plays as it does from its season

#### Scenario: Favourite series plays all

- **WHEN** the user chooses Play All on a favourite series row
- **THEN** the series' episodes play as Play All on that series does in the All view

#### Scenario: Favourite artist plays

- **WHEN** the user chooses Play All on a favourite artist row
- **THEN** the artist's tracks play in album, disc, and track order

### Requirement: A non-music Emby Hero carries a heart toggle

The GUI Hero of an Emby item SHALL paint a heart control in its upper right exactly when the item
supports favourites and the Hero does not belong to a Grouped Music destination. The heart SHALL
show the item's favourite state. Pressing it SHALL run the same write as the menu entry: on
success the heart, the menu entry, and every held copy of the item SHALL show the new state; on
failure nothing SHALL change and an error toast SHALL show. A Hero whose item does not support
favourites SHALL paint no heart in place of a disabled one.

#### Scenario: Favourite from the Hero

- **GIVEN** the Hero shows an unfavourited movie
- **WHEN** the user presses the heart and Emby accepts the write
- **THEN** the heart shows the favourite state and the item's menu offers Remove from Favourites

#### Scenario: Unfavourite from the Hero

- **GIVEN** the Hero shows a favourited series
- **WHEN** the user presses the heart and Emby accepts the write
- **THEN** the heart shows the unfavourited state

#### Scenario: Write fails

- **WHEN** the user presses the heart and Emby answers with an error
- **THEN** the heart does not change and an error toast shows

#### Scenario: Music Hero

- **WHEN** the Hero of a Grouped Music destination paints
- **THEN** it shows no heart

#### Scenario: Season Hero

- **WHEN** the Hero shows a TV season
- **THEN** it shows no heart

### Requirement: There is no cross-library favourites section

mbv SHALL NOT show a combined favourites list across libraries. GUI Home SHALL NOT paint a
Favourites section.

#### Scenario: Home

- **WHEN** GUI Home paints
- **THEN** it shows no Favourites section
