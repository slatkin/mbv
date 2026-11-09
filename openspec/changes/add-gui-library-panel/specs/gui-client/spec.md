## MODIFIED Requirements

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

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection while the Library panel does not show; while the Library panel shows, the Library Status bar shows them. A surface, or a Library destination, that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder: the top bar carries only the destination tabs that the GUI Client presents, and it carries the Search field only once Search is presented.

#### Scenario: The queue has slots

- **GIVEN** the Owner process holds a Bound queue with slots
- **WHEN** the GUI Client window is open
- **THEN** the expanded panel shows, from top to bottom, a header, the artwork, the title, the scrubber, the transport, the track chips, the queue header, the queue rows, and a footer line
- **AND** the header shows the playback target with a chooser, the playback state with a leading dot, and a collapse control
- **AND** the artwork shows the active item with its season and episode badge
- **AND** the transport has previous, back 10 s, play/pause, forward 30 s, and next
- **AND** the queue lists the slots in canonical order, marks the active slot, and numbers every row with its 1-based position in the queue

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
- **THEN** the artwork paints in the canvas's fixed still slot with a caption that names the image source, and the season and episode badge sits at the bottom left as the canvas places it

#### Scenario: The footer paints the queue's source text

- **WHEN** the expanded panel paints the footer line
- **THEN** the left line shows the queue's existing source text — the playlist text the frame carries — without a leading icon, the owner marker paints the label then the dot, and the line uses the canvas footer's type and border
- **AND** the prefix, toast, autosave, and dirty states still paint in the footer line when they are active and the Library panel does not show

### Requirement: GUI keyboard interaction matches the TUI Client

For every action that the player panel and the Status bar present, each key chord SHALL have the same effect as in the TUI Client, and these surfaces SHALL use the same `[keys]` configuration and the same keyboard router. The key hints that the canvas shows for them are the default bindings. The canvas chords `z` (collapse), `q` (queue fold), and `c` (cycle subtitle) bind in a GUI-only key context; the TUI Client's default bindings SHALL NOT change. The Library panel's keys are GUI keys that the requirement "The Library panel handles its own keys" defines; a key that the Library panel does not take SHALL reach the keyboard router.

#### Scenario: A configured binding is used in both Clients

- **GIVEN** the user rebinds a queue action in `[keys]`
- **WHEN** the user presses the new chord in the GUI Client
- **THEN** the GUI Client runs that queue action, as the TUI Client does

#### Scenario: Prefix mode

- **WHEN** the user arms prefix mode in the GUI Client
- **THEN** the footer line shows the armed state and the next chord resolves in the prefix namespace, as in the TUI Client

### Requirement: GUI pointer input resolves through semantic targets

A pointer press, drag, or wheel step on the player panel SHALL resolve to the semantic target that the GUI painted at that point in the current frame. The effect SHALL be the gesture that the design canvas defines. Where the canvas is silent and the TUI has a matching gesture, the effect SHALL be the same. Pointer input on the Library panel is the window content's own (the requirement "The Library panel's interaction state lives in the window content").

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

### Requirement: The window content holds no interaction state

The interaction state of the player panel and the Status bar (focus, cursor, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) SHALL live in the Client core, not in the window content. For these surfaces the window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it. Panel focus between the Library panel and the Queue panel SHALL live in the Client core. The layout that the container size picks is not interaction state. The Library panel's interaction state is the exception that the requirement "The Library panel's interaction state lives in the window content" defines.

#### Scenario: The window content reloads

- **GIVEN** the queue cursor is on a slot and the queue has focus
- **WHEN** the window content reloads
- **THEN** the queue has focus and the cursor is on the same slot

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router

## ADDED Requirements

### Requirement: The GUI presents the Library panel beside the Queue panel

The GUI Client SHALL present the Library panel and the Queue panel in one window. The Library panel's content, arrangement, and states SHALL follow the Library artboard of the design canvas, not the TUI Client. Panel focus SHALL move to a panel by a press in it and by Tab or Shift+Tab across the panel boundary.

#### Scenario: Both panels show

- **GIVEN** the window is wide enough for both panels
- **WHEN** the GUI Client window is open
- **THEN** the Library panel and the Queue panel both show
- **AND** exactly one of them has Panel focus

#### Scenario: The GUI Client starts with configured Services

- **GIVEN** an Emby Service with a Music library is configured
- **WHEN** the GUI Client attaches to the Owner
- **THEN** the Library loads that Service's libraries by the same Service startup steps as the TUI Client, with no step copied into the GUI Client

#### Scenario: The user presses in the panel without focus

- **GIVEN** the Queue panel has Panel focus
- **WHEN** the user presses a Library row
- **THEN** the Library panel takes Panel focus and the press acts on the row

