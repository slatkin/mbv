# Spec Delta

## ADDED Requirements

### Requirement: The window opens with every section

When the GUI window opens, it SHALL ask the Client core for its sections. The Client core SHALL answer with one update that carries every section that is present, the theme included. Before the Owner attach completes, the answer SHALL carry the attach state and the theme.

#### Scenario: The window opens after the attach

- **GIVEN** the Owner attach completed and the queue holds slots
- **WHEN** the window opens
- **THEN** the window paints the theme, the queue, and the playback panel as the core holds them, with no input from the user

#### Scenario: The window opens during the attach

- **GIVEN** the Owner attach waits for an Owner that shuts down
- **WHEN** the window opens
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

The window SHALL derive every value that follows the playback position or the wall clock from the last playhead and its own clock: the elapsed and remaining times, the finish times, the active queue row's progress, and the current Audiobooks chapter. The Client core SHALL send none of these values. The window SHALL redraw for these values only when a painted value changes.

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

#### Scenario: Paused playback draws nothing

- **GIVEN** playback is paused and no finish time is painted
- **WHEN** the user gives no input and no update arrives
- **THEN** the window does not redraw

### Requirement: The Client core holds the shared surfaces' interaction state

The interaction state of the player panel and the Status bar (focus, cursor, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) and Panel focus SHALL live in the Client core. For these surfaces the window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it.

#### Scenario: The window is resized

- **GIVEN** the queue cursor is on a slot and the queue has focus
- **WHEN** the user resizes the window across a panel state threshold
- **THEN** the queue has focus and the cursor is on the same slot

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router

### Requirement: The window content holds the Library panel's interaction state

The Library panel's interaction state (cursor, hover, scroll, open tree nodes, focus inside the panel, filter and Search text, the open Hero overlay and item menu) SHALL live in the window content. The Client core SHALL hold the data selection: the active destination, each destination's selected collection, and the Hero's item. The window content SHALL change the data selection only through the Client core.

#### Scenario: A list update keeps the window's state

- **GIVEN** Music is active with Hip-Hop selected, the list scrolled, and a tree node open
- **WHEN** an update carries the same Library list
- **THEN** the list's cursor, scroll, and open nodes stay as they were

#### Scenario: The TUI Client browses a library

- **WHEN** the user browses any destination in the TUI Client
- **THEN** every key, pointer gesture, and painted frame behaves as before this change

## RENAMED Requirements

- FROM: `### Requirement: Library art comes through the frame`
- TO: `### Requirement: Artwork comes from the core's image cache`

## MODIFIED Requirements

### Requirement: GUI pointer input resolves through semantic targets

A pointer press, drag, or wheel step on the player panel SHALL resolve to the semantic target that the GUI painted at that point in the current frame. The effect SHALL be the gesture that the design canvas defines. Where the canvas is silent and the TUI has a matching gesture, the effect SHALL be the same. Pointer input on the Library panel is the window content's own (the requirement "The window content holds the Library panel's interaction state").

#### Scenario: The slice gestures

- **WHEN** the user uses the pointer on the panel
- **THEN** the panel supports row selection, row activation, the row-actions button and menu, row drag, wheel scrolling, the transport controls, the scrubber, the track chips and their popovers, the playback-target chooser, and the collapse and fold controls

#### Scenario: A queue row is clicked

- **WHEN** the user clicks a painted queue row
- **THEN** the queue cursor moves to that slot, as a click does in the TUI Client

#### Scenario: The wheel moves over the queue

- **WHEN** the user scrolls the wheel over the queue
- **THEN** the queue viewport moves by the uniform wheel step and the cursor follows the TUI wheel rules

#### Scenario: A control is clicked in a surface without focus

- **GIVEN** the queue has focus
- **WHEN** the user clicks the play/pause control
- **THEN** playback toggles

#### Scenario: A press lands on no target

- **WHEN** the user clicks a point with no painted target
- **THEN** nothing changes

### Requirement: The GUI Client is a separate program

