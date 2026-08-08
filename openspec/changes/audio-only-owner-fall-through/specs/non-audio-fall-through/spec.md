## Purpose

Lets a client controlling an audio-only Player owner play or stage a non-audio
item on itself, without disconnecting from that owner, so a video launched by
mistake or on purpose plays where there is a display instead of being refused.

## ADDED Requirements

### Requirement: An explicitly launched non-audio item plays on the client

When a client holds Direct remote control over a Player owner that has declared
itself audio-only, and the user explicitly plays a wholly non-audio selection,
the client SHALL play it on its own player instead of submitting it to the
owner.

#### Scenario: User plays a film while controlling an audio-only owner

- **WHEN** the user plays a video item from the library while controlling an
  audio-only owner
- **THEN** the item SHALL play on the client
- **THEN** the client SHALL NOT submit the item to the owner
- **THEN** the control connection to the owner SHALL remain open

#### Scenario: Owner has not declared itself audio-only

- **WHEN** the user plays a video item while controlling an owner that has not
  declared itself audio-only
- **THEN** the client SHALL submit the item to the owner as it does today

### Requirement: Fall-through applies to explicit action only

The client SHALL route to itself only in response to a deliberate user play or
enqueue. It SHALL NOT route to itself when a queue advances, when playback
resumes, or in response to any owner-initiated event.

#### Scenario: An owner's queue advances

- **WHEN** an owner's queue advances to its next item
- **THEN** the client SHALL take no routing decision

#### Scenario: A mixed selection is played

- **WHEN** the user plays a selection containing both audio and non-audio items
- **THEN** the client SHALL submit it to the owner
- **THEN** the client SHALL strip the non-audio items before submitting
- **THEN** the client SHALL report how many items it dropped
- **THEN** the dropped items SHALL NOT be added to the client's own queue

### Requirement: Starting local playback stops the owner

When a client begins playing a fallen-through item, it SHALL stop the owner's
playback rather than pausing it.

#### Scenario: Owner is playing when the film starts

- **WHEN** the user plays a fallen-through item while the owner is playing
- **THEN** the owner SHALL stop
- **THEN** the owner's playback position SHALL NOT be preserved for resumption

#### Scenario: Owner is idle when the film starts

- **WHEN** the user plays a fallen-through item while the owner is idle
- **THEN** the owner SHALL remain idle
- **THEN** the owner's queue SHALL be unchanged

### Requirement: The owner remains the target for the next addition

Fall-through SHALL apply to one item at a time. After a fallen-through item
finishes, is stopped, or fails, the next play or enqueue SHALL be evaluated
against the owner again on the same terms.

#### Scenario: Audio played after a film ends

- **WHEN** a fallen-through item finishes and the user then plays an audio
  selection
- **THEN** the selection SHALL be submitted to the owner
- **THEN** playback SHALL occur on the owner

#### Scenario: A second film played after the first

- **WHEN** a fallen-through item finishes and the user then plays another
  non-audio item
- **THEN** that item SHALL fall through in turn

### Requirement: An enqueued non-audio item is staged, not played

When the user explicitly enqueues a wholly non-audio selection while controlling
an audio-only owner, the client SHALL add it to its own queue without starting
playback and without disturbing the owner.

#### Scenario: Enqueuing a film while music plays

- **WHEN** the user enqueues a video item while the owner is playing
- **THEN** the item SHALL be added to the client's own queue
- **THEN** the owner SHALL continue playing
- **THEN** the client SHALL report that the item was added to its own queue

#### Scenario: Disconnecting with a staged queue

- **WHEN** the user disconnects from the owner while the client's own queue
  holds staged items
- **THEN** the staged queue SHALL remain
- **THEN** playback SHALL NOT start on its own

### Requirement: A playing fallen-through item is shown in the owner's queue view

While a fallen-through item is playing, the client SHALL show it pinned above
the owner's queue, styled as a selected row, and marked as playing on the
client. The row SHALL NOT be selectable and SHALL be skipped by cursor
navigation, because the item is not a member of that queue.

#### Scenario: Viewing the owner's queue during local playback

- **WHEN** the user views the owner's queue while a fallen-through item plays
- **THEN** the item SHALL appear pinned above the owner's items
- **THEN** the row SHALL carry a marker identifying it as playing on the client
- **THEN** the row SHALL show position and progress from the client's player

#### Scenario: Navigating the owner's queue during local playback

- **WHEN** the user moves the cursor through the owner's queue
- **THEN** the cursor SHALL NOT land on the pinned row
- **THEN** queue actions SHALL NOT target the pinned row

#### Scenario: Local playback ends

- **WHEN** a fallen-through item finishes or is stopped
- **THEN** the pinned row SHALL be removed from the owner's queue view

### Requirement: A remote connection and the active playback target are separate

The client SHALL track which player is the active playback target separately
from whether a remote attachment exists. Holding a connection to an owner SHALL
NOT by itself mean playback is directed there.

#### Scenario: Transport control during local playback

- **WHEN** the user pauses, seeks, or skips while a fallen-through item plays
- **THEN** the command SHALL act on the client's player
- **THEN** the command SHALL NOT be sent to the owner

#### Scenario: Owner state during local playback

- **WHEN** a fallen-through item is playing
- **THEN** the client SHALL continue to receive and display the owner's queue
  state
- **THEN** the client SHALL continue to be identified as controlling that owner
