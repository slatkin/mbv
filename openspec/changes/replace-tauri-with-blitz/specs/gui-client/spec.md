## MODIFIED Requirements

### Requirement: The GUI Client is a separate program

mbv SHALL ship the GUI Client as a program separate from the `mbv` terminal program. The `mbv` program SHALL NOT load or link the windowing and rendering stack that the GUI Client uses. The GUI Client SHALL run no web engine process and no script engine.

#### Scenario: The terminal program carries no window stack

- **WHEN** the `mbv` program is built
- **THEN** its dependency graph contains no windowing or rendering library that the GUI Client uses

#### Scenario: The GUI Client starts on its own

- **WHEN** the user starts the GUI Client program
- **THEN** it opens one native window and presents mbv in it without a terminal

#### Scenario: The GUI Client is one process

- **WHEN** the GUI Client window is open
- **THEN** the GUI Client runs as one process with no web engine child process

### Requirement: The GUI program ships beside the terminal program

Every release artifact (the tarball, the `.deb`, and the Arch packages) SHALL install the `mbv-gui` program beside the `mbv` program, and SHALL declare the Vulkan loader and fontconfig runtime dependencies. No artifact SHALL declare a WebKitGTK dependency. The GUI Client finds and starts its Owner from the sibling `mbv` program, and the build-id handshake refuses a mismatched pair, so both programs SHALL come from one build.

#### Scenario: A package installs both programs

- **WHEN** the tarball, the `.deb`, or an Arch package is installed
- **THEN** `mbv` and `mbv-gui` are installed in the same directory

#### Scenario: A package declares the render stack

- **WHEN** the `.deb` or an Arch package is inspected
- **THEN** it depends on the Vulkan loader and fontconfig
- **AND** it does not depend on WebKitGTK

### Requirement: The GUI Client uses the typography and colours of the canvas

Text SHALL use IBM Plex Sans. Times, key hints, chips, and counters SHALL use JetBrains Mono with tabular figures. Both faces SHALL ship with the GUI Client, and the window SHALL fetch no font at runtime. Every clock the panel paints SHALL use the canvas's zero-padded 24-hour `HH:MM` form, and every minus sign in a time SHALL be U+2212. The queue, row menu, popover, and drag visuals SHALL paint the canvas hexes that name no `mbv-theme` slot through GUI-local `--canvas-*` tokens in the window's stylesheet, and the TUI's palette SHALL NOT change.

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
- **THEN** the value is the canvas hex, read from the GUI-local `--canvas-*` tokens of the window's stylesheet
- **AND** `mbv-theme`'s slot table and the TUI's palette stay unchanged

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

## REMOVED Requirements

### Requirement: The window content holds no interaction state

**Reason**: Its scenario "The window content reloads" needs a window-content reload, and the Blitz window has none. The rule itself stays.
**Migration**: The rule moves unchanged to "The Client core holds the player panel's interaction state".

### Requirement: The Library panel's interaction state lives in the window content

**Reason**: Its scenario "The window content reloads on a Library item" needs a window-content reload, and the Blitz window has none. The rule itself stays.
**Migration**: The rule moves unchanged to "The window content holds the Library panel's interaction state".

### Requirement: A window that mounts receives every section

**Reason**: Its reload sentence and its scenario "The window content reloads" need a window-content reload, and the Blitz window has none.
**Migration**: The rule, without the reload, moves to "A window that opens receives every section".

## ADDED Requirements

### Requirement: The Client core holds the player panel's interaction state

The interaction state of the player panel and the Status bar (focus, cursor, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) SHALL live in the Client core, not in the window content. For these surfaces the window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it. Panel focus between the Library panel and the Queue panel SHALL live in the Client core.

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router

### Requirement: Layout and the predicted playhead are not interaction state

The layout that the container size picks SHALL NOT count as interaction state. The position that the window advances between two playhead updates SHALL be a prediction of the Client core's playhead, not interaction state. The Library panel's interaction state is the exception that the requirement "The window content holds the Library panel's interaction state" defines.

#### Scenario: A resize picks a layout

- **WHEN** the window content picks a panel state from its container size
- **THEN** the Client core's collapse and queue-fold toggles do not change

### Requirement: The window content holds the Library panel's interaction state

The Library panel's interaction state (cursor, hover, scroll, open tree nodes, focus inside the panel, filter and Search text, the open Hero overlay and item menu) SHALL live in the window content. The Client core SHALL hold the data selection: the active destination, each destination's selected collection, and the Hero's item. The window content SHALL change the data selection only through the Client core.

#### Scenario: The TUI Client browses a library

- **WHEN** the user browses any destination in the TUI Client
- **THEN** every key, pointer gesture, and painted frame behaves as before this change

### Requirement: A window that opens receives every section

When the window content mounts, it SHALL ask the Client core for its sections. The Client core SHALL answer with one update that carries every section that is present. Before the Owner attach completes, the answer SHALL carry the attach state and the theme.

#### Scenario: The window opens on an attached Owner

- **GIVEN** the Owner holds queue slots and the GUI launch location names a picked album
- **WHEN** the GUI Client window opens and the attach completes
- **THEN** the window asks for its sections once when it mounts
- **AND** the window paints the theme, the queue, the playback panel, the Library list, and the Hero as the core holds them

#### Scenario: The window mounts during the attach

- **GIVEN** the Owner attach waits for an Owner that shuts down
- **WHEN** the window content mounts
- **THEN** the window paints the waiting state in the theme

#### Scenario: A layout report changes nothing

- **WHEN** the window reports a layout that moves no list capacity and no region a section reads
- **THEN** the window receives no update
