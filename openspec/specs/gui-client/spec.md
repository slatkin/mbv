# gui-client Specification

## Purpose
Defines the GUI Client: a Client that presents mbv in a native desktop window, with the same Owner relationship as the TUI Client.

## Requirements

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

### Requirement: The GUI Client attaches to the Owner process as a Client

The GUI Client SHALL be a Client of the Owner process under the same rules as a TUI Client. It SHALL find, start, and attach to the Owner process as a TUI Client does. It SHALL NOT hold the owner lock. It SHALL NOT become a Player owner. When the Owner process exits, the GUI Client SHALL follow `daemon-disconnect-handling`: an announced shutdown SHALL end the run, and an unannounced connection loss SHALL present the recovery dialog.

#### Scenario: No Owner process is running

- **WHEN** the GUI Client starts and no Owner process is running
- **THEN** the GUI Client starts the Owner process from the `mbv` program and attaches to it

#### Scenario: A TUI Client and a GUI Client are attached at once

- **GIVEN** Stay-alive is enabled and a TUI Client is attached
- **WHEN** the GUI Client starts
- **THEN** it attaches alongside the TUI Client without displacing it
- **AND** both show the same Bound queue and playback state

#### Scenario: Stay-alive is disabled and a Client is attached

- **GIVEN** Stay-alive is disabled and a TUI Client is attached
- **WHEN** the GUI Client starts
- **THEN** the GUI Client refuses to start with the same explanation a second TUI Client gets

#### Scenario: The Owner process is from a different build

- **GIVEN** a running Owner process from a different build
- **WHEN** the GUI Client starts
- **THEN** the window shows the mismatch and offers Restart and Quit, each on a key
- **AND** Restart stops that Owner process, starts a new one from the `mbv` program, and attaches to it

#### Scenario: Unannounced Owner loss presents the recovery dialog

- **GIVEN** the GUI Client is attached to the Owner process
- **WHEN** the connection closes with no announced shutdown
- **THEN** the window shows the recovery dialog with the last playing title, the daemon log path, and the Restart and resume, Restart, don't resume, and Quit choices, each on its key
- **AND** no other key and no press outside the dialog answers it, and the GUI Client does not exit on its own

### Requirement: The GUI Client never runs playback

The GUI Client SHALL NOT start, embed, or control mpv directly. It SHALL send playback and queue requests to the Owner process over ctrl. Video SHALL play in the window of the mpv child process, as it does for a TUI Client.

#### Scenario: Video plays from the GUI Client

- **WHEN** the user plays a video item from the GUI Client
- **THEN** the Owner process plays it in the mpv child process window
- **AND** the GUI Client window does not show the video

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The window SHALL show the top bar as one global header across the full window width, above the Library panel and the Queue column, in every arrangement; the Settings screen SHALL show no global header. The panel SHALL show no header row above its artwork: the artwork SHALL be the panel's topmost content, directly under the global header. The footer line SHALL show the playback-target chooser and the collapse control at its left edge. The GUI Client SHALL paint no now-playing status word on any surface. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection while the Library panel does not show; while the Library panel shows, the Library Status bar shows them. The footer line SHALL show the total time of the shown queue and, while rows are marked, the total time of the marked rows. A surface, or a Library destination, that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder: the top bar carries only the destination tabs that the GUI Client presents, and the Library Status bar carries the Search field only once Search is presented.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, the artwork with its overlays, the title, the queue header, the queue rows, and a footer line
- **AND** the footer line's left shows the playback target with a chooser and the collapse control
- **AND** the artwork shows the active item, with the track chips at its top left, the season and episode badge at its top right, and the player controls at its bottom
- **AND** the player controls have play/pause and the scrubber on one row, and previous, back 10 s, forward 30 s, next, and the auto-hide control on the row under it
- **AND** the queue lists the slots in canonical order, marks the start slot, and numbers every row with its 1-based position in the queue

#### Scenario: The Status bar projection holds a toast

- **WHEN** the shared Status bar projection holds a toast
- **THEN** the GUI Client shows the toast in the Library Status bar while the Library panel shows, and otherwise in the footer line or in a conventional GUI form

#### Scenario: The prefix is armed

- **WHEN** the user arms prefix mode
- **THEN** the Library Status bar shows the armed state while the Library panel shows, and otherwise the footer line shows it

#### Scenario: The queue is empty

- **GIVEN** the Owner process holds an empty queue
- **WHEN** the GUI Client window is open
- **THEN** the panel shows an empty-queue state in the type, colour, and spacing of the canvas

#### Scenario: A surface is not yet ported

- **WHEN** the user presses a key bound to a surface that the GUI Client does not present
- **THEN** no surface opens and no placeholder appears

#### Scenario: A destination is not yet presented

- **GIVEN** the GUI Client presents only the Music destination
- **WHEN** the top bar paints
- **THEN** it shows no tab for another destination, and a digit key for one opens nothing

#### Scenario: The artwork has a still slot

- **WHEN** the expanded panel paints the artwork of the active item
- **THEN** a loaded image fills the panel's width and takes its height from the artwork's aspect, as the TUI's visual slot paints at full column width
- **AND** no placeholder image and no source caption paint
- **AND** the season and episode badge sits at the top right of the artwork

#### Scenario: The footer paints the queue's source text

- **WHEN** the expanded panel paints the footer line
- **THEN** the left line shows the playback-target chooser and the collapse control, then the queue's existing source text — the playlist text the frame carries — without a leading icon, the right line shows the shown queue's total time, and the line uses the canvas footer's type and border
- **AND** the footer shows no Player-owner label and no liveness dot
- **AND** the prefix, toast, autosave, and dirty states still paint in the footer line when they are active and the Library panel does not show

#### Scenario: The footer's target chooser opens above the band

- **WHEN** the user presses the playback-target control in the footer line
- **THEN** the chooser's popover opens above the footer band, anchored to the pressed control, and lists the live choice list with the attached output marked
- **AND** Esc and a press outside the popover close it

#### Scenario: No now-playing status word paints

- **WHEN** the window paints at any playback state
- **THEN** no surface paints a now-playing status word

#### Scenario: The footer totals the marked rows

- **GIVEN** three rows of the shown queue are marked
- **WHEN** the footer line paints
- **THEN** it shows the total time of the three marked rows beside the total time of the queue

#### Scenario: No row is marked

- **WHEN** no row of the shown queue is marked
- **THEN** the footer shows only the total time of the queue

#### Scenario: A runtime is unknown

- **GIVEN** the shown queue holds a slot whose runtime is unknown
- **WHEN** the footer line paints
- **THEN** the total shows the sum of the known runtimes and marks it as a lower bound

#### Scenario: The filter narrows the rows

- **GIVEN** the Queue filter narrows the shown queue
- **WHEN** the footer line paints
- **THEN** the total time still covers every slot of the shown queue

#### Scenario: The header spans both columns

- **GIVEN** the Library panel and the Queue column show, or the Queue column is collapsed to its rail
- **WHEN** the window paints
- **THEN** the top bar spans the full window width, and the Library column and the Queue column or its rail start below it

#### Scenario: The Settings screen shows

- **WHEN** the Settings screen shows
- **THEN** no global header paints

