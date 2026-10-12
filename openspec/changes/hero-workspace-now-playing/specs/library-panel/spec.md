# Spec Delta — library-panel

## ADDED Requirements

### Requirement: Workspace rows mark the queue's now-playing item

A Hero Workspace SHALL mark the constituent row that is currently playing in the queue with the
same canonical now-playing row state the Queue projects for its active row, including live progress
that follows the playback position. Every other row SHALL keep its ordinary derived state. When the
queue has no active item, or the active item is not one of the Workspace's constituents, no
Workspace row SHALL carry the now-playing state. A Hero pane without a Workspace is unaffected.

#### Scenario: The playing episode's row lights in its series Workspace

- **WHEN** the queue is playing an episode of the series whose Workspace is shown
- **THEN** that episode's row carries the canonical now-playing state with live progress
- **AND** the other episode rows keep their ordinary state

#### Scenario: Workspace progress follows playback

- **WHEN** the playback position of the active item advances while its Workspace row is shown
- **THEN** that row's projected progress follows the live position

#### Scenario: Nothing is playing

- **WHEN** the queue has no active item
- **THEN** no Workspace row carries the now-playing state

### Requirement: Workspace now-playing matching is by item identity

A Workspace row SHALL carry the now-playing state whenever its constituent item is the queue's
active item, regardless of which surface composed the queue. The pending-selection rule mirrors the
Queue: when the user selects a Workspace constituent to play while another item is still active,
the selected row SHALL carry the now-playing state before the Player owner confirms it, and the
outgoing active row SHALL drop it. A row SHALL NOT carry the state by title or position resemblance
alone.

#### Scenario: Playback started from another surface

- **WHEN** the queue's active item is a Workspace constituent whose playback was started from a
  surface other than that Workspace
- **THEN** the matching row still carries the now-playing state

#### Scenario: A pending selection moves the state

- **WHEN** the user selects a different Workspace constituent to play while another item is active
- **THEN** the selected row carries the now-playing state before the Player owner confirms it
- **AND** the previously active row drops it

### Requirement: Audiobookshelf chapters mark the playing chapter by book position

For an Audiobookshelf book Workspace, the active queue item is the book. When the queue's active
item is the Workspace's book, the chapter that contains the current playback position SHALL carry
the canonical now-playing state with live progress. When the active item is any other book, episode,
or item, no chapter row SHALL carry the state.

#### Scenario: The playing book's chapter lights

- **WHEN** the queue is playing the book whose Workspace is shown
- **THEN** the chapter containing the playback position carries the now-playing state

#### Scenario: A different book is playing

- **WHEN** the queue is playing a book other than the Workspace's book
- **THEN** no chapter row carries the now-playing state
