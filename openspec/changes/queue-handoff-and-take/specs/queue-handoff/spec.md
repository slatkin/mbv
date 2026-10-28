# Spec Delta

## Purpose

Moves the music between the local Owner and a directly reachable peer Player owner by copying a queue and its position. Each Player owner keeps its own queue; Hand off and Take are the only actions that copy a queue between them.

## ADDED Requirements

### Requirement: Hand off copies the local queue to the peer and moves playback there

Hand off SHALL replace the peer queue with a copy of the local Owner queue and its Queue source. When the local Owner has an active slot, the peer SHALL start that slot at the local playback position. The local Owner SHALL keep its queue unchanged. The Client SHALL stop local playback only after the peer reports playback of the handed-off slot.

#### Scenario: Hand off while local playback runs

- **WHEN** the local Owner plays slot 3 of 10 at 12:40 and the user hands off to a peer
- **THEN** the peer queue SHALL hold the same 10 items in the same order
- **AND** the peer SHALL play the third item from 12:40
- **AND** local playback SHALL stop after the peer reports that item as playing
- **AND** the local Owner SHALL still hold its 10 items

#### Scenario: Hand off while nothing plays

- **WHEN** the local Owner holds a queue with no active slot and the user hands off
- **THEN** the peer queue SHALL be replaced with the copy
- **AND** no playback SHALL start on the peer

### Requirement: Hand off connects the Client to the peer

When Hand off succeeds, the Client SHALL be connected to the peer as by Connect: the queue panel SHALL show the peer queue and queue edits SHALL reach the peer.

#### Scenario: Queue panel after Hand off

- **WHEN** a Hand off to a peer succeeds
- **THEN** the queue panel SHALL show the peer queue
- **AND** the queue panel SHALL name the peer as its Player owner

### Requirement: Take copies the peer queue to the local Owner and moves playback here

Take SHALL replace the local Owner queue with a copy of the peer queue and its Queue source. When the peer has an active slot, the local Owner SHALL start that slot at the peer playback position. The peer SHALL keep its queue. The Client SHALL stop the peer only after the local Owner reports playback of the taken slot.

#### Scenario: Take while the peer plays

- **WHEN** a peer plays slot 2 of 5 at 03:10 and the user takes from it
- **THEN** the local Owner queue SHALL hold the same 5 items in the same order
- **AND** the local Owner SHALL play the second item from 03:10
- **AND** the peer SHALL stop after the local Owner reports that item as playing
- **AND** the peer SHALL still hold its 5 items

#### Scenario: Take while the peer is idle

- **WHEN** the peer holds a queue with no active slot and the user takes from it
- **THEN** the local Owner queue SHALL be replaced with the copy
- **AND** no playback SHALL start locally

### Requirement: Take disconnects and works without a connection

After a successful Take, the Client SHALL be disconnected from the peer and SHALL show the local Owner queue. Take SHALL also be available for a peer the Client is not connected to; it SHALL then leave the Client unconnected.

#### Scenario: Take from the connected peer

- **WHEN** the Client is connected to a peer and Take succeeds
- **THEN** the Client SHALL no longer be connected to the peer
- **AND** the queue panel SHALL show the local Owner queue

#### Scenario: Take from an unconnected peer

- **WHEN** the Client is not connected to a peer and the user takes from that peer
- **THEN** the local Owner SHALL receive the copy
- **AND** the Client SHALL stay unconnected

### Requirement: Replacing a populated target queue asks first

Hand off SHALL ask the user to confirm before it replaces a peer queue that is not empty, and Take SHALL ask before it replaces a local Owner queue that is not empty. The prompt SHALL name the target. Cancel SHALL change nothing on either Player owner.

#### Scenario: Peer already holds a queue

- **WHEN** the user hands off to a peer whose queue holds items
- **THEN** the Client SHALL ask for confirmation before it sends the copy
- **AND** cancel SHALL leave both queues and both playbacks unchanged

#### Scenario: Target queue is empty

- **WHEN** the target queue is empty
- **THEN** Hand off or Take SHALL proceed without a prompt

### Requirement: A failed Hand off or Take leaves the source playing

When the target refuses the copy, refuses or cannot admit the active item, or the connection is lost before the target reports playback, the Client SHALL report the failure and SHALL NOT stop the source. The source queue SHALL be unchanged.

#### Scenario: Audio-only peer cannot play the active video

- **WHEN** the local Owner plays a video and the user hands off to an audio-only peer
- **THEN** the Client SHALL report that the peer cannot play the active item
- **AND** local playback SHALL continue

#### Scenario: Connection lost during Hand off

- **WHEN** the connection to the peer drops after the copy is sent and before the peer reports playback
- **THEN** the Client SHALL report the failure
- **AND** local playback SHALL continue

### Requirement: Hand off and Take need a peer that accepts a start position

Hand off SHALL be offered only to a peer that advertises the queue replacement start-position capability. Take SHALL need that capability only on the local Owner. When the peer lacks it, the Client SHALL refuse Hand off with a message that names the cause and SHALL change nothing.

#### Scenario: Older peer

- **WHEN** the user hands off to a peer that does not advertise the capability
- **THEN** the Client SHALL report that the peer does not support Hand off
- **AND** neither queue nor playback SHALL change

### Requirement: Connect and Disconnect move only control

Connect SHALL make the Client show and edit the peer queue and SHALL NOT copy a queue or start, stop, or move playback. Disconnect SHALL make the Client show the local Owner queue and SHALL leave the peer queue and playback unchanged.

#### Scenario: Disconnect leaves the peer playing

- **WHEN** the Client is connected to a peer that plays and the user disconnects
- **THEN** the peer SHALL continue to play
- **AND** the queue panel SHALL show the local Owner queue

#### Scenario: Connect does not move playback

- **WHEN** the local Owner plays and the user connects to a peer
- **THEN** local playback SHALL continue
- **AND** the queue panel SHALL show the peer queue