### Requirement: The player controls overlay the artwork

In the expanded panel, the player controls SHALL paint over the bottom of the artwork and SHALL take no more than one third of the artwork's height. The controls SHALL scale down with the artwork's height to stay inside that limit. The track chips and the season and episode badge SHALL paint over the top of the artwork. A press on a control over the artwork SHALL act on that control, not on the artwork.

#### Scenario: A short image scales the controls

- **GIVEN** the artwork's height is less than three times the controls' full height
- **WHEN** the expanded panel paints
- **THEN** the controls scale down so that they take no more than one third of the artwork's height

#### Scenario: A press lands on a control over the artwork

- **WHEN** the user presses play/pause over the artwork
- **THEN** the play/pause request runs and the artwork's own press does not

#### Scenario: A track popover opens over the artwork

- **WHEN** the user opens the audio popover from its chip at the top of the artwork
- **THEN** the popover paints in full, also where it reaches past the bottom of the artwork

### Requirement: The player controls auto-hide when the user turns it on

The player controls over the artwork SHALL show at all times by default. The auto-hide control SHALL turn auto-hide on and off. With auto-hide on, the controls SHALL hide while an item plays and the pointer is not over the artwork, and SHALL show while the pointer is over the artwork and while the playback is paused. Auto-hide SHALL NOT hide the track chips or the badge. The choice SHALL persist across launches.

#### Scenario: Auto-hide is off

- **GIVEN** auto-hide is off
- **WHEN** an item plays and the pointer is away from the artwork
- **THEN** the player controls show

#### Scenario: Auto-hide hides the controls during playback

- **GIVEN** auto-hide is on
- **WHEN** an item plays and the pointer is away from the artwork
- **THEN** the player controls are hidden and the track chips and the badge still show

#### Scenario: The pointer comes over the artwork

- **GIVEN** auto-hide is on and an item plays
- **WHEN** the pointer moves over the artwork
- **THEN** the player controls show

#### Scenario: The playback is paused

- **GIVEN** auto-hide is on
- **WHEN** the playback is paused and the pointer is away from the artwork
- **THEN** the player controls show

#### Scenario: The choice persists

- **GIVEN** the user turned auto-hide on
- **WHEN** the GUI Client starts again
- **THEN** auto-hide is on

### Requirement: The player panel without an image shows the player alone

When the active item has no image, the expanded panel SHALL paint no artwork box. Under the global header it SHALL show the track chips and the badge on one line, then the player controls, then the title. The player controls SHALL always show in that case, whatever the auto-hide setting, and the auto-hide control SHALL still show and keep its setting.

#### Scenario: The item has no image

- **GIVEN** the active item has no image
- **WHEN** the expanded panel paints
- **THEN** no artwork box and no placeholder paint, and the track chips and badge line, the player controls, and the title paint in that order under the global header

#### Scenario: Auto-hide is on and the item has no image

- **GIVEN** auto-hide is on and the active item has no image
- **WHEN** an item plays and the pointer is away from the panel
- **THEN** the player controls show and the auto-hide control still shows that auto-hide is on

### Requirement: The stopped player shows its buttons

While a shown local queue holds slots and nothing plays, the expanded panel SHALL show the two control rows under the global header, with an empty scrubber that resolves no seek. The panel SHALL show no artwork, no title, and no track chips. A press on play/pause SHALL start the shown queue.

#### Scenario: Play from the stopped state

- **GIVEN** a shown local queue holds slots and nothing plays
- **WHEN** the user presses play/pause in the player controls
- **THEN** the shown queue starts from its start slot

#### Scenario: A shown peer queue is stopped

- **GIVEN** the shown queue is a peer's queue and nothing plays
- **WHEN** the expanded panel paints
- **THEN** the panel paints no control rows

#### Scenario: An empty shown local queue is stopped

- **GIVEN** a shown local queue holds no slots and nothing plays
- **WHEN** the expanded panel paints
- **THEN** the panel paints no control rows

### Requirement: GUI keyboard interaction matches the TUI Client

For every action that the player panel and the Status bar present, each key chord SHALL have the same effect as in the TUI Client, and these surfaces SHALL use the same `[keys]` configuration and the same keyboard router. The key hints that the canvas shows for them are the default bindings. The canvas chords `z` (collapse), `q` (queue fold), and `c` (cycle subtitle) bind in a GUI-only key context; the TUI Client's default bindings SHALL NOT change. The Library panel's keys are GUI keys that the requirement "The Library keys act on the hovered row" defines; a key that the Library panel does not take SHALL reach the keyboard router.

#### Scenario: A configured binding is used in both Clients

- **GIVEN** the user rebinds a queue action in `[keys]`
- **WHEN** the user presses the new chord in the GUI Client
- **THEN** the GUI Client runs that queue action, as the TUI Client does

#### Scenario: Prefix mode

- **WHEN** the user arms prefix mode in the GUI Client
- **THEN** the footer line shows the armed state and the next chord resolves in the prefix namespace, as in the TUI Client

### Requirement: Keys go to the pointer target

The GUI Client SHALL hold no Panel focus. At each key press it SHALL find the key's target under the pointer: a Library row, the Library column, or the Queue list box (its header, rows, and footer). A key that the target does not take SHALL go to the keyboard router. With no target, only the global chords SHALL act. A pointer move, a press, or a scroll SHALL NOT store a focus.

#### Scenario: A key acts where the pointer is

- **GIVEN** the user last pressed a Queue row
- **WHEN** the pointer rests on a Library row and the user presses `a`
- **THEN** the Library row is added to the queue
- **AND** the Queue's active row does not change

#### Scenario: A key over the Queue list box reaches the Queue

- **WHEN** the pointer rests over the Queue list box and the user presses `/`
- **THEN** the Queue filter opens

#### Scenario: A key with no target runs only the global chords

- **GIVEN** the pointer rests over the player block above the Queue list box
- **WHEN** the user presses Down, then Space
- **THEN** Down changes nothing
- **AND** Space plays or pauses

#### Scenario: A scroll moves a new row under a still pointer

- **GIVEN** the pointer rests over the Music list
- **WHEN** the wheel scrolls the list so that another track row is under the pointer, and the user presses Enter without a pointer move
- **THEN** the track row now under the pointer plays

#### Scenario: The pointer leaves the window

- **WHEN** the pointer is outside the window and the user presses a key
- **THEN** only the global chords act

### Requirement: Open text fields and menus keep the keys

While the Search field, the Library filter, the Queue filter, or the collection dropdown's filter field is open for typing, it SHALL take the keys wherever the pointer is. While a core overlay is open or prefix mode is armed, every key SHALL go to the Client core. Esc SHALL close the open Library row menu, the item menu, the Hero overlay, the collection dropdown, or the Search results wherever the pointer is.

#### Scenario: The pointer moves while the user types

- **GIVEN** the Library filter is open
- **WHEN** the pointer moves over the Queue list box and the user types `bo`
- **THEN** the Library filter holds `bo`
- **AND** the Queue does not change

#### Scenario: Esc closes the row menu away from the pointer

- **GIVEN** a Library row's menu is open
- **WHEN** the pointer rests over the Queue list box and the user presses Esc
- **THEN** the row menu closes
- **AND** the Queue does not change

