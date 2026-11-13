# Spec Delta

## ADDED Requirements

### Requirement: The window updates only the sections that changed

The Client core SHALL send the window an update only when the content of at least one window section changed. An update SHALL carry only the changed sections, each with its own revision. The window SHALL replace only the sections that an update names and SHALL keep every other section as it was. The Client core SHALL NOT send an update only because a timer fired. When only time passed, it SHALL send an update only when a toast expires or a Library deadline passes.

#### Scenario: Nothing changes while the window is idle

- **GIVEN** playback is paused or stopped and no toast is shown
- **AND** no Library deadline is pending
- **WHEN** the user gives no input and no Service or Owner event arrives
- **THEN** the window receives no update

#### Scenario: A Library deadline passes

- **GIVEN** the user selected a Library row and gives no more input
- **WHEN** the navigation-idle gate of the Hero neighbour warm-up passes, or the retry time of an interrupted art fetch passes
- **THEN** the window receives an update that carries only the Library sections whose content changed

#### Scenario: One section changes

- **GIVEN** the Library panel shows a list
- **WHEN** the user moves the queue cursor
- **THEN** the update carries the queue section
- **AND** the update does not carry the Library list

#### Scenario: Two sections change together

- **WHEN** the active slot changes
- **THEN** the queue section, the playback section, and the playhead arrive in one update
- **AND** the window never paints the new active slot in one section and the old one in another

#### Scenario: A pick changes only the detail

- **GIVEN** the Library list shows many rows
- **WHEN** the user picks a different row
- **THEN** the update carries the Library detail
- **AND** the update does not carry the Library list

### Requirement: A window that mounts receives every section

When the window content mounts, it SHALL ask the Client core for its sections. The Client core SHALL answer with one update that carries every section that is present. A window-content reload SHALL repaint the core's state, the theme included, without a state change behind it. Before the Owner attach completes, the answer SHALL carry the attach state and the theme.

#### Scenario: The window content reloads

- **GIVEN** the Library panel shows a picked album and the queue holds slots
- **WHEN** the window content reloads
- **THEN** the window asks for its sections when it mounts
- **AND** the window paints the theme, the queue, the playback panel, the Library list, and the Hero as the core holds them

#### Scenario: The window mounts during the attach

- **GIVEN** the Owner attach waits for an Owner that shuts down
- **WHEN** the window content mounts
- **THEN** the window paints the waiting state in the theme

#### Scenario: A layout report changes nothing

- **WHEN** the window reports a layout that moves no list capacity and no region a section reads
- **THEN** the window receives no update

### Requirement: The playhead advances in the window between discontinuities

The Client core SHALL send the playhead (the active slot, the position, the runtime, and the paused state) only when it is discontinuous: a track change, a pause or a resume, a runtime change, a seek, or a drift past a tolerance between the position the window predicts and the position the Owner reports. Between two sends the window SHALL advance the position itself while playback is not paused.

#### Scenario: Playback runs without input

- **GIVEN** an item plays
- **WHEN** the user gives no input for ten seconds
- **THEN** the scrubber and the elapsed and remaining times advance with playback
- **AND** the window receives no playhead update

#### Scenario: The user pauses

- **WHEN** the user pauses playback
- **THEN** the window receives the playhead with the paused state and the position at the pause
- **AND** the scrubber and the elapsed and remaining times stop

#### Scenario: The user seeks

- **WHEN** the user seeks forward 30 seconds
- **THEN** the window receives the playhead with the new position
- **AND** the scrubber and the elapsed and remaining times continue from it

#### Scenario: The reported position drifts

- **GIVEN** the window's predicted position and the Owner's newest reported position differ by more than the tolerance
- **WHEN** the Client core observes that report
- **THEN** the window receives the playhead with the reported position

#### Scenario: The output reports no new position

- **GIVEN** an item plays on an output that reports its position only from time to time
- **WHEN** no new report arrives
- **THEN** the window keeps advancing its predicted position
- **AND** the window receives no playhead update

### Requirement: The window derives the values that follow the position or the wall clock

The window SHALL derive every value that follows the playback position or the wall clock from the last playhead and its own clock: the elapsed and remaining times, the finish times, the active queue row's progress, and the current Audiobooks chapter. The Client core SHALL send none of these values.

#### Scenario: A minute passes

- **GIVEN** an item plays or is paused
- **WHEN** the wall clock passes to the next minute and the user gives no input
- **THEN** the finish times show the new minute
- **AND** the window receives no update

#### Scenario: Playback enters the next chapter

- **GIVEN** the Hero shows the playing book
- **WHEN** the playback position passes into the next chapter
- **THEN** the Hero marks the next chapter as the current chapter
- **AND** the window receives no update

## RENAMED Requirements

- FROM: `### Requirement: Library art comes through the frame`
- TO: `### Requirement: Artwork comes from the core's image cache`

## MODIFIED Requirements

### Requirement: The window content holds no interaction state

The interaction state of the player panel and the Status bar (focus, cursor, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) SHALL live in the Client core, not in the window content. For these surfaces the window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it. Panel focus between the Library panel and the Queue panel SHALL live in the Client core. The layout that the container size picks is not interaction state. The Library panel's interaction state is the exception that the requirement "The Library panel's interaction state lives in the window content" defines. The position that the window advances between two playhead updates is a prediction of the Client core's playhead, not interaction state.

#### Scenario: The window content reloads

- **GIVEN** the queue cursor is on a slot and the queue has focus
- **WHEN** the window content reloads
- **THEN** the window asks the Client core for its sections when it mounts
- **AND** the queue has focus and the cursor is on the same slot

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router

### Requirement: Artwork comes from the core's image cache

The window SHALL paint artwork (the Library art, the queue card art, and the trickplay sheets) only from the mbv image cache. A section SHALL carry a reference to an image, never the image's bytes. The window SHALL load a referenced image from the Client core, and the Client core SHALL answer from its image cache. The window SHALL make no network request for an image.

#### Scenario: An album is picked

- **WHEN** the user picks an album whose artwork is not yet cached
- **THEN** the Hero paints the artwork placeholder until a later update references the cached image
- **AND** the window content makes no network request for the artwork

#### Scenario: The same image shows again

- **GIVEN** the window already loaded an album's artwork
- **WHEN** the user picks a different album and then picks the first album again
- **THEN** no update carries the artwork's bytes

#### Scenario: A referenced image is no longer cached

- **GIVEN** an update referenced an image
- **WHEN** the Client core no longer holds that image when the window loads it
- **THEN** the window shows the image's loading state in its place
