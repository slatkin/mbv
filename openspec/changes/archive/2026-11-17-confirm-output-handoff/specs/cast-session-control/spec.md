# Spec Delta

## Purpose

Defines how a Client gates a playback-output selection that would hand off an active slot: one
shared prompt, asked before the Owner performs the hand-off. The Owner-side hand-off semantics
live in `owner-playback-output` and are unchanged.

## ADDED Requirements

### Requirement: A hand-off asks the user first

When a user playback-output selection would hand off an active slot — a slot is active on the
current output and the selected target is not that output — the Client SHALL ask before the
Owner performs the hand-off. The prompt SHALL name the target ("Continue playback on the target
name?") and SHALL offer Yes and No. Both Clients (the TUI and the GUI Client) SHALL apply the
same prompt through their shared App dispatch. A selection that hands nothing off SHALL keep its
current path.

#### Scenario: A selection that would hand off asks

- **WHEN** the user selects a playback target while a slot is active on the current output and
  the target is not that output
- **THEN** the Client asks "Continue playback on the target?" with Yes and No
- **AND** no output selection reaches the Owner until the user answers

#### Scenario: Yes runs today's hand-off

- **WHEN** the user answers Yes
- **THEN** the Owner performs the hand-off exactly as an unasked selection would

#### Scenario: No cancels

- **WHEN** the user answers No
- **THEN** nothing changes and the current output keeps playing

#### Scenario: Nothing active, or the no-op selection, skips the prompt

- **WHEN** nothing is active on the current output, or the selected target already is the output
- **THEN** the selection runs its current path without a prompt

#### Scenario: The local target asks while a remote output plays

- **WHEN** the user selects the local target while a Cast receiver or an Emby Session holds an
  active slot
- **THEN** the Client asks before it detaches the remote output, and No leaves it playing

#### Scenario: The direct-endpoint fork keeps its own decline

- **WHEN** the user selects an Emby Session that advertises a direct endpoint and the hand-off
  fork applies
- **THEN** the fork asks instead of the continue-playback prompt
- **AND** declining it keeps attaching the Session's direct endpoint with the prior output
  playing

## MODIFIED Requirements

### Requirement: Playback targets are mutually exclusive

Connecting to a new playback target SHALL sever the currently connected one, sending the outgoing target's owner no playback commands: it keeps playing. A cast output, an Emby Session output, a direct remote, and a library route are mutually exclusive Client targets. Selecting one SHALL return the Client to its Local daemon owner before the owner selects it, and one remote output replacing the other hands off the active slot — after the Client asked first, as `A hand-off asks the user first` requires. A receiver becomes the output only by explicit user selection.

#### Scenario: Cast target is selected while a direct remote or library route is connected

- **WHEN** the user selects a cast target while mbv has a direct remote or a library route
- **THEN** the Client asks first when the local Owner holds an active slot behind that
  connection, and the Yes answer severs it
- **AND** the Local daemon owner makes the receiver its active output

#### Scenario: Cast target is selected while a session is watched

- **WHEN** the user selects a cast target while an Emby Session is the owner's output
- **THEN** the Client asks first while a slot is active on the Session
- **AND** after the Yes answer the Session stops and the receiver continues the active slot

#### Scenario: A session or direct remote is connected while a cast target is attached

- **WHEN** the user connects to Direct remote control (Sessions panel or auto-reconnect)
  while a cast target is the active output
- **THEN** the severed link sends no playback commands and the receiver keeps playing
- **AND** the newly connected target becomes the one active target

#### Scenario: An Emby Session is selected while a cast target is attached

- **WHEN** the user selects an Emby Session in the Sessions panel while a cast receiver is the output
- **THEN** the Client asks first while a slot is active on the receiver (the hand-off fork for a
  Session that advertises a direct endpoint, the continue-playback prompt for one that does not)
- **AND** after the Yes answer the owner makes the Session its output: the receiver stops
- **AND** the Session continues the active slot

#### Scenario: The hand-off fork is offered before a Session switch

- **WHEN** the user selects an Emby Session that advertises a direct endpoint while a slot is active on the local Owner's output and the Owner advertises `emby-session-output`
- **THEN** the Client asks whether to hand off before it switches
- **AND** accepting makes the Session the Owner's output: the Owner stops the old output and the Session continues the active slot at its position

#### Scenario: Declining the hand-off keeps the prior output playing

- **WHEN** the user declines the hand-off fork
- **THEN** the Client attaches the Session's direct endpoint
- **AND** the prior output keeps playing

#### Scenario: Nothing active on the output skips the fork

- **WHEN** the user selects an Emby Session that advertises a direct endpoint while nothing is active on the local Owner's output
- **THEN** the plain switch runs without asking
- **AND** the same holds for the continue-playback prompt: nothing active, or the Session's
  device already the output (the Owner's no-op hand-off), selects without asking

#### Scenario: A different cast target is selected while one is attached

- **WHEN** the user selects a cast target while another cast receiver is the output
- **THEN** the Client asks first while a slot is active on the current receiver
- **AND** after the Yes answer the previous receiver stops, its connection is dropped, and the
  new receiver continues the active slot

#### Scenario: mbv launches

- **WHEN** the owner starts, regardless of `auto_reconnect` or any previously attached
  receiver
- **THEN** no cast receiver is the output until the user selects one

### Requirement: Selecting a cast target makes it the owner's output

Selecting a cast target SHALL ask the Local daemon owner to make that receiver its active playback output — after the Client asked first, as `A hand-off asks the user first` requires — with the hand-off that `owner-playback-output` defines. The owner SHALL hold the receiver connection. A Client SHALL hold no connection to the receiver. Detaching from a cast target SHALL select local playback as the output, after the same ask while the receiver holds an active slot.

#### Scenario: Cast target is selected while local playback plays

- **WHEN** the user selects a cast target while local playback plays a slot
- **THEN** the Client asks first, and the Yes answer connects the receiver and continues the slot on it
- **AND** local playback stops

#### Scenario: Playing while a receiver is the output

- **WHEN** the user plays a selection while a receiver is the active output
- **THEN** the owner installs the selection as its queue and loads only the selected slot onto the receiver
- **AND** local playback does not start

#### Scenario: Detaching from a cast target

- **WHEN** the user detaches from the cast target
- **THEN** the Client asks first while a slot is active on the receiver, and the Yes answer makes local playback the output and continues the active slot
- **AND** the receiver stops