#### Scenario: The queue is empty

- **GIVEN** the queue is empty
- **WHEN** the user moves Panel focus to the Queue panel
- **THEN** the Queue panel has Panel focus and shows its empty-queue state

### Requirement: The window arrangement follows the Library column width

The GUI Client SHALL arrange the panels from the measured Library column width (the window width minus the Queue panel's current width), with thresholds from the canvas sizes: Mini below 480 px, Narrow from 480 px, and Wide from 880 px with at least 600 px of height. The Queue panel SHALL keep its own four states inside each arrangement.

#### Scenario: The window is Mini

- **WHEN** the Library column width is below 480 px
- **THEN** only the panel with Panel focus shows

#### Scenario: The window is Narrow

- **WHEN** the Library column width is Narrow
- **THEN** both panels show, the Library panel shows its list without a Hero pane, and picking an item opens the Hero as an overlay over the list

#### Scenario: The window is Wide

- **WHEN** the Library column width is Wide and the height guard holds
- **THEN** both panels show and the Hero pane sits beside the list

#### Scenario: The window is resized across a threshold

- **GIVEN** the Library cursor is on an item
- **WHEN** the Library column width crosses a threshold
- **THEN** the arrangement changes and the cursor, the selected pill, and the Hero's item do not change

### Requirement: The Library panel's interaction state lives in the window content

The Library panel's interaction state (cursor, hover, scroll, open tree nodes, focus inside the panel, filter and Search text, the open Hero overlay and item menu) SHALL live in the window content. The Client core SHALL hold the data selection: the active destination, each destination's selected collection, and the Hero's item. The window content SHALL change the data selection only through the Client core.

#### Scenario: The window content reloads on a Library item

- **GIVEN** Music is active with Hip-Hop selected and an album in the Hero
- **WHEN** the window content reloads
- **THEN** Music is active with Hip-Hop selected and the same album in the Hero
- **AND** the list's cursor, scroll, and open nodes start from their defaults

#### Scenario: The TUI Client browses a library

- **WHEN** the user browses any destination in the TUI Client
- **THEN** every key, pointer gesture, and painted frame behaves as before this change

### Requirement: The window content takes the window keys

While the Library panel shows, the window content SHALL take Tab and Shift+Tab, which move focus to the next or previous pane (the pill row, the list, the Hero, the Queue panel), and the digits `1` to `7`, which open the first to seventh destination tab after Home. Once Search is presented, it SHALL also take `Ctrl+K`, which focuses the Search field. While a core overlay is open or prefix mode is armed, every key SHALL go to the Client core.

#### Scenario: Tab moves to the Queue panel

- **GIVEN** the Library panel has Panel focus and its last pane has focus
- **WHEN** the user presses Tab
- **THEN** the Queue panel has Panel focus

#### Scenario: A digit opens a destination

- **WHEN** the user presses `2` while the Library panel shows
- **THEN** the second destination tab after Home becomes active

### Requirement: The Library panel handles its own keys

While the Library panel has Panel focus, the window content SHALL take the Library keys: `/` opens the list filter, Enter plays the item under the cursor or presses the focused control, `n` plays it next, `a` adds it to the queue, Space and Alt+Space select the focused collection pill, the arrow, Page Up, Page Down, Home, and End keys move the cursor, and Esc closes the open filter, overlay, or menu. Every key that the window content does not take SHALL go to the Client core's keyboard router.

#### Scenario: Right and Left in the list and on the pill row

- **WHEN** the user presses Right on a closed artist row, then Left
- **THEN** the artist opens and then closes
- **AND** on the pill row Right and Left move the focused pill without a change to the selection

#### Scenario: The Library keys reach the Library under Library focus

- **GIVEN** the Library panel has Panel focus and no pill has focus
- **WHEN** the user presses `/`, types `bo`, presses Esc, and then presses Space
- **THEN** `/` opens the Library filter, not the Queue filter, the list shows only the rows whose names contain `bo`, Esc closes the filter, and Space plays or pauses

#### Scenario: The Queue keeps its keys under Queue focus

- **GIVEN** the Queue panel has Panel focus
- **WHEN** the user presses `/`, then Esc, then `a`
- **THEN** `/` opens the Queue filter and `a` opens the audio popover

#### Scenario: Prefix mode passes the Library

- **GIVEN** the Library panel has Panel focus and prefix mode is armed
- **WHEN** the user presses a key
- **THEN** the key resolves in the prefix namespace, and the Library does not take it

### Requirement: Collection pills are single-select

One collection pill SHALL be selected at a time, as the TUI's Grouped Music pill is: one group is always selected. A press on a collection pill SHALL select it and deselect the others, and an Alt-press SHALL select it the same way. Space SHALL select the focused collection pill. A press SHALL NOT deselect the selected pill, so no gesture leaves the row empty. The list SHALL show the selected group's albums, sorted as one list. Latest, filter, and letter-bucket pills SHALL stay single-select.

#### Scenario: Collection pills per destination

- **WHEN** a destination paints its pill row
- **THEN** the collection pills are the Music group pills, the Podcast show pills, the Feeds subscription pills, and the group pills of an Emby library in feed-group mode
- **AND** every other pill is single-select

#### Scenario: A second collection is selected

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user presses the Soul pill
- **THEN** only the Soul pill is selected and the list shows the artists of Soul
- **AND** the list header reads Soul · sort name

#### Scenario: The selected pill is pressed again

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user presses the Hip-Hop pill
- **THEN** Hip-Hop stays selected and the list does not change

#### Scenario: An Alt-press selects the pill

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user Alt-presses the Soul pill
- **THEN** only the Soul pill is selected

#### Scenario: The keyboard selects a pill

- **GIVEN** the pill row has focus and the Soul pill is the focused pill
- **WHEN** the user presses Space
- **THEN** Soul is selected as a press selects it, and playback does not change

#### Scenario: A non-collection pill is selected

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user selects the Latest pill
- **THEN** only the Latest pill is selected

### Requirement: The pill selection persists per destination

Each destination SHALL keep its own selected pill while the GUI Client runs and across a restart of the GUI Client.

#### Scenario: The user changes destination and returns

- **GIVEN** Soul is selected in Music
- **WHEN** the user opens another destination and then Music again
- **THEN** Soul is still selected

#### Scenario: The GUI starts again

- **GIVEN** the GUI Client quit with Soul selected in Music
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Soul selected

#### Scenario: A saved collection no longer exists

- **GIVEN** the GUI Client quit with Soul selected, and the library no longer has Soul
- **WHEN** the GUI Client starts again
- **THEN** Music opens with the first group selected, as the TUI does

### Requirement: Each collection pill keeps its last picked album

Each collection pill SHALL keep the album its Hero last showed, while the GUI Client runs and across a restart of the GUI Client. A pill with no kept album, or whose kept album the selected collection no longer holds, SHALL show the first album in its list. The Hero SHALL therefore never be blank while the selected collection's albums are loaded.

#### Scenario: A pill reopens on its own album

- **GIVEN** the user picked Paul's Boutique under Hip-Hop, and then selected Soul, whose first album is 3 Feet High and Rising
- **WHEN** the user selects Hip-Hop again
- **THEN** the Hero shows Paul's Boutique
- **AND** selecting Soul again shows 3 Feet High and Rising

#### Scenario: A pill with no kept album

- **GIVEN** Soul has never had an album picked
- **WHEN** the user selects Soul
- **THEN** the Hero shows the first album in the Soul list

#### Scenario: The kept albums survive a restart

- **GIVEN** the GUI Client quit with Paul's Boutique kept under Hip-Hop
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Hip-Hop selected, and the Hero shows Paul's Boutique

#### Scenario: A kept album no longer exists

- **GIVEN** the kept album is no longer in the pill's list
- **WHEN** the pill is selected
- **THEN** the Hero shows the first album in the list

### Requirement: The panel widths persist across launches

The Queue panel's column width and the Wide list column's width that the user dragged SHALL persist across launches of the GUI Client, and a double-click SHALL reset a divider to its default and save the default. A saved width that is missing or unreadable SHALL fall back to the default.

#### Scenario: A dragged divider survives a restart

- **GIVEN** the user dragged the list column divider to 420 px
- **WHEN** the GUI Client starts again
- **THEN** the list column opens at 420 px

### Requirement: The GUI Client keeps its own launch location

At orderly exit the GUI Client SHALL save its launch location: the active destination, the selected collection of each destination, and Panel focus. At start it SHALL restore that location; a destination with no saved selection SHALL open on its first group. The GUI Client SHALL NOT read or write the TUI launch state, and SHALL NOT save its launch location while the window is open. While the GUI Client presents only some destinations, a saved destination that it does not present SHALL fall back to the first presented destination.

#### Scenario: The GUI and the TUI exit in turn

- **GIVEN** the TUI exits on Shows, and the GUI Client later exits on Music with Library focus
- **WHEN** the TUI and the GUI Client each start again
- **THEN** the TUI starts on Shows and the GUI Client starts on Music with Library focus

### Requirement: The Library list is the canvas tree

The Music list SHALL show the artists of the selected collection as a tree: artist rows, album rows with the year, and track rows with the track number, with a letter heading above the first artist of each letter. A press on an artist or album row SHALL open or close it and pick it; a press on a track row SHALL move the cursor onto the row and pick its album. A double-click on a track row of the list SHALL play that track, as the Enter play does: the queue SHALL be replaced — with autoload on, by the track's album in disc/track order starting at that track (the library playback policy's autoload rule); with autoload off, by the track alone. The list SHALL keep the cursor row fully visible after every keyboard movement.