#### Scenario: The dropdown filter keeps the keys

- **GIVEN** the collection dropdown is open
- **WHEN** the pointer moves over the Queue list box and the user types `ti`
- **THEN** the dropdown filter holds `ti`
- **AND** the Queue does not change

### Requirement: The Queue is a ledger with one active row

While the Queue holds slots, it SHALL have one active row, and the active row SHALL show at all times, wherever the pointer is. A press on a row, and the arrow, Page Up, Page Down, Home, and End keys over the Queue list box, SHALL move the active row. A hover SHALL NOT move it. Keys over the Queue list box SHALL act on the active row. The now-playing row, the active row, the hovered row, and each marked row SHALL be distinguishable.

#### Scenario: A hover does not move the active row

- **GIVEN** the third Queue row is the active row
- **WHEN** the pointer rests on the fifth Queue row
- **THEN** the third row stays the active row
- **AND** the fifth row shows as hovered

#### Scenario: The active row shows while the pointer is in the Library

- **WHEN** the pointer moves from the Queue list box into the Library column
- **THEN** the active row still shows as active

#### Scenario: An arrow key over the Queue moves the active row

- **GIVEN** the pointer rests over the Queue list box and the third row is the active row
- **WHEN** the user presses Down
- **THEN** the fourth row is the active row and it is fully visible

#### Scenario: Enter plays the active row, not the hovered row

- **GIVEN** the third Queue row is the active row and the pointer rests on the fifth row
- **WHEN** the user presses Enter
- **THEN** the third row plays

#### Scenario: The playing row and the active row differ

- **WHEN** the second row plays and the fourth row is the active row
- **THEN** each of the two rows shows its own state, and neither hides the other

### Requirement: Scrolling follows the cursor

The queue SHALL keep the cursor row fully visible after every keyboard movement. The queue SHALL use fixed row heights so that capacity is a whole number of rows.

#### Scenario: The cursor moves past the last visible row

- **WHEN** the user moves the queue cursor below the last fully visible row
- **THEN** the queue scrolls so that the cursor row is fully visible

### Requirement: Motion never delays input

Any animation in the GUI Client SHALL be feedback only. A key press SHALL take effect on the next frame, whether or not an animation is in progress.

#### Scenario: Keys arrive during an animation

- **WHEN** the user presses a movement key while a transition is in progress
- **THEN** the movement applies on the next frame and the transition does not queue or delay it

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

### Requirement: The panel changes state with its own size

The panel SHALL pick one of four states from its own size, not from the window size: expanded, compact strip, expanded without queue, or rail. The `z` key SHALL toggle the collapse and the `q` key SHALL toggle the queue fold. The Client core SHALL hold the two toggles. The front end SHALL pick the layout from the measured size of the panel container, the same measurement that it reports to the Client core in `LayoutReport`, so the painted layout, the list capacity, and the hit map follow one size.

#### Scenario: The container is shorter than 520 px

- **WHEN** the panel container is shorter than 520 px
- **THEN** the panel shows the compact strip: a 96×54 still, the title, the parent line with the next item, play/pause, next, and a thin progress line

#### Scenario: The queue is folded

- **WHEN** the user presses `q` in the expanded panel
- **THEN** the queue rows are replaced by one line with the remaining count, the queue finish clock, and a control that expands the queue

#### Scenario: The panel is collapsed to the rail

- **WHEN** the user presses `z`
- **THEN** the panel shows the 72 px rail: a progress ring around the artwork, play/pause, next, the remaining time, and a control that expands the panel
- **AND** pressing `z` again restores the previous state

### Requirement: The scrubber seeks and previews

The scrubber SHALL show the progress, the chapter boundaries, the intro and credits marks, and a thumb, with the elapsed time and the remaining time with the finish clock. A press on the bar SHALL seek to that position. The back 10 s and forward 30 s chords SHALL do the same work as the transport buttons.

#### Scenario: The pointer hovers the bar

- **WHEN** the pointer is over the scrubber bar
- **THEN** a preview shows the trickplay frame, the time at the pointer, and the chapter name
- **AND** focus and playback do not change

#### Scenario: The bar is pressed

- **WHEN** the user presses the bar at a position
- **THEN** the Owner process seeks to that position

#### Scenario: No trickplay frames exist

- **GIVEN** the active item has no trickplay data
- **WHEN** the pointer is over the scrubber bar
- **THEN** the preview shows the time and the chapter without a frame

### Requirement: Track chips open popovers

The panel SHALL show one chip each for the video stream, the audio track, and the subtitle track of the active item. The audio and subtitle chips SHALL each open a popover that lists the choices and marks the chosen one, and the open chip SHALL show the canvas's open state. The video chip's press SHALL change nothing: the video chip shows the resolved stream label only and SHALL NOT open a popover. The chosen track SHALL be sent to the Owner process. The `c` key SHALL cycle the subtitle track.

#### Scenario: A subtitle track is chosen

- **WHEN** the user opens the subtitle popover and chooses a track
- **THEN** the Owner process switches to that track and the popover marks it
- **AND** the popover lists Off and each subtitle track

#### Scenario: The open chip shows its state

- **WHEN** a chip's popover is open
- **THEN** that chip paints the canvas's open state and the other two do not

#### Scenario: The video chip is pressed

- **WHEN** the user presses the video chip
- **THEN** no popover opens and no request leaves the Client

### Requirement: The queue shows a row menu and drag reorder

The queue header SHALL hold only what the requirement "The queue header holds the chooser and its controls" (`gui-queue-chooser`) names. Rows SHALL have a fixed height of 30 px and SHALL carry the canvas's one-line row: its queue ordinal, its title line, and its progress where the state has one. A row SHALL carry no right-column value. The shown queue's start slot SHALL be marked as playing while its queue plays and as the start of playback while its queue is stopped or resting, and the two marks SHALL be distinct. A shown queue whose owner reports no start slot, such as a peer without named queues, SHALL carry the playing mark on its observed active slot, and no start mark. Every queue edit SHALL be a request to the Player owner of the shown queue. The window content SHALL NOT change the canonical queue.

#### Scenario: Rows show their state

- **WHEN** the queue holds a watched row, the active row, and a row with partial progress
- **THEN** the watched row is struck through, the active row is filled, and the partial row shows a thin progress bar under its title
- **AND** each row carries its 1-based queue ordinal as the canvas paints it

#### Scenario: The right column is switched

- **WHEN** the queue header and the queue rows paint
- **THEN** the header shows no right-column switch and no row shows a length or an end time
- **AND** no GUI key switches a right column

#### Scenario: The queue header counts the queue

- **WHEN** the queue header paints
- **THEN** it shows no remaining count and no queue finish clock

#### Scenario: A stopped queue marks its start slot

- **GIVEN** the shown queue is stopped with its third slot as its start slot
- **WHEN** the queue rows paint
- **THEN** the third row carries the start mark, and no row carries the playing mark

#### Scenario: A peer queue marks its playing row

- **GIVEN** the GUI Client shows a peer that reports no start slot and plays its second slot
- **WHEN** the queue rows paint
- **THEN** the second row carries the playing mark

#### Scenario: Play turns the start mark into the playing mark