mbv SHALL ship the GUI Client as a program separate from the `mbv` terminal program. The `mbv` program SHALL NOT load or link the windowing and GPU rendering stack that the GUI Client uses. The GUI Client SHALL draw its window itself and SHALL NOT embed a web engine.

#### Scenario: The terminal program carries no window stack

- **WHEN** the `mbv` program is built
- **THEN** its dependency graph contains no windowing or GPU rendering library that the GUI Client uses

#### Scenario: The GUI Client starts on its own

- **WHEN** the user starts the GUI Client program
- **THEN** it opens one native window and presents mbv in it without a terminal

#### Scenario: The GUI Client runs in one process

- **WHEN** the GUI Client window shows
- **THEN** the GUI Client runs no helper process for its window content

### Requirement: The GUI program ships beside the terminal program

Every release artifact (the tarball, the `.deb`, and the Arch packages) SHALL install the `mbv-gui` program beside the `mbv` program, and SHALL declare the runtime libraries that the GUI program loads: the Vulkan loader, the Wayland client library, and xkbcommon. No artifact SHALL declare a web engine. The GUI Client finds and starts its Owner from the sibling `mbv` program, and the build-id handshake refuses a mismatched pair, so both programs SHALL come from one build.

#### Scenario: A package installs both programs

- **WHEN** the tarball, the `.deb`, or an Arch package is installed
- **THEN** `mbv` and `mbv-gui` are installed in the same directory

#### Scenario: A package declares the GUI runtime

- **WHEN** the `.deb` or an Arch package is inspected
- **THEN** it declares the Vulkan loader, the Wayland client library, and xkbcommon
- **AND** it declares no WebKitGTK package

### Requirement: The GUI Client uses the typography and colours of the canvas

Text SHALL use IBM Plex Sans. Times, key hints, chips, and counters SHALL use JetBrains Mono with tabular figures. Both faces SHALL ship inside the GUI program, and the window SHALL load no font from the network or from the system font set. Every clock the panel paints SHALL use the canvas's zero-padded 24-hour `HH:MM` form, and every minus sign in a time SHALL be U+2212. The queue, row menu, popover, and drag visuals SHALL paint the canvas colours that name no `mbv-theme` slot through GUI-local canvas tokens in the GUI program, and the TUI's palette SHALL NOT change.

#### Scenario: Fonts load with no network

- **GIVEN** the machine has no network
- **WHEN** the GUI Client window opens
- **THEN** text and times show in the two bundled faces

#### Scenario: A finish clock passes midnight

- **GIVEN** the queue's finish time falls on the next day
- **WHEN** the panel paints a finish clock
- **THEN** the time shows as zero-padded 24-hour `HH:MM`, and the queue finish clock carries the canvas's next-day suffix

#### Scenario: Queue canvas colours resolve locally

- **WHEN** the queue, row menu, popover, or drag visuals paint a colour that names no `mbv-theme` slot
- **THEN** the value is the canvas colour, read from the GUI program's canvas tokens
- **AND** `mbv-theme`'s slot table and the TUI's palette stay unchanged

### Requirement: Artwork comes from the core's image cache

The window SHALL paint artwork (the Library art, the queue card art, and the trickplay sheets) only from the mbv image cache. A section SHALL carry a reference to an image, never the image's bytes. The window SHALL load a referenced image from the Client core in its own process, and the Client core SHALL answer from its image cache. The window SHALL make no network request for an image.

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

## REMOVED Requirements

### Requirement: The window content holds no interaction state

**Reason**: The GPUI window has no content that reloads apart from the program, so the reload scenario has no meaning. The rule itself stays.
**Migration**: The ADDED requirement "The Client core holds the shared surfaces' interaction state" carries the same rule with a resize scenario.

### Requirement: The Library panel's interaction state lives in the window content

**Reason**: The GPUI window has no content that reloads apart from the program, so the reload scenario has no meaning. The rule itself stays.
**Migration**: The ADDED requirement "The window content holds the Library panel's interaction state" carries the same rule with an update scenario.