#### Scenario: A track is double-clicked

- **WHEN** the user double-clicks a track row of the list
- **THEN** the queue is replaced and the track plays
- **AND** with autoload on the queue holds that track's album starting at the track, and with autoload off the track alone

#### Scenario: An album is opened

- **WHEN** the user presses an album row whose tracks are not loaded
- **THEN** the album opens, its tracks appear when they load, and the Hero shows the album

#### Scenario: The cursor moves past the last visible row

- **WHEN** the user moves the Library cursor below the last fully visible row
- **THEN** the list scrolls so that the cursor row is fully visible

#### Scenario: The list is loading or empty

- **WHEN** the selected collection is loading or holds no artists
- **THEN** the list paints a loading state or an empty text

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show a letter rail from A to Z. A letter with no rows SHALL be disabled. A press on a letter SHALL scroll to the first row whose sort name starts with that letter and move the cursor there. The rail's keyboard path SHALL be the list's `/` filter. A list sorted by date SHALL show no rail.

#### Scenario: A letter is pressed

- **WHEN** the user presses the letter B on the rail of the Music artist list
- **THEN** the cursor moves to the first artist whose sort name starts with B and the row is fully visible

#### Scenario: A letter without rows is pressed

- **WHEN** the user presses a disabled letter
- **THEN** the cursor and the list do not change

