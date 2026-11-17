# Spec Delta

## Purpose

Adds the Client-side gate over the Owner's hand-off: a selection that would hand off an active
slot asks first. The Owner-side hand-off semantics in this capability are unchanged.

## ADDED Requirements

### Requirement: A Client asks before a hand-off

When a user selection of a playback output would hand off an active slot — a slot is active on
the current output and the selected target is not that output — the Client SHALL ask first, with
a prompt that names the target and offers Yes and No. Both Clients SHALL apply the same prompt
through their shared App dispatch. Launch auto-reconnect and the owner's own advance SHALL NOT
ask: they are not user selections.

#### Scenario: A selection that would hand off asks

- **WHEN** the user selects a playback target while a slot is active on the current output and
  the target is not that output
- **THEN** the Client asks "Continue playback on the target?" with Yes and No
- **AND** no output selection reaches the Owner until the user answers

#### Scenario: Selecting local playback asks while a remote output plays

- **WHEN** the user selects local playback while a Cast receiver or an Emby Session holds an
  active slot
- **THEN** the Client asks before it selects local playback
- **AND** No leaves the remote output playing; Yes stops it and local playback continues the
  slot from its position

### Requirement: Yes runs the hand-off and No cancels

The Yes answer SHALL send the selection unchanged, so the Owner performs the hand-off this
capability defines. The No answer SHALL send nothing, and the current output SHALL keep playing.
A selection that hands nothing off SHALL keep its current path without a prompt.

#### Scenario: Yes runs the hand-off unchanged

- **WHEN** the user answers Yes
- **THEN** the Owner stops the old output and continues the active slot on the new output at its
  position, exactly as an unasked selection would

#### Scenario: No cancels

- **WHEN** the user answers No
- **THEN** the Client sends nothing and the current output keeps playing

#### Scenario: Nothing active, or the no-op selection, skips the prompt

- **WHEN** nothing is active on the current output, or the selected target already is the output
  (the Owner's no-op hand-off)
- **THEN** the selection runs its current path without a prompt
