## MODIFIED Requirements

### Requirement: Shuffle is offered only for collections and multi-selections
Shuffle SHALL appear only on a collection row, meaning a row with playable
items under it such as a series, season, album, artist, or folder, and on a
multi-selection. A leaf row SHALL NOT offer Shuffle. An Audiobookshelf book is
a leaf: its chapters are positions inside one queue item, not a collection of
items. This rule covers the Shuffle entry that replaces the queue. The Queue
list shuffle-order toggle (capability `queue-play-order`) is a different entry:
it changes the playback order of the existing queue, and it appears on every
Queue row.

#### Scenario: Single book has no Shuffle
- **WHEN** the user opens the context menu on one Audiobookshelf book
- **THEN** the menu does not offer Shuffle

#### Scenario: Audiobookshelf multi-selection shuffles
- **WHEN** two Audiobookshelf podcast episodes are selected and the user chooses Shuffle
- **THEN** the queue is replaced with both episodes in random order and the first starts

#### Scenario: Queue row offers the order toggle, not queue replacement
- **WHEN** the user opens the context menu on a Queue row
- **THEN** the menu offers the shuffle-order toggle and does not offer the Shuffle entry that replaces the queue
