# non-audio-fall-through Specification

## Purpose
Lets a client directly controlling an audio-only Player owner handle explicit
non-audio actions locally without ending control of the owner or hiding its
Bound queue.
## Requirements
### Requirement: Eligible control relationships

Fall-through SHALL apply when an audio-only owner is controlled through
Sessions-panel Direct remote control or an explicit remote daemon attachment.
It SHALL NOT apply through a Library route, Session watch, or a peer that has not
advertised audio-only.

#### Scenario: Direct remote control is eligible
- **WHEN** the user explicitly plays a non-audio item while Direct remote control targets an owner advertising audio-only
- **THEN** the client SHALL apply fall-through

#### Scenario: Explicit daemon attachment is eligible
- **WHEN** a client launched against an explicit remote daemon endpoint explicitly plays a non-audio item and that owner advertises audio-only
- **THEN** the client SHALL apply fall-through

#### Scenario: Library route is ineligible
- **WHEN** a Library route targets an owner advertising audio-only
- **THEN** explicit play and enqueue actions SHALL retain Library-route behavior and SHALL NOT fall through locally

### Requirement: Routing decision for explicit selections

For each eligible explicit play or enqueue, the client SHALL choose a Submission
destination from the owner's advertised capability and the selection contents
before submitting or mutating queue presentation state.

#### Scenario: Wholly non-audio selection
- **WHEN** an eligible explicit selection contains only non-audio items
- **THEN** the client SHALL send no item from that selection to the owner
- **THEN** the client SHALL direct the selection to its own queue or Player according to the action

#### Scenario: Mixed selection
- **WHEN** an eligible explicit selection contains audio and non-audio items
- **THEN** the client SHALL submit only the audio items to the owner
- **THEN** the client SHALL report the number of non-audio items dropped
- **THEN** the client SHALL NOT stage the dropped items locally

#### Scenario: Wholly audio selection
- **WHEN** an eligible explicit selection contains only audio items
- **THEN** the client SHALL submit the selection to the owner as it does today

#### Scenario: Peer without the capability
- **WHEN** the controlled peer has not advertised audio-only
- **THEN** the client SHALL submit the selection as it does today

### Requirement: Fall-through is per explicit action

The client SHALL evaluate every explicit play or enqueue independently.
Fall-through SHALL NOT become a persistent routing mode and SHALL NOT be invoked
by queue auto-advance, resume, or owner-initiated events.

#### Scenario: Action after fall-through
- **WHEN** the user performs another explicit action after an item has fallen through
- **THEN** the client SHALL evaluate that action against the still-attached owner's capability and the new selection contents

#### Scenario: Bound queue advances
- **WHEN** either Player owner advances within its Bound queue
- **THEN** the client SHALL NOT invoke fall-through routing for that advance

### Requirement: Playing locally preserves the attachment

Before local playback of a fallen-through item begins, the client SHALL prepare
a local Player, constructing one if necessary. Only after preparation succeeds
SHALL it stop the attached owner and make the local Player the Transport owner.
The owner attachment and its Bound queue SHALL remain live.

#### Scenario: Playing a video
- **WHEN** the user explicitly plays a wholly non-audio selection through an eligible relationship
- **THEN** the client SHALL prepare local playback and then stop the owner rather than pause it
- **THEN** the client SHALL play the selection locally
- **THEN** the owner attachment SHALL remain established

#### Scenario: No local Player has been constructed
- **WHEN** an eligible fall-through play begins without an existing local Player
- **THEN** the client SHALL construct the local Player before stopping the owner and attempt normal local playback

### Requirement: Enqueuing locally does not change transport ownership

A wholly non-audio explicit enqueue SHALL add the selection to the client's own
queue without starting playback, stopping the owner, or changing the Transport
owner.

#### Scenario: Enqueue a video
- **WHEN** the user explicitly enqueues a wholly non-audio selection through an eligible relationship
- **THEN** the client SHALL add the selection to its own queue
- **THEN** the client SHALL leave owner playback and transport ownership unchanged

### Requirement: Queue availability is independent of transport ownership

While a local Player is the Transport owner during fall-through, the client
SHALL keep Local and Remote Queue scopes available and SHALL direct queue
commands to the Player owner that owns the selected queue.

#### Scenario: View the owner queue during local playback
- **WHEN** a fallen-through item is playing locally and the user selects Remote Queue scope
- **THEN** the client SHALL display the attached owner's current Bound queue

#### Scenario: Mutate the owner queue during local playback
- **WHEN** a fallen-through item is playing locally and the user performs a queue action in Remote Queue scope
- **THEN** the client SHALL send that action to the attached owner and SHALL NOT send it to the local Player

### Requirement: Player events retain owner origin

The client SHALL apply each player event to the Local or Attached-owner session
that emitted it rather than inferring its origin from the current Transport
owner.

#### Scenario: Parked owner reports a queue update
- **WHEN** the attached owner reports a queue update during local fall-through playback
- **THEN** the client SHALL update only the Remote Bound queue

#### Scenario: Parked owner reports stopped
- **WHEN** the attached owner reports stop or completion during local fall-through playback
- **THEN** the client SHALL NOT end local playback or mutate the Local queue

#### Scenario: Local playback ends
- **WHEN** the Local session reports that fallen-through playback has ended and the owner remains attached
- **THEN** the client SHALL return transport ownership to the attached owner

#### Scenario: Attached owner disconnects
- **WHEN** the attached owner disconnects during local fall-through playback
- **THEN** the client SHALL remove the attachment and Remote Queue scope
- **THEN** the client SHALL allow local playback to continue

### Requirement: Pinned row in the owner queue view

While a fallen-through item plays and Remote Queue scope is visible, the client
SHALL render that item above the owner's queue items in selected-row styling,
marked as playing on the client, with local position and progress. The row SHALL
be a non-selectable projection rather than a member of the owner's queue.

#### Scenario: Remote queue view during fall-through
- **WHEN** Remote Queue scope is visible while a fallen-through item plays
- **THEN** the client SHALL render the projected local-playing row above the owner's items
- **THEN** cursor navigation and queue actions SHALL skip that row

#### Scenario: Local playback ends
- **WHEN** fallen-through local playback ends
- **THEN** the client SHALL remove the projected row

