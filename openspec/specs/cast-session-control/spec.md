# cast-session-control Specification

## Purpose

Governs attaching mbv to a Google Cast receiver, handing items to the receiver's own
queue, controlling and displaying playback on it, and what becomes of the session when
mbv exits.

## Requirements

### Requirement: Transport commands control the attached receiver

While a cast target is the active output, play, pause, stop, seek, next, previous, volume, and mute commands from any Client SHALL go to the owner, and the owner SHALL apply them to the receiver. The same key bindings SHALL control local playback and the receiver. Audio-track and subtitle-track selection are not supported for cast targets (see `cast-media-dispatch`). The owner SHALL reject them with a reason.

#### Scenario: Transport key is pressed while attached

- **WHEN** the user presses a transport key while a cast target is the active output
- **THEN** the owner sends the corresponding command to the receiver

#### Scenario: Command is not supported by the receiver

- **WHEN** the receiver reports it cannot perform a requested command
- **THEN** mbv surfaces the failure and leaves its displayed state governed by the
  receiver's reported status

#### Scenario: Track selection while attached

- **WHEN** the user cycles the audio or subtitle track while a cast target is the active output
- **THEN** the owner rejects the command with a reason and the receiver is unchanged

### Requirement: Displayed playback state comes from the receiver

While a cast target is the active output, the owner SHALL build the presented now-playing slot, position, duration, and paused state from the receiver's reported status, and SHALL broadcast that state to every Client in the same form as local playback state.

#### Scenario: Receiver is playing

- **WHEN** the receiver reports that the item the owner loaded is playing
- **THEN** every Client presents that slot's title, position, duration, and playing state

#### Scenario: Receiver is idle

- **WHEN** the receiver reports no active media
- **THEN** every Client presents no active playback for that target

#### Scenario: Receiver plays something mbv did not dispatch

- **WHEN** the receiver reports media that the owner did not load
- **THEN** mbv presents the receiver's reported state without treating it as an error
- **AND** the owner does not advance the queue or report progress for that media

### Requirement: Playback position is extrapolated between status reports

The owner SHALL present a continuously advancing position derived from the receiver's last reported position, elapsed wall-clock time, and reported playback rate, and SHALL correct it against periodic status reports. It SHALL NOT advance the presented position while the receiver reports a paused, buffering, loading, or stalled state.

#### Scenario: Steady playback between status reports

- **WHEN** the receiver is playing and no new status report has arrived
- **THEN** the owner advances the presented position from the last report and elapsed time

#### Scenario: Receiver stalls

- **WHEN** the receiver reports a buffering, loading, or paused state
- **THEN** the owner holds the presented position until playback resumes

#### Scenario: Extrapolation has drifted

- **WHEN** a status report disagrees with the extrapolated position
- **THEN** the owner adopts the reported position

### Requirement: Progress is reported while mbv is attached

While a cast target is the active output and the owner receives its status, the owner SHALL report playback progress and completion to the provider of the item it loaded onto the receiver. The owner SHALL report progress only when the receiver's reported media is the item it loaded.

#### Scenario: Dispatched item progresses

- **WHEN** the receiver reports progress on the item the owner loaded
- **THEN** the owner reports that progress to the item's provider

#### Scenario: Playing item is unrecognised

- **WHEN** the receiver reports progress on media the owner did not load
- **THEN** the owner SHALL NOT report progress for any item

#### Scenario: mbv is not attached

- **WHEN** the owner has no connection to the receiver
- **THEN** the owner reports no progress for what the receiver plays

#### Scenario: Output changes

- **WHEN** the active output changes away from the receiver
- **THEN** the owner reports the stop of the receiver's item to its provider before the new output reports a start

### Requirement: Exiting mbv leaves the receiver playing

The exit of a Client SHALL NOT affect the receiver. The owner SHALL continue to drive it and advance the queue. When the owner itself exits while a receiver is the output, it SHALL leave the receiver playing its current item and SHALL NOT stop it or tear down its session.

#### Scenario: mbv exits while the receiver is playing

- **WHEN** the last Client exits and the owner stays alive
- **THEN** the receiver continues playing
- **AND** the owner advances to the next slot when the item finishes

#### Scenario: Owner exits while the receiver is playing

- **WHEN** the owner exits while a receiver plays the active slot
- **THEN** the receiver continues playing its current item
- **AND** nothing advances it after that item

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

### Requirement: Losing the receiver connection is isolated from mbv

If the connection to the receiver drops, the owner SHALL log the diagnostic, present the target as disconnected, stop reporting progress for it, and keep its queue and active slot. On reconnect the owner SHALL join the receiver's running session and SHALL NOT restart the receiver app. If the active item finished while the connection was down and the receiver still reports that end, the owner SHALL advance.

#### Scenario: Connection drops while attached

- **WHEN** the connection to the receiver is lost
- **THEN** every Client presents the target as disconnected and remains usable
- **AND** the owner SHALL NOT discard its queue or active slot

#### Scenario: Reconnect while the item still plays

- **WHEN** the owner reconnects and the receiver still plays the item the owner loaded
- **THEN** playback on the receiver is not interrupted
- **AND** the owner resumes presenting its state

#### Scenario: Item finished during the outage

- **WHEN** the owner reconnects and the receiver reports that the loaded item finished
- **THEN** the owner advances to the next slot

#### Scenario: Receiver session is gone after reconnect

- **WHEN** the owner reconnects and the receiver has no session of the item the owner loaded
- **THEN** the owner does not advance and presents no active playback

### Requirement: Selecting a cast target makes it the owner's output

Selecting a cast target SHALL ask the Local daemon owner to make that receiver its active playback output, with the hand-off that `owner-playback-output` defines. The owner SHALL hold the receiver connection. A Client SHALL hold no connection to the receiver. Detaching from a cast target SHALL select local playback as the output.

#### Scenario: Cast target is selected while local playback plays

- **WHEN** the user selects a cast target while local playback plays a slot
- **THEN** the owner connects to the receiver and continues the slot on it
- **AND** local playback stops

#### Scenario: Playing while a receiver is the output

- **WHEN** the user plays a selection while a receiver is the active output
- **THEN** the owner installs the selection as its queue and loads only the selected slot onto the receiver
- **AND** local playback does not start

#### Scenario: Detaching from a cast target

- **WHEN** the user detaches from the cast target
- **THEN** local playback becomes the output and continues the active slot
- **AND** the receiver stops

### Requirement: The receiver plays only the owner's active slot

The owner SHALL give the receiver only the item of the active slot, one item at a time. The owner SHALL NOT send a message that carries more than one queue item to the receiver.

#### Scenario: Large queue is played

- **WHEN** the user plays a queue of several hundred items while a receiver is the output
- **THEN** the receiver is given only the selected item
- **AND** the receiver connection stays open

#### Scenario: Receiver finishes an item

- **WHEN** the receiver reports that the active item finished
- **THEN** the owner loads the next slot of its play order onto the receiver