- **GIVEN** the shown queue is stopped with its third slot as its start slot
- **WHEN** the user presses Play
- **THEN** the third row carries the playing mark

#### Scenario: A row is hovered

- **WHEN** the pointer is over a queue row
- **THEN** the row paints the canvas's hovered row: the grip replaces the row's ordinal and the row-actions button shows at the row's end

#### Scenario: The row menu opens

- **WHEN** the row-actions button is pressed
- **THEN** the menu opens with Play now (`⏎`), Play next (`n`), Mark watched (`w`), Go to episode (`g`), and Remove from queue (`dd`)

#### Scenario: The destructive entry is distinct

- **WHEN** the row menu paints
- **THEN** Remove from queue paints in the canvas's destructive colour, separated from the other entries by the menu divider

#### Scenario: A row is removed from the menu

- **WHEN** the user chooses Remove from queue
- **THEN** the Player owner of the shown queue removes that slot from it

#### Scenario: A row is dragged

- **WHEN** the user drags a row to a new position and releases
- **THEN** the queue shows an insertion line and a drag ghost during the drag
- **AND** the Player owner of the shown queue moves the slot to the new position

#### Scenario: A row is moved by key

- **WHEN** the user presses `Alt+J` on a row
- **THEN** the row moves one place down, as the drag would do

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

### Requirement: The GUI presents the Library panel beside the Queue panel

The GUI Client SHALL present the Library panel and the Queue panel in one window. The Library panel's content, arrangement, and states SHALL follow the Library artboard of the design canvas, not the TUI Client. Neither panel SHALL hold a focus: a press in either panel SHALL act at once, and keys SHALL go to the pointer target (the requirement "Keys go to the pointer target"). While both panels show, the Library panel SHALL be at the left window edge and the Queue panel's column SHALL be at the right window edge. While the Queue panel is collapsed to its rail, the rail SHALL be at the right window edge.

#### Scenario: Both panels show

- **GIVEN** the window is wide enough for both panels
- **WHEN** the GUI Client window is open
- **THEN** the Library panel and the Queue panel both show
- **AND** neither panel shows as focused

#### Scenario: The Queue column is at the right window edge

- **GIVEN** the window is wide enough for both panels
- **WHEN** the GUI Client window is open
- **THEN** the Library panel starts at the left window edge
- **AND** the Queue panel's column ends at the right window edge, to the right of the Library panel

#### Scenario: The collapsed rail is at the right window edge

- **GIVEN** the window is wide enough for both panels
- **WHEN** the user collapses the Queue panel to its rail
- **THEN** the rail ends at the right window edge, to the right of the Library panel
- **AND** the rail's expand control points toward the Library panel

#### Scenario: The GUI Client starts with configured Services

- **GIVEN** an Emby Service with a Music library is configured
- **WHEN** the GUI Client attaches to the Owner
- **THEN** the Library loads that Service's libraries by the same Service startup steps as the TUI Client, with no step copied into the GUI Client

#### Scenario: The user presses in the panel without focus

- **GIVEN** the user last pressed a Queue row
- **WHEN** the user presses a Library row
- **THEN** the press acts on the Library row at once

#### Scenario: The queue is empty

- **WHEN** the queue is empty
- **THEN** the Queue panel shows its empty-queue state

### Requirement: The window arrangement follows the Library column width

