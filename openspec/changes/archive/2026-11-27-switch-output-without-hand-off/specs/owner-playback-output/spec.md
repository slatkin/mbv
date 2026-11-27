# Spec Delta

## MODIFIED Requirements

### Requirement: A Client asks before a hand-off

When a user selection of a playback output would hand off an active slot — a slot is active on
the current output and the selected target is not that output — the Client SHALL ask first, with
a prompt that names the target and offers two answers that both act: continue the slot on the
target, or switch to the target and hand nothing off. The prompt SHALL offer no answer that
cancels the selection. Both Clients SHALL apply the same prompt through their shared App
dispatch. Launch auto-reconnect and the owner's own advance SHALL NOT ask: they are not user
selections.

#### Scenario: A selection that would hand off asks

- **WHEN** the user selects a playback target while a slot is active on the current output and
  the target is not that output
- **THEN** the Client asks whether to continue playback on the target or to switch to it only
- **AND** no output selection reaches the Owner until the user answers

#### Scenario: Selecting local playback asks while a remote output plays

- **WHEN** the user selects local playback while a Cast receiver or an Emby Session holds an
  active slot
- **THEN** the Client asks before it selects local playback
- **AND** the continue answer stops the remote output and local playback continues the slot from
  its position
- **AND** the switch answer stops the remote output and makes local playback the output with
  nothing playing

## REMOVED Requirements

### Requirement: Yes runs the hand-off and No cancels

**Reason**: The No answer cancelled the whole selection, so a user could not switch to a target
without moving the playing slot there. The two answers now both act.
**Migration**: `The switch answer hands nothing off` replaces it. The continue answer keeps the
meaning of the old Yes answer.

## ADDED Requirements

### Requirement: The switch answer hands nothing off

The continue answer SHALL send the selection unchanged, so the Owner performs the hand-off this capability defines. The switch answer SHALL send the output switch for the same target. When the local Owner does not advertise the output switch, the prompt SHALL offer Yes and No instead, and No SHALL send nothing. A selection that hands nothing off SHALL keep its current path without a prompt.

#### Scenario: Continue runs the hand-off unchanged

- **WHEN** the user answers with the continue answer
- **THEN** the Owner stops the old output and continues the active slot on the new output at its
  position, exactly as an unasked selection would

#### Scenario: Switch hands nothing off

- **WHEN** the user answers with the switch answer
- **THEN** the Client sends the output switch for the target
- **AND** the Owner stops the old output, makes the target the output, and starts no slot on it

#### Scenario: An Owner without the output switch keeps a cancel

- **WHEN** the local Owner does not advertise the output switch and the user is asked
- **THEN** the prompt offers Yes and No
- **AND** No sends nothing and the current output keeps playing

#### Scenario: Nothing active, or the no-op selection, skips the prompt

- **WHEN** nothing is active on the current output, or the selected target already is the output
  (the Owner's no-op hand-off)
- **THEN** the selection runs its current path without a prompt

### Requirement: An output switch hands nothing off

When the Owner receives an output switch for a target, it SHALL select that output as an output
selection does, but it SHALL capture no hand-off: the old output stops, the target becomes the
active output, and no slot starts on it. The Owner SHALL keep the queue and its order unchanged.
A switch to the output that is already active SHALL be rejected as a selection of it is.

#### Scenario: Local playback switches to a receiver

- **WHEN** local playback plays a slot and the Owner receives an output switch to a Cast receiver
  and the receiver connects
- **THEN** local playback stops
- **AND** the receiver is the output and plays nothing
- **AND** the queue is unchanged

#### Scenario: A remote output switches to local playback

- **WHEN** a Cast receiver or an Emby Session plays a slot and the Owner receives an output
  switch to local playback
- **THEN** the remote output stops
- **AND** local playback is the output and plays nothing

#### Scenario: A paused slot is not carried over

- **WHEN** the active slot is paused and the Owner receives an output switch to a different output
- **THEN** the new output holds no slot and nothing is paused or playing on it

#### Scenario: The target cannot be reached

- **WHEN** the Owner receives an output switch and the target does not connect or is not listed
- **THEN** the Owner rejects the switch with a reason
- **AND** the old output keeps playing

#### Scenario: Playing after a switch uses the new output

- **WHEN** the user plays a slot after a switch made a remote receiver the output
- **THEN** the owner loads that slot onto the receiver and local playback does not start
