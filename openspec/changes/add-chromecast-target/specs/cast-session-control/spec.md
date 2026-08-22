## Purpose

Governs attaching mbv to a Google Cast receiver, handing items to the receiver's own
queue, controlling and displaying playback on it, and what becomes of the session when
mbv exits and starts again.

## ADDED Requirements

### Requirement: Attaching to a cast target does not engage the local player

Selecting a cast target SHALL attach mbv to that receiver without starting, stopping, or
reconfiguring the local media player, and SHALL NOT create a player target. While a cast
target is attached, playing a selection SHALL dispatch it to the receiver instead of the
local player.

#### Scenario: Cast target is selected

- **WHEN** the user selects a cast target
- **THEN** mbv attaches to that receiver
- **AND** the local media player is neither started nor reconfigured

#### Scenario: Playing while attached

- **WHEN** the user plays a selection while a cast target is attached
- **THEN** mbv dispatches the selection to the receiver
- **AND** SHALL NOT begin local playback of it

#### Scenario: Detaching from a cast target

- **WHEN** the user detaches from a cast target
- **THEN** subsequent playback uses the local player
- **AND** the receiver is left as it is

### Requirement: The receiver owns what it plays

mbv SHALL dispatch the items of a played selection to the receiver's own queue and SHALL
NOT project, track, or reconcile that queue afterwards. Advancement between dispatched
items SHALL be performed by the receiver. Changes to mbv's queue after dispatch SHALL NOT
alter what the receiver plays.

#### Scenario: Multi-item selection is dispatched

- **WHEN** the user plays a selection of several items while attached to a cast target
- **THEN** mbv dispatches those items to the receiver
- **AND** the receiver advances between them without further instruction from mbv

#### Scenario: mbv's queue changes after dispatch

- **WHEN** the user reorders or removes items in mbv's queue after dispatching to a
  receiver
- **THEN** what the receiver plays is unchanged

#### Scenario: A new selection is played

- **WHEN** the user plays a different selection while items are already playing on the
  receiver
- **THEN** mbv dispatches the new selection, replacing what the receiver holds

### Requirement: Transport commands control the attached receiver

While a cast target is attached, mbv SHALL route play, pause, stop, seek, next, previous,
volume, and mute commands to that receiver, using the same key bindings that control other
attached targets. Subtitle-track selection is not supported for cast targets in v1 (see
`cast-media-dispatch`).

#### Scenario: Transport key is pressed while attached

- **WHEN** the user presses a transport key while a cast target is attached
- **THEN** mbv sends the corresponding command to the receiver

#### Scenario: Command is not supported by the receiver

- **WHEN** the receiver reports it cannot perform a requested command
- **THEN** mbv surfaces the failure and leaves its displayed state governed by the
  receiver's reported status

### Requirement: Displayed playback state comes from the receiver

While a cast target is attached, mbv SHALL present now-playing title, position, duration,
and paused state from the receiver's reported status rather than from local playback
state.

#### Scenario: Receiver is playing

- **WHEN** the receiver reports a playing item
- **THEN** mbv presents that item's title, position, duration, and playing state

#### Scenario: Receiver is idle

- **WHEN** the receiver reports no active media
- **THEN** mbv presents no active playback for that target

#### Scenario: Receiver plays something mbv did not dispatch

- **WHEN** the receiver reports media that mbv did not dispatch
- **THEN** mbv presents the receiver's reported state without treating it as an error

### Requirement: Playback position is extrapolated between status reports

mbv SHALL present a continuously advancing position derived from the receiver's last
reported position, elapsed wall-clock time, and reported playback rate, and SHALL correct
it against periodic status reports. mbv SHALL NOT advance the presented position while the
receiver reports a paused, buffering, or stalled state.

#### Scenario: Steady playback between status reports

- **WHEN** the receiver is playing and no new status report has arrived
- **THEN** mbv advances the presented position from the last report and elapsed time

#### Scenario: Receiver stalls

- **WHEN** the receiver reports a buffering or paused state
- **THEN** mbv holds the presented position until playback resumes

#### Scenario: Extrapolation has drifted

- **WHEN** a status report disagrees with the extrapolated position
- **THEN** mbv adopts the reported position

### Requirement: Progress is reported while mbv is attached

While attached to a cast target and receiving status, mbv SHALL report playback progress
and completion to the provider of the item the receiver is playing, when mbv can identify
that item as one it dispatched.

#### Scenario: Dispatched item progresses

- **WHEN** the receiver reports progress on an item mbv dispatched
- **THEN** mbv reports that progress to the item's provider

#### Scenario: Playing item is unrecognised

- **WHEN** the receiver reports progress on media mbv cannot identify
- **THEN** mbv SHALL NOT report progress for any item

#### Scenario: mbv is not attached

- **WHEN** mbv is not attached to the receiver
- **THEN** mbv reports no progress for what the receiver plays

### Requirement: Exiting mbv leaves the receiver playing

When mbv exits while attached to a cast target, it SHALL leave the receiver playing and
SHALL NOT stop it or tear down its session.

#### Scenario: mbv exits while the receiver is playing

- **WHEN** mbv exits while attached to a playing receiver
- **THEN** the receiver continues playing
- **AND** mbv stops reporting progress for it

### Requirement: mbv reattaches to a running cast session on launch

When automatic reconnection is enabled, mbv SHALL attempt on launch to reattach to a cast
receiver it was attached to at exit, restoring control and displayed state from the
receiver's reported status. Reattaching SHALL NOT dispatch items or alter what the
receiver is playing. When automatic reconnection is disabled, mbv SHALL NOT attach to any
cast receiver on launch.

#### Scenario: Reattach enabled and receiver still playing

- **WHEN** mbv launches with automatic reconnection enabled and the persisted receiver is
  still playing
- **THEN** mbv reattaches, presents the receiver's reported state, and resumes reporting
  progress for items it can identify

#### Scenario: Reattach enabled and receiver is idle

- **WHEN** mbv launches with automatic reconnection enabled and the persisted receiver is
  idle
- **THEN** mbv attaches and presents no active playback
- **AND** SHALL NOT dispatch anything

#### Scenario: Reattach disabled

- **WHEN** mbv launches with automatic reconnection disabled
- **THEN** mbv SHALL NOT attach to any cast receiver until the user selects one

### Requirement: Losing the receiver connection is isolated from mbv

If the connection to an attached receiver drops, mbv SHALL log the diagnostic, present the
target as disconnected, stop reporting progress for it, and keep the queue, panel, and
input handling running.

#### Scenario: Connection drops while attached

- **WHEN** the connection to the attached receiver is lost
- **THEN** mbv presents the target as disconnected and remains usable
- **AND** SHALL NOT discard mbv's queue
