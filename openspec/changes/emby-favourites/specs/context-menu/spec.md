# Spec Delta

## MODIFIED Requirements

### Requirement: Library lists share one standard action set
Every library list row menu SHALL use the standard set in this order. A leaf
row offers Play, Enqueue, then one played-state entry. A collection row
offers Play All, Shuffle, Enqueue, then one played-state entry. An Emby
row whose item supports favourites (capability `emby-favourites`) adds one
favourites entry after the played-state entry, or after Enqueue when the
row has no played-state entry. A multi-selection offers Play, Shuffle, Add to
Queue, Mark Played, then Mark Unplayed, and no favourites entry. Entries a
documented exception removes SHALL be left out without reordering the rest.

#### Scenario: Movie leaf
- **WHEN** the user opens the context menu on an unplayed movie that is not a favourite
- **THEN** the menu offers Play, Enqueue, Mark Played, Add to Favourites, in that order

#### Scenario: TV season collection
- **WHEN** the user opens the context menu on a TV season with unplayed episodes
- **THEN** the menu offers Play All, Shuffle, Enqueue, Mark Played, in that order

#### Scenario: TV series collection
- **WHEN** the user opens the context menu on a TV series that is not a favourite and has unplayed episodes
- **THEN** the menu offers Play All, Shuffle, Enqueue, Mark Played, Add to Favourites, in that order

#### Scenario: Audiobookshelf podcast episode leaf
- **WHEN** the user opens the context menu on an unfinished Audiobookshelf podcast episode
- **THEN** the menu offers Play, Enqueue, Mark Played, in that order

#### Scenario: Feeds multi-selection
- **WHEN** three Feeds entries are selected and the user opens the context menu
- **THEN** the menu offers Play, Shuffle, Enqueue, Mark Played, Mark Unplayed, in that order


### Requirement: Music rows offer no played-state entry
Music tracks, albums, and artists SHALL NOT offer Mark Played or Mark
Unplayed, either on a single row or in a multi-selection. mbv never resumes
music and ignores music played state everywhere, so the entry would have no
visible effect. This rule does not remove the favourites entry
(capability `emby-favourites`), which music items offer.

#### Scenario: Album menu
- **WHEN** the user opens the context menu on a music album
- **THEN** the menu offers Play All, Shuffle, Enqueue, Add to Favourites, and no played-state entry

#### Scenario: Mixed multi-selection with a track
- **WHEN** a multi-selection holds a movie and a music track
- **THEN** the menu offers neither Mark Played nor Mark Unplayed

### Requirement: List-specific removals follow the standard entries
An entry that removes an item from the list it is shown in SHALL appear after
the standard entries. Continue Watching rows SHALL add Remove from Continue
Watching, and a removable multi-selection SHALL add Remove.

#### Scenario: Continue Watching row
- **WHEN** the user opens the context menu on an unplayed Continue Watching row
- **THEN** the menu offers Play, Enqueue, Mark Played, Add to Favourites, then Remove from Continue Watching

#### Scenario: Continue Watching multi-selection
- **WHEN** two Continue Watching rows are selected and the user opens the context menu
- **THEN** Remove is the last entry, after Mark Unplayed