The GUI Client SHALL arrange the panels from the measured Library column width (the window width minus the Queue panel's current width), with thresholds from the canvas sizes: Mini below 480 px, Narrow from 480 px, and Wide from 880 px with at least 600 px of height. The Queue panel SHALL keep its own four states inside each arrangement. In Mini only the Queue panel SHALL show, and no key or press SHALL change that. In Wide the list SHALL be at the left edge of the Library panel and the Hero pane SHALL be to its right, between the list and the Queue panel.

#### Scenario: The window is Mini

- **WHEN** the Library column width is below 480 px
- **THEN** only the Queue panel shows

#### Scenario: A key in Mini does not show the Library

- **GIVEN** the window is Mini
- **WHEN** the user presses any key or presses anywhere in the window
- **THEN** only the Queue panel shows

#### Scenario: The window is Narrow

- **WHEN** the Library column width is Narrow
- **THEN** both panels show, the Library panel shows its list without a Hero pane, and picking an item opens the Hero as an overlay over the list

#### Scenario: The window is Wide

- **WHEN** the Library column width is Wide and the height guard holds
- **THEN** both panels show and the Hero pane sits beside the list

#### Scenario: The Wide list is at the left window edge

- **GIVEN** the Library column width is Wide and the height guard holds
- **WHEN** the window paints
- **THEN** the list starts at the left window edge
- **AND** the Hero pane is to the right of the list and to the left of the Queue panel

#### Scenario: The window is resized across a threshold

- **GIVEN** a collection pill is selected and the Hero shows an item
- **WHEN** the Library column width crosses a threshold
- **THEN** the arrangement changes and the selected pill and the Hero's item do not change

### Requirement: The window content takes the destination and Search keys

While the Library panel shows, the window content SHALL take the digits `1` to `7`, which open the first to seventh destination tab after Home, wherever the pointer is. Once Search is presented, it SHALL also take `Ctrl+K`, which focuses the Search field. The window content SHALL NOT take Tab or Shift+Tab: they SHALL go to the Client core's keyboard router. While a core overlay is open or prefix mode is armed, every key SHALL go to the Client core.

#### Scenario: Tab reaches the keyboard router

- **WHEN** the user presses Tab while the Library panel shows
- **THEN** the Client core's keyboard router resolves the chord
- **AND** no pane of the window takes a focus

#### Scenario: A digit opens a destination

- **WHEN** the user presses `2` while the Library panel shows
- **THEN** the second destination tab after Home becomes active

### Requirement: A router destination step opens a presented destination

When the Client core's keyboard router resolves a destination step in the GUI Client (the next or previous destination, or a destination by number), the Client core SHALL open that destination as a press on its tab opens it. It SHALL NOT open a destination that the GUI Client does not present.

#### Scenario: Tab opens the next destination

- **WHEN** the user presses Tab while the Library panel shows
- **THEN** the next presented destination opens and loads its content, as a press on its tab does

#### Scenario: Tab skips a destination that the GUI does not present

- **GIVEN** the next TUI tab is a destination that the GUI Client does not present
- **WHEN** the user presses Tab
- **THEN** the next presented destination opens

### Requirement: The Library keys act on the hovered row

While the pointer is over the Library column, the window content SHALL take the Library keys. `/` opens the list filter. Enter plays the hovered row, `n` plays it next, and `a` adds it to the queue; with no hovered row they do nothing. While the pointer is over the destination's list, the arrow, Page Up, Page Down, Home, and End keys scroll that list. Every key that the window content does not take SHALL go to the Client core's keyboard router.

#### Scenario: Enter plays the hovered row

- **WHEN** the pointer rests on a track row of the Music list and the user presses Enter
- **THEN** the track plays as a double-click on the row plays it

#### Scenario: The Library keys reach the Library under the pointer

- **GIVEN** the pointer rests over the Library list
- **WHEN** the user presses `/`, types `bo`, presses Esc, and then presses Space
- **THEN** `/` opens the Library filter, not the Queue filter, the list shows only the rows whose names contain `bo`, Esc closes the filter, and Space plays or pauses

#### Scenario: The filter picks nothing

- **GIVEN** the Hero shows an album
- **WHEN** the Library filter narrows the list so that the album's row is hidden
- **THEN** the Hero still shows the album

#### Scenario: The Queue keeps its keys under the pointer

- **GIVEN** the pointer rests over the Queue list box
- **WHEN** the user presses `/`, then Esc, then `a`
- **THEN** `/` opens the Queue filter and `a` opens the audio popover

#### Scenario: An arrow key scrolls the list

- **GIVEN** the pointer rests over the Music list
- **WHEN** the user presses Down
- **THEN** the list scrolls by one row
- **AND** no row is picked and the Hero does not change

#### Scenario: Enter with no hovered row

- **GIVEN** the pointer rests over the Library column but on no row
- **WHEN** the user presses Enter, `n`, or `a`
- **THEN** nothing plays and the queue does not change

#### Scenario: Enter plays an Emby library's folder

- **WHEN** the pointer rests on a folder or boxset row of an Emby library and the user presses Enter
- **THEN** the folder plays through the TUI's folder-play rule, the queue is replaced by its items, and no nested list opens

#### Scenario: Prefix mode passes the Library

- **GIVEN** the pointer rests over the Library column and prefix mode is armed
- **WHEN** the user presses a key
- **THEN** the key resolves in the prefix namespace, and the Library does not take it

### Requirement: Library rows have no cursor and no selection

No Library list, Hero tracklist, or Workspace table SHALL have a keyboard cursor or a selected row. A single press on a Hero tracklist row or a Workspace table row SHALL do nothing. A double-click on such a row, its hover play button, or Enter while the pointer rests on it SHALL play it. The item that the Hero shows SHALL stay marked in the list.

#### Scenario: A press on a Hero row plays nothing

- **WHEN** the user presses once on a row of the Hero tracklist or a Workspace table
- **THEN** nothing plays and no row becomes selected

#### Scenario: Enter plays the hovered Hero row

- **WHEN** the pointer rests on a Workspace table row and the user presses Enter
- **THEN** that row plays

#### Scenario: The Hero's item stays marked

- **GIVEN** the Hero shows an album that the user picked
- **WHEN** the pointer moves away from the list
- **THEN** the album's row stays marked as the Hero's item

### Requirement: A collection dropdown selects the collections

Music, Podcasts, Feeds, and an Emby library in feed-group mode SHALL each show one collection dropdown in the list header. The dropdown SHALL offer All and one entry per collection: a Music group, a Podcast show, a Feeds subscription, or an Emby folder. The list SHALL show the de-duplicated union of the checked collections, or every collection while All is selected. No other destination SHALL show a collection dropdown.

#### Scenario: Collections per destination

- **WHEN** a destination with collections paints its list
- **THEN** the dropdown offers All and the Music groups, the Podcast shows, the Feeds subscriptions, or the folders of an Emby library in feed-group mode

#### Scenario: A destination without collections

- **WHEN** Shows, Audiobooks, or a plain Emby library paints its list
- **THEN** no collection dropdown shows
- **AND** Shows paints its own single-choice view dropdown, which is not a collection filter

#### Scenario: Two collections are checked

- **GIVEN** Hip-Hop and Soul are checked in Music, and one artist has albums in both
- **WHEN** the list paints
- **THEN** the list shows the artists of Hip-Hop and Soul sorted as one list, and that artist shows one time

#### Scenario: Space over the trigger plays or pauses

- **GIVEN** the dropdown is closed and the pointer rests on its trigger
- **WHEN** the user presses Space
- **THEN** playback plays or pauses and the selection does not change

### Requirement: The collection dropdown sits in the list header and no pill row remains

The GUI Library panel SHALL show no pill row and no selector row between the top bar and the list in any destination. The dropdown trigger SHALL sit in the list header on the title's line, at the right end of the line, and SHALL show that it opens a menu. The open dropdown SHALL open under its trigger, over the list.

#### Scenario: Podcasts header

- **WHEN** the Podcasts destination paints
- **THEN** the list header's title line shows the list title at the left and the dropdown trigger at the right end, and nothing else
- **AND** no row of pills or selector controls shows between the top bar and the list

#### Scenario: No destination paints a pill row

- **WHEN** any destination paints, Home, Shows, and Audiobooks included
- **THEN** no pill row shows above the list

#### Scenario: The dropdown opens under the trigger

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens directly under the trigger, over the list rows

### Requirement: The selection is All or checked collections

The selection SHALL be All or a non-empty set of checked collections, never both. A press on an unchecked collection SHALL check it and clear All. A press on a checked collection SHALL uncheck it; when no collection stays checked, the selection SHALL become All. A press on All SHALL uncheck every collection. A destination SHALL start on All when it holds no saved selection.

#### Scenario: The first collection is checked

- **GIVEN** All is selected
- **WHEN** the user presses Tifo Football
- **THEN** only Tifo Football is checked and All is not

#### Scenario: A second collection is checked

- **GIVEN** Tifo Football is checked
- **WHEN** the user presses Off Menu
- **THEN** Tifo Football and Off Menu are checked

#### Scenario: The last collection is unchecked

- **GIVEN** only Tifo Football is checked
- **WHEN** the user presses Tifo Football
- **THEN** All is selected and no collection is checked

#### Scenario: All is pressed

- **GIVEN** Tifo Football and Off Menu are checked
- **WHEN** the user presses All
- **THEN** All is selected and no collection is checked

#### Scenario: A failed collection is fetched again on a press

- **GIVEN** a Music group whose albums failed to load
- **WHEN** the user presses All
- **THEN** the group's albums are requested again and the list shows every group's albums once they load
- **AND** a repaint alone does not request them again

#### Scenario: A checked collection no longer exists

- **GIVEN** Soul and Jazz are checked, and the library no longer has Soul
- **WHEN** the list paints
- **THEN** only Jazz is checked
- **AND** when the library has none of the checked collections, All is selected

### Requirement: The dropdown trigger names the selection

The closed dropdown SHALL show a trigger that names the selection: All, the one checked collection, or the count of checked collections. A press on the trigger SHALL open the dropdown, or close it while it is open.

#### Scenario: Two collections are checked

- **GIVEN** Tifo Football and Off Menu are checked
- **WHEN** the dropdown is closed
- **THEN** the trigger shows the count of checked collections

#### Scenario: The trigger opens the dropdown

- **WHEN** the user presses the trigger
- **THEN** the dropdown opens with All and every collection, each with its check state

### Requirement: The open dropdown filters its collections and closes

The open dropdown SHALL show a filter field. Text in the field SHALL narrow the collection entries to the ones whose name matches, by the Library filter's match rule; All SHALL stay shown. A check change SHALL leave the dropdown open. A press outside the dropdown, a destination change, or Esc SHALL close it and clear the filter text. The dropdown SHALL bind no other key.

#### Scenario: The filter narrows the entries

- **GIVEN** the dropdown is open
- **WHEN** the user types `tifo`
- **THEN** the dropdown shows All and Tifo Football, and no other collection

#### Scenario: A check leaves the dropdown open

- **GIVEN** the dropdown is open
- **WHEN** the user presses a collection
- **THEN** its check state changes and the dropdown stays open

#### Scenario: A press outside closes the dropdown

- **GIVEN** the dropdown is open with the filter text `tifo`
- **WHEN** the user presses a Library row outside the dropdown
- **THEN** the dropdown closes and the next open shows an empty filter

### Requirement: Feed-based lists paint newest first

Podcasts, Feeds, and an Emby library in feed-group mode SHALL paint their lists with the most recent row first; undated rows SHALL stay last. The GUI SHALL show no date order control; the core keeps its date-order command.

#### Scenario: A folder paints newest first

- **WHEN** the user checks one folder of an Emby library in feed-group mode
- **THEN** the list shows that folder's items newest first

#### Scenario: No destination shows a date order control

- **WHEN** any destination paints its list header
- **THEN** no date order control shows

### Requirement: The collection selection persists per destination

Each destination SHALL keep its own selection while the GUI Client runs. Every collection dropdown selection and its date order, where one is held, SHALL persist across a restart through the same launch-location entry, keyed by collection target: Music groups, Podcast shows, Feeds subscriptions, or Emby feed-group folders. An entry written before targets existed SHALL load as Music. A destination with no saved selection SHALL start on All.

#### Scenario: The user changes destination and returns

- **GIVEN** Soul and Jazz are checked in Music
- **WHEN** the user opens another destination and then Music again
- **THEN** Soul and Jazz are still checked

#### Scenario: The GUI starts again

- **GIVEN** the GUI Client quit with Soul and Jazz checked in Music
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Soul and Jazz checked

#### Scenario: A saved collection no longer exists

- **GIVEN** the GUI Client quit with only Soul checked, and the library no longer has Soul
- **WHEN** the GUI Client starts again
- **THEN** Music opens on All

#### Scenario: Non-Music selections survive a restart

- **GIVEN** the GUI Client quit with a show checked in Podcasts, a subscription checked in Feeds, and a folder checked in an Emby feed-group library
- **WHEN** the GUI Client starts again
- **THEN** Podcasts, Feeds, and that Emby library open with those collections checked

#### Scenario: Saved ids restore to their own dropdown

- **GIVEN** a launch entry names the Feeds target with a saved subscription
- **WHEN** the GUI Client starts again
- **THEN** Feeds opens with that subscription checked and no other dropdown reads it

#### Scenario: An old launch entry loads as Music

- **GIVEN** the GUI Client quit with Soul and Jazz saved as Music groups before launch entries named a collection target
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Soul and Jazz checked

#### Scenario: A destination with no saved selection starts on All

- **GIVEN** the GUI Client quit with no saved Podcasts or Feeds selection
- **WHEN** the GUI Client starts again
- **THEN** Podcasts and Feeds open on All

#### Scenario: The Shows view survives a restart

- **GIVEN** the GUI Client quit with Upcoming selected in a Shows destination
- **WHEN** the GUI Client starts again
- **THEN** that Shows destination opens with Upcoming selected

#### Scenario: An old launch file has no Shows view

- **GIVEN** the GUI Client quit before the launch file held a Shows view
- **WHEN** the GUI Client starts again
- **THEN** Shows opens on All

### Requirement: Each Music group keeps its last picked album

Each Music group SHALL keep the album its Hero last showed while it was the only checked group, across a restart too. When the selection becomes one group, the Hero SHALL show its kept album, else its first album. For All or several groups, the Hero SHALL keep its album while the union holds it, else show the union's first album. The Hero SHALL never be blank while the albums are loaded.

#### Scenario: A group reopens on its own album

- **GIVEN** the user picked Paul's Boutique with only Hip-Hop checked, and then checked only Soul
- **WHEN** the user checks only Hip-Hop again
- **THEN** the Hero shows Paul's Boutique

#### Scenario: A second group keeps the Hero

- **GIVEN** only Hip-Hop is checked and the Hero shows Paul's Boutique
- **WHEN** the user also checks Soul
- **THEN** the Hero still shows Paul's Boutique

#### Scenario: The kept albums survive a restart

- **GIVEN** the GUI Client quit with Paul's Boutique kept under Hip-Hop and only Hip-Hop checked
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Hip-Hop checked, and the Hero shows Paul's Boutique

### Requirement: Shows selects one view

The GUI Shows destination's list header SHALL paint one dropdown with exactly three single-choice entries: All, New, and Upcoming, in that order. All SHALL be the default. A press on one entry SHALL replace the active entry and close the dropdown. The choice SHALL be a view of the destination and SHALL NOT be a collection set, a collection target, a collection filter, or a key binding. The TUI Client's Shows modes SHALL NOT change.

#### Scenario: Shows opens on All

- **WHEN** the user opens a Shows destination with no saved view
- **THEN** the list shows the library's series sorted by name with the letter rail, the Hero and Workspace are present, and the dropdown marks All

#### Scenario: One view replaces another

- **GIVEN** All is active and the view dropdown is open
- **WHEN** the user presses New
- **THEN** New is the only marked entry and the dropdown closes

### Requirement: Shows paints the New and Upcoming lists

Under New, the GUI Shows destination SHALL show the old Shows Latest episodes list, merged across every Emby TV library, newest first and de-duplicated by item id. A New row SHALL select into the Hero, play, and enqueue as a Recently added Emby row does. Under Upcoming, it SHALL show the old Shows Upcoming premieres list, merged across every Emby TV library, soonest first. An empty list SHALL show no rows. GUI Home SHALL NOT show either list. The TUI Client's Home SHALL NOT change.

#### Scenario: Two TV libraries have new episodes

- **GIVEN** two Emby TV libraries each have recent episodes
- **WHEN** the Shows destination paints New
- **THEN** the list shows the episodes of both libraries as one list, newest first

#### Scenario: A New row plays

- **WHEN** the user plays a New row
- **THEN** that episode plays as a Recently added episode row plays

#### Scenario: Upcoming has no premieres

- **WHEN** no Emby TV library has an upcoming premiere
- **THEN** the Shows destination's Upcoming view shows no rows

#### Scenario: Home has no relocated sections

- **WHEN** GUI Home paints
- **THEN** it shows only Continue watching and Recently added sections, with no Latest episodes or Upcoming section

### Requirement: Shows Upcoming rows are display-only

An Upcoming row in the Shows destination SHALL NOT select, play, or enqueue. A press on it SHALL change nothing, a play or enqueue key over it SHALL do nothing, and the Hero SHALL keep its item.

#### Scenario: An Upcoming row is pressed

- **GIVEN** the Shows Hero shows a series
- **WHEN** the user presses an Upcoming row
- **THEN** the Hero still shows that series and nothing plays

#### Scenario: Enter over an Upcoming row

- **WHEN** the pointer rests on an Upcoming row and the user presses Enter
- **THEN** nothing plays and the queue does not change





### Requirement: The panel widths persist across launches

The Queue panel's column width and the Wide list column's width that the user dragged SHALL persist across launches of the GUI Client, and a double-click SHALL reset a divider to its default and save the default. A saved width that is missing or unreadable SHALL fall back to the default. A drag of the Queue divider toward the left window edge SHALL make the Queue column wider. A drag of the list divider toward the right window edge SHALL make the list column wider. A width saved before the columns changed sides SHALL load as the same column's width.

#### Scenario: A dragged divider survives a restart

- **GIVEN** the user dragged the list column divider to 420 px
- **WHEN** the GUI Client starts again
- **THEN** the list column opens at 420 px

#### Scenario: A drag to the left widens the Queue column

- **GIVEN** both panels show, the Queue panel is not collapsed, and a Queue column 50 px wider is inside the allowed width range
- **WHEN** the user drags the Queue divider 50 px to the left
- **THEN** the Queue column is 50 px wider and the Library column is 50 px narrower

#### Scenario: A drag to the right widens the list

- **GIVEN** the Library column width is Wide, the height guard holds, and a list 50 px wider is inside the allowed width range
- **WHEN** the user drags the list divider 50 px to the right
- **THEN** the list column is 50 px wider and the Hero pane is 50 px narrower

#### Scenario: A saved Queue width keeps its column

- **GIVEN** an earlier GUI Client saved a Queue column width of 460 px
- **WHEN** the GUI Client starts
- **THEN** the Queue column at the right window edge opens at 460 px

### Requirement: The GUI Client keeps its own launch location

At orderly exit the GUI Client SHALL save its launch location: the active destination and the selected collection of each Grouped Music destination. It SHALL save no Panel focus. At start it SHALL restore that location; a destination with no saved selection SHALL open on its first group. A launch location saved before this change SHALL still load, and its saved Panel focus SHALL change nothing. The GUI Client SHALL NOT read or write the TUI launch state, and SHALL NOT save its launch location while the window is open. While the GUI Client presents only some destinations, a saved destination that it does not present SHALL open no other destination.

#### Scenario: The GUI and the TUI exit in turn

- **GIVEN** the TUI exits on Shows, and the GUI Client later exits on Music
- **WHEN** the TUI and the GUI Client each start again
- **THEN** the TUI starts on Shows and the GUI Client starts on Music

#### Scenario: An older launch location loads

- **GIVEN** a launch location that an earlier GUI Client saved on Music with Queue focus
- **WHEN** the GUI Client starts
- **THEN** it restores Music, and no panel shows as focused

### Requirement: The Music list is the canvas tree

The Music list SHALL show the artists of the selected collection as a tree of artist rows, album rows, and track rows, with a letter heading above the first artist of each letter. A press on an artist or album row SHALL open or close it and pick it. A press on a track row SHALL pick its album. A double-click on a track row SHALL play that track as the Enter play does.

#### Scenario: A track is double-clicked

- **WHEN** the user double-clicks a track row of the list
- **THEN** the queue is replaced and the track plays
- **AND** with autoload on the queue holds that track's album in disc and track order starting at the track, and with autoload off the track alone

#### Scenario: An album is opened

- **WHEN** the user presses an album row whose tracks are not loaded
- **THEN** the album opens, its tracks appear when they load, and the Hero shows the album

#### Scenario: A track row is pressed

- **WHEN** the user presses a track row
- **THEN** the Hero shows the track's album
- **AND** no row becomes selected

#### Scenario: The rows show their facts

- **WHEN** the list paints an album row and a track row
- **THEN** the album row shows the album's year and the track row shows the track number

#### Scenario: The list is loading or empty

- **WHEN** the selected collection is loading or holds no artists
- **THEN** the list paints a loading state or an empty text

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show a letter rail from A to Z. A letter with no rows SHALL be disabled. A press on a letter SHALL scroll the list to the first row whose sort name starts with that letter; it SHALL pick and select nothing. The rail's keyboard path SHALL be the list's `/` filter. A list sorted by date SHALL show no rail. An Emby library that is not a feed-group library SHALL show the rail and no collection dropdown: the rail replaces the TUI's letter-range pill row, as it does for Shows.

#### Scenario: A plain Emby library paints the rail and no collection dropdown

- **WHEN** a Movies library paints its list
- **THEN** the list is flat and sorted by name, it shows the letter rail, and no collection dropdown shows

#### Scenario: A letter is pressed

- **WHEN** the user presses the letter B on the rail of the Music artist list
- **THEN** the list scrolls so that the first artist whose sort name starts with B is fully visible
- **AND** the Hero does not change

#### Scenario: A letter without rows is pressed

- **WHEN** the user presses a disabled letter
- **THEN** the list does not change

### Requirement: The Hero shows the picked item with its actions

The Hero SHALL show the picked album with its artist header, its square artwork, and its tracklist. It SHALL offer Play, Play next with the key hint `n`, Add to queue with the key hint `a`, Shuffle artist, and More actions. Play SHALL replace the queue and start the item without the TUI's populated-queue replace-confirm. The saved-playlist save/discard prompt SHALL still present, through the GUI's confirmation overlay. Play next SHALL admit the item directly after the active slot, and SHALL append it when there is no active slot or the active slot is the last. No slot already in the queue SHALL move. More actions SHALL open the item's menu. The tracklist row's hover play button SHALL replace the row's track number on hover and play the row's track, and a double-click on a tracklist row SHALL play that track the same way, as it does in every Workspace list (Shows episodes, Audiobooks chapters, Feeds entries). The tracklist row of the track playing at the playhead SHALL show a play glyph beside its title. A highlight marks focus only, so the playing row SHALL NOT paint a green fill.

#### Scenario: The Hero paints a picked album

- **WHEN** the user picks the album Licensed to Ill
- **THEN** the artist header shows the artist, the selected collections, the album count, and the track count and total time
- **AND** the album block shows the year, the title, the track count, the time, and the square artwork, above the tracklist

#### Scenario: Play next from the Hero

- **WHEN** the user presses the Hero's Play next control
- **THEN** the album is admitted directly after the active slot
- **AND** no slot already in the queue moves

#### Scenario: A tracklist row's hover controls

- **WHEN** the pointer is over a tracklist row and the user presses its Play next or Add to queue control
- **THEN** that track is admitted after the active slot, or at the end of the queue

#### Scenario: A tracklist row is played

- **GIVEN** the pointer is over a tracklist row
- **WHEN** the user presses the row's play button, or double-clicks the row
- **THEN** the track plays: the queue is replaced, as a double-clicked list track is

#### Scenario: More actions opens the item menu

- **WHEN** the user presses the Hero's More actions control
- **THEN** the item's menu opens in the row-menu look, and each entry runs its action for that item

### Requirement: The top bar shows the destination tabs and Search

The window SHALL show a top bar with a Home icon and the destination tabs as its global header. Once Search is presented, the Library Status bar SHALL show a Search field with the hint `Ctrl K` at its left side. A press on a tab SHALL make its destination active. The Search field SHALL search all libraries, show the results above the field, and activate a result by Enter or a press.

#### Scenario: A tab is pressed

- **WHEN** the user presses the Music tab
- **THEN** Music becomes the active destination and its tab is marked current

#### Scenario: A tab is pressed in Mini while the Queue shows

- **GIVEN** the arrangement is Mini and the Queue shows
- **WHEN** the user presses a destination tab
- **THEN** that destination becomes active and the Library shows in place of the Queue

#### Scenario: The active destination's tab is pressed again in Mini while the Library shows

- **GIVEN** the arrangement is Mini and the Library shows for an active destination
- **WHEN** the user presses that destination's tab again, or the Home icon while the Home content is present
- **THEN** the Library's press flips the view back to the Queue

#### Scenario: A different destination's tab is pressed in Mini while the Library shows

- **GIVEN** the arrangement is Mini and the Library shows
- **WHEN** the user presses a destination tab whose destination is not active
- **THEN** that destination becomes active and the Library stays showing

#### Scenario: Search opens from the keyboard

- **GIVEN** the Library panel shows
- **WHEN** the user presses `Ctrl+K`
- **THEN** the Search field in the Library Status bar has focus

#### Scenario: A Search result is activated

- **WHEN** the user activates a Search result
- **THEN** the GUI Client opens the result's destination on that item

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the Search field on the left, and the location on the right: the destination, All or the names of the checked collections, and the picked item. The bar SHALL show no key hints. While prefix mode is armed or a toast is active, that SHALL paint after the Search field, and the field and the location SHALL stay.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop checked and a Beastie Boys album picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the Search field and the location Music, Hip-Hop, Beastie Boys, and no key hints

#### Scenario: Two collections are checked

- **GIVEN** Feeds is active with Tifo Football and Off Menu checked
- **WHEN** the Library Status bar paints
- **THEN** the location names Feeds, then both subscriptions, then the picked entry

#### Scenario: A toast arrives

- **WHEN** the shared Status bar projection holds a toast while the Library panel shows
- **THEN** the toast paints after the Search field until it ends, and the field and the location stay

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

### Requirement: Every Hero is one frame with the largest unit as its title

The Hero of Music, Shows, Podcasts, Audiobooks, and Feeds SHALL paint one frame: a header band, a detail block beside or above the art, and a list band. The header band's title SHALL be the name of the largest unit: the artist, the series, the podcast, the feed, or, for a book, the book. The detail block SHALL show the selected smaller unit (the album, the season or episode, the episode, the entry) with its duration and progress, its Play, Play next, and Add to queue actions, and its overview. Plain facts with no role SHALL sit on one line under the header title. A destination SHALL map its facts onto this frame and SHALL NOT remove a band, add a layout, or paint its own copy of a part. Home and Movies are not yet on the frame.

#### Scenario: A Podcasts episode's Hero

- **WHEN** the Hero shows an episode of a podcast
- **THEN** the header title is the podcast's name
- **AND** the detail block shows the episode's name, its duration, and its actions

#### Scenario: A Feeds entry's Hero

- **WHEN** the Hero shows an entry of a feed
- **THEN** the header title is the feed's name
- **AND** the list band holds that feed's entries, newest first, with the selected entry marked

#### Scenario: A TV series' actions act on the selected season

- **WHEN** a series is selected and the user presses Play, Play next, or Add to queue
- **THEN** the selected season is played, admitted after the active slot, or appended

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, flat list rows, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. A destination SHALL supply its data, its hook prefix, and its handlers, and SHALL NOT restate the part's look. A Workspace table SHALL paint a header row, a number or play-glyph cell, a title, a duration, and a progress run. An Audiobooks chapter row SHALL show its number.

#### Scenario: A list row looks the same in every destination

- **WHEN** an Emby, Audiobooks, Podcasts, Feeds, or Shows Latest row paints
- **THEN** it paints through the shared item row with the same padding, title, subtitle, and trailing text

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

### Requirement: A repaint copies no section content

The window SHALL read a Library list, Library detail, Queue, or Settings section without copying its rows or entries. A repaint that follows a change to some other section SHALL NOT allocate a new value for a section whose revision did not change. A section update SHALL cost one copy of that section's content, however many components read it.

#### Scenario: A repaint after another section changed keeps the Queue value

- **GIVEN** the window holds a Queue section at one revision
- **WHEN** a batch updates only the Status bar section
- **THEN** every reader of the Queue section sees the same Queue value as before the batch

#### Scenario: A Library section update copies the section once

- **GIVEN** the Library panel and its destination list both read the Library list section
- **WHEN** a batch carries a new revision of that section
- **THEN** the window holds one new value of the section
- **AND** the panel and the destination list read that one value

#### Scenario: A repeated revision writes nothing

- **WHEN** a batch carries a section at the revision that the window already holds
- **THEN** the window keeps the value it holds and notifies no reader of that section

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

### Requirement: The Client core holds the player panel's interaction state

The interaction state of the player panel and the Status bar (the Queue's active row, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) SHALL live in the Client core, not in the window content. For these surfaces the window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it, each key with its pointer target. The GUI Client SHALL have no Panel focus.

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router, against the key's pointer target

### Requirement: Layout and the predicted playhead are not interaction state

The layout that the container size picks SHALL NOT count as interaction state. The position that the window advances between two playhead updates SHALL be a prediction of the Client core's playhead, not interaction state. The Library panel's interaction state is the exception that the requirement "The window content holds the Library panel's interaction state" defines.

#### Scenario: A resize picks a layout

- **WHEN** the window content picks a panel state from its container size
- **THEN** the Client core's collapse and queue-fold toggles do not change

### Requirement: The window content holds the Library panel's interaction state

The Library panel's interaction state (hover, scroll, open tree nodes, filter and Search text, the open Hero overlay and item menu) SHALL live in the window content. The Library panel SHALL hold no cursor and no focus. The Client core SHALL hold the data selection: the active destination, each destination's selected collection, and the Hero's item. The window content SHALL change the data selection only through the Client core.

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

### Requirement: The window paints the stylesheet

The window's author stylesheet SHALL apply to the document from the first frame. The author stylesheet is the generated Tailwind sheet (`app.generated.css`), the cell-unit rule, and the theme rule. A class in the window markup SHALL paint the declarations of its rule.

#### Scenario: The window opens

- **WHEN** the GUI Client window opens
- **THEN** a Tailwind class in the markup resolves to the computed value of its rule

### Requirement: The window content follows a resize

When the surface size of the GUI Client window changes, the next presented frame SHALL lay out the window content at the new size. No later event SHALL be needed for the layout to change.

#### Scenario: The user drags a window edge

- **WHEN** the user drags a window edge
- **THEN** each presented frame lays out the content at that frame's surface size

### Requirement: Pointer input never stops the GUI Client

A click, double-click, drag, or wheel step on any point of the window SHALL NOT end the GUI Client process. This holds when the input arrives after an earlier input changed the document.

#### Scenario: The user double-clicks a queue row

- **GIVEN** an earlier press rebuilt the queue rows
- **WHEN** the user double-clicks a queue row
- **THEN** the row plays
- **AND** the window stays open and accepts the next input

### Requirement: The window looks as the Svelte front end looked

Each surface of the window SHALL paint the markup structure, class strings, and computed colours of its Svelte source at commit `38248e262^` (`crates/mbv-gui/ui/src/`). A difference from that source SHALL exist only where a Blitz gap named in `design.md` (D20) forces it.

#### Scenario: The Queue panel paints at the queue column width

- **WHEN** the Queue panel paints at the queue column width
- **THEN** the panel fills that column
- **AND** no band of a colour that the Svelte panel did not paint appears
