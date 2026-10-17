# Spec Delta

## MODIFIED Requirements

### Requirement: Playback targets are mutually exclusive

Connecting to a new playback target SHALL sever the currently connected one. A cast output, an Emby Session output, a direct remote, and a library route are mutually exclusive targets of a Client. Selecting a cast target or an Emby Session SHALL return the Client to its Local daemon owner before the owner selects it. When one remote output replaces the other, the owner SHALL hand off the active slot. The owner SHALL NOT select a cast receiver when it starts: a receiver becomes the output only by explicit user selection.

#### Scenario: Cast target is selected while a direct remote or library route is connected

- **WHEN** the user selects a cast target while mbv has a direct remote or a library route
- **THEN** that connection is severed
- **AND** the Local daemon owner makes the receiver its active output

#### Scenario: Cast target is selected while a session is watched

- **WHEN** the user selects a cast target while an Emby Session is the owner's output
- **THEN** the Session stops
- **AND** the receiver continues the active slot

#### Scenario: A session or direct remote is connected while a cast target is attached

- **WHEN** the user connects to Direct remote control (Sessions panel or auto-reconnect)
  while a cast target is the active output
- **THEN** the owner stops the receiver and selects local playback as its output
- **AND** local playback does not start
- **AND** the newly connected target becomes the one active target

#### Scenario: An Emby Session is selected while a cast target is attached

- **WHEN** the user selects an Emby Session in the Sessions panel while a cast receiver is the output
- **THEN** the receiver stops
- **AND** the Session continues the active slot

#### Scenario: A different cast target is selected while one is attached

- **WHEN** the user selects a cast target while another cast receiver is the output
- **THEN** the previous receiver stops and its connection is dropped
- **AND** the new receiver continues the active slot

#### Scenario: mbv launches

- **WHEN** the owner starts, regardless of `auto_reconnect` or any previously attached
  receiver
- **THEN** no cast receiver is the output until the user selects one
