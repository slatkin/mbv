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

### Requirement: Library leaf rows offer the standard media actions
A library row for a single playable item SHALL offer Play, Add to Queue, and
one played-state entry. This SHALL hold for Emby leaves, Feeds entries,
Audiobookshelf podcast episodes, and Audiobookshelf books. Emby audio tracks
SHALL keep their existing menu, which has no played-state entry.

#### Scenario: Audiobookshelf episode menu
- **WHEN** the user opens the context menu on an unfinished Audiobookshelf podcast episode
- **THEN** the menu offers Play, Add to Queue, and Mark Played, in that order

#### Scenario: Audiobookshelf book menu
- **WHEN** the user opens the context menu on an Audiobookshelf book
- **THEN** the menu offers Play, Add to Queue, and one played-state entry

#### Scenario: Add to Queue on a podcast episode
- **WHEN** the user chooses Add to Queue on an Audiobookshelf podcast episode
- **THEN** the episode is appended to the viewed queue through the existing enqueue path

### Requirement: The single-row played-state entry follows the item's state
A single-row menu SHALL offer exactly one played-state entry. It SHALL be Mark
Unplayed when the item is finished or played, and Mark Played otherwise,
including in-progress items. This SHALL apply to Feeds entries in the same way
as to Emby and Audiobookshelf items.

#### Scenario: Played feed entry
- **WHEN** the user opens the context menu on a played Feeds entry
- **THEN** the menu offers Mark Unplayed and does not offer Mark Played

#### Scenario: In-progress book
- **WHEN** the user opens the context menu on an Audiobookshelf book with a saved position that is not finished
- **THEN** the menu offers Mark Played and does not offer Mark Unplayed

### Requirement: Shuffle is offered only for collections and multi-selections
Shuffle SHALL appear only on a row that has a collection of playable items
under it, such as an Emby folder, and on a multi-selection menu. A single
episode, book, feed entry, or other leaf row SHALL NOT offer Shuffle. An
Audiobookshelf book's chapters SHALL NOT count as a collection.

#### Scenario: Single book has no Shuffle
- **WHEN** the user opens the context menu on one Audiobookshelf book
- **THEN** the menu does not offer Shuffle

#### Scenario: Feeds multi-selection offers Shuffle
- **WHEN** three Feeds entries are selected and the user opens the context menu
- **THEN** the menu offers Play, Shuffle, Add to Queue, Mark Played, and Mark Unplayed

#### Scenario: Audiobookshelf multi-selection offers Shuffle
- **WHEN** two Audiobookshelf podcast episodes are selected and the user chooses Shuffle
- **THEN** the queue is replaced with both episodes in random order and the first starts