### Requirement: The Hero shows the picked item with its actions

The Hero SHALL show the picked album with its artist header, its square artwork, and its tracklist. It SHALL offer Play, Play next with the key hint `n`, Add to queue with the key hint `a`, Shuffle artist, and More actions. Play SHALL replace the queue and start the item without the TUI's populated-queue replace-confirm. The saved-playlist save/discard prompt SHALL still present, through the GUI's confirmation overlay. Play next SHALL admit the item directly after the active slot, and SHALL append it when there is no active slot or the active slot is the last. No slot already in the queue SHALL move. More actions SHALL open the item's menu. The tracklist row's hover play button SHALL be the tracklist's only play gesture: on hover it replaces the row's track number, and it plays the row's track as a double-clicked list track plays. A double-click on a tracklist row SHALL NOT play. The tracklist row of the track playing at the playhead SHALL show a play glyph beside its title. A highlight marks focus only, so the playing row SHALL NOT paint a green fill.

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

#### Scenario: A tracklist play button is pressed

- **GIVEN** the pointer is over a tracklist row
- **WHEN** the user presses the row's play button
- **THEN** the track plays: the queue is replaced, as a double-clicked list track is
- **AND** a double-click on the tracklist row itself does not play

#### Scenario: More actions opens the item menu

- **WHEN** the user presses the Hero's More actions control
- **THEN** the item's menu opens in the row-menu look, and each entry runs its action for that item

### Requirement: The top bar shows the destination tabs and Search

The Library panel SHALL show a top bar with a Home icon and the destination tabs, and, once Search is presented, a Search field with the hint `Ctrl K`. A press on a tab SHALL make its destination active. The Search field SHALL search all libraries, show the results under the field, and activate a result by Enter or a press.

#### Scenario: A tab is pressed

- **WHEN** the user presses the Music tab
- **THEN** Music becomes the active destination and its tab is marked current

#### Scenario: Search opens from the keyboard

- **WHEN** the user presses `Ctrl+K`
- **THEN** the Search field has focus

#### Scenario: A Search result is activated

- **WHEN** the user activates a Search result
- **THEN** the GUI Client opens the result's destination on that item

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the canvas key hints (`/ filter`, `⏎ play`, `n next`, `a queue`, `tab pane`, `1–7 library`) on the left and the location on the right: the destination, the selected collection's name, and the picked item, joined by `›`. While prefix mode is armed or a toast is active, that SHALL replace the key hints, and the location SHALL stay.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop selected and a Beastie Boys album picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the key hints and the location Music › Hip-Hop › Beastie Boys

#### Scenario: A toast arrives

- **WHEN** the shared Status bar projection holds a toast while the Library panel shows
- **THEN** the toast replaces the key hints until it ends, and the location stays

### Requirement: Library art comes through the frame

The Library panel SHALL paint artwork only from the mbv image cache through the frame. The window content SHALL fetch no image itself.

#### Scenario: An album is picked

- **WHEN** the user picks an album whose artwork is not yet cached
- **THEN** the Hero paints the artwork placeholder until a later frame carries the cached image
- **AND** the window content makes no network request for the artwork
