# Spec Delta

## MODIFIED Requirements

### Requirement: Keyboard-triggered menu anchors to the selected item
When the context menu is opened via the keyboard shortcut from Home, an Emby
browse view, a Feeds list, an Audiobookshelf podcast or book list, or the
queue, its position SHALL be derived from the fresh-frame screen rectangle of
the currently selected row or grid cell in the focused panel.

The menu's right edge SHALL align with the selected item's right edge whenever
the rendered menu fits horizontally at that position. The menu SHALL open with
its top edge aligned to the selected item's top edge when its full rendered
height fits below that edge in the containing panel. Otherwise it SHALL open
upward with its bottom edge aligned to the selected item's bottom edge.

After choosing the preferred position, the menu SHALL be clamped inside the
containing panel when exact alignment would place it outside that panel. If the
menu itself exceeds the available panel dimension, the panel edge SHALL win and
the terminal renderer MAY clip the unavoidable overflow.

#### Scenario: Selected item near the top of the panel
- **WHEN** the shortcut is pressed and the menu fits from the selected item's
  top edge to the panel's bottom edge
- **THEN** the menu opens downward with its top-right corner aligned to the
  selected item's top-right corner

#### Scenario: Selected item near the bottom of the panel
- **WHEN** the shortcut is pressed and the menu does not fit below but does fit
  above the selected item
- **THEN** the menu opens upward with its bottom-right corner aligned to the
  selected item's bottom-right corner

#### Scenario: Preferred alignment exceeds a panel edge
- **WHEN** the preferred placement would extend outside the containing panel
- **THEN** the menu position is clamped to that panel's nearest edge

#### Scenario: Selected item is a grid cell
- **WHEN** a supported Emby view presents selectable items in multiple columns
  and the shortcut is pressed
- **THEN** the menu anchors to the selected cell's actual on-screen rectangle,
  not the full row or panel width

#### Scenario: Nested detail content is present
- **WHEN** the selected row or cell expands or renders nested hero/detail
  content
- **THEN** the outer selectable row or cell remains the menu anchor and nested
  content does not replace it

#### Scenario: Unsupported destination is active
- **WHEN** the focused panel has no selected row or cell, for example an
  empty list, and the shortcut is pressed
- **THEN** no context menu opens

#### Scenario: Audiobookshelf row is selected
- **WHEN** an Audiobookshelf podcast episode or book row is selected and the
  shortcut is pressed
- **THEN** a context menu opens anchored to that row

## ADDED Requirements

### Requirement: Library lists share one standard action set
Every library list row menu SHALL use the standard set in this order. A leaf
row offers Play, Add to Queue, then one played-state entry. A collection row
offers Play All, Shuffle, Add to Queue, then one played-state entry. A
multi-selection offers Play, Shuffle, Add to Queue, Mark Played, then Mark
Unplayed. Entries a documented exception removes SHALL be left out without
reordering the rest.

#### Scenario: Movie leaf
- **WHEN** the user opens the context menu on an unplayed movie
- **THEN** the menu offers Play, Add to Queue, Mark Played, in that order

#### Scenario: TV season collection
- **WHEN** the user opens the context menu on a TV season with unplayed episodes
- **THEN** the menu offers Play All, Shuffle, Add to Queue, Mark Played, in that order

#### Scenario: Audiobookshelf podcast episode leaf
- **WHEN** the user opens the context menu on an unfinished Audiobookshelf podcast episode
- **THEN** the menu offers Play, Add to Queue, Mark Played, in that order

#### Scenario: Feeds multi-selection
- **WHEN** three Feeds entries are selected and the user opens the context menu
- **THEN** the menu offers Play, Shuffle, Add to Queue, Mark Played, Mark Unplayed, in that order

### Requirement: Every painted library row resolves its own menu
A library row's menu SHALL target the item that row paints, whatever source
filled the list. This SHALL include rows in an Emby homevideos feed view and
rows shown in a destination's Latest mode.

#### Scenario: YouTube video row
- **WHEN** the user right-clicks a video row in an Emby homevideos feed view, such as a YouTube tab
- **THEN** the standard leaf menu opens for that video

#### Scenario: Latest row
- **WHEN** an Emby library is in Latest mode and the user presses the menu key on a row
- **THEN** the standard menu opens for the item that row shows

### Requirement: The played-state entry is labelled Played and follows state
Every played-state entry SHALL read "Mark Played" or "Mark Unplayed". No menu
SHALL say "Watched". A single-row menu SHALL offer only the entry that changes
the current state: Mark Unplayed for a played or finished item, or a
collection with no unplayed children, and Mark Played otherwise, including
in-progress items.

#### Scenario: Played movie
- **WHEN** the user opens the context menu on a played movie
- **THEN** the menu offers Mark Unplayed and does not offer Mark Played or any "Watched" entry

#### Scenario: Played feed entry
- **WHEN** the user opens the context menu on a played Feeds entry
- **THEN** the menu offers Mark Unplayed and does not offer Mark Played

#### Scenario: In-progress book
- **WHEN** the user opens the context menu on an Audiobookshelf book with a saved position that is not finished
- **THEN** the menu offers Mark Played and does not offer Mark Unplayed

### Requirement: Shuffle is offered only for collections and multi-selections
Shuffle SHALL appear only on a collection row, meaning a row with playable
items under it such as a series, season, album, artist, or folder, and on a
multi-selection. A leaf row SHALL NOT offer Shuffle. An Audiobookshelf book is
a leaf: its chapters are positions inside one queue item, not a collection of
items.

#### Scenario: Single book has no Shuffle
- **WHEN** the user opens the context menu on one Audiobookshelf book
- **THEN** the menu does not offer Shuffle

#### Scenario: Audiobookshelf multi-selection shuffles
- **WHEN** two Audiobookshelf podcast episodes are selected and the user chooses Shuffle
- **THEN** the queue is replaced with both episodes in random order and the first starts

### Requirement: Music rows offer no played-state entry
Music tracks, albums, and artists SHALL NOT offer Mark Played or Mark
Unplayed, either on a single row or in a multi-selection. mbv never resumes
music and ignores music played state everywhere, so the entry would have no
visible effect.

#### Scenario: Album menu
- **WHEN** the user opens the context menu on a music album
- **THEN** the menu offers Play All, Shuffle, Add to Queue, and no played-state entry

#### Scenario: Mixed multi-selection with a track
- **WHEN** a multi-selection holds a movie and a music track
- **THEN** the menu offers neither Mark Played nor Mark Unplayed

### Requirement: List-specific removals follow the standard entries
An entry that removes an item from the list it is shown in SHALL appear after
the standard entries. Continue Watching rows SHALL add Remove from Continue
Watching, and a removable multi-selection SHALL add Remove.

#### Scenario: Continue Watching row
- **WHEN** the user opens the context menu on an unplayed Continue Watching row
- **THEN** the menu offers Play, Add to Queue, Mark Played, then Remove from Continue Watching

#### Scenario: Continue Watching multi-selection
- **WHEN** two Continue Watching rows are selected and the user opens the context menu
- **THEN** Remove is the last entry, after Mark Unplayed

### Requirement: Selector pills open no context menu
Pills in a selector row SHALL NOT open a context menu. Examples are podcast
show pills, feed subscription pills, Emby homevideos channel-group pills, and
letter pills. A pill selects which rows the list shows. It is not a playable
row.

#### Scenario: Right-click a show pill
- **WHEN** the user right-clicks a podcast show pill
- **THEN** no context menu opens
