# Spec Delta

## ADDED Requirements

### Requirement: A switch leaves the old remote output playing

When the Owner performs an output switch away from a Cast receiver or an Emby Session, it SHALL
release that device without stopping it: the Owner SHALL send the device no stop and SHALL NOT
tear down a receiver application. The device SHALL keep playing what it plays, and the Owner SHALL
no longer drive or track it. The Owner SHALL still make the target the output, keep the queue
unchanged, and start no slot on the target. A switch away from local playback SHALL stop the
local run, because the Owner hosts it. This requirement supersedes every statement that an output
switch stops a remote old output. A hand-off (the continue answer) SHALL keep stopping the old
output, because the slot moves to the target.

#### Scenario: A receiver is released by a switch

- **WHEN** a Cast receiver plays a slot and the Owner receives an output switch to another target
  and the target connects
- **THEN** the receiver keeps playing and receives no stop
- **AND** the target is the output and plays nothing
- **AND** the queue is unchanged

#### Scenario: An Emby Session is released by a switch

- **WHEN** an Emby Session plays the loaded item and the Owner receives an output switch to
  another target and the target connects
- **THEN** the Session device keeps playing and receives no stop
- **AND** the Owner no longer reacts to that device's playback

#### Scenario: A released device does not move the queue

- **WHEN** a released device ends, pauses, or stops its media after the switch
- **THEN** the Owner records nothing for it and does not advance the queue

#### Scenario: Local playback is stopped by a switch

- **WHEN** local playback plays a slot and the Owner receives an output switch to a remote target
- **THEN** the local run stops
- **AND** the target is the output and plays nothing

#### Scenario: A remote output switches to local playback

- **WHEN** a Cast receiver or an Emby Session plays a slot and the Owner receives an output
  switch to local playback
- **THEN** the remote device keeps playing and receives no stop
- **AND** local playback is the output and plays nothing

#### Scenario: A hand-off still stops the old output

- **WHEN** the Owner performs a selection that hands off the active slot
- **THEN** the old output stops and the target continues the slot

#### Scenario: A failed switch changes nothing

- **WHEN** the target does not connect
- **THEN** the Owner rejects the switch and the old output stays the output and keeps playing
