## MODIFIED Requirements

### Requirement: The first GUI slice presents the player panel of the design canvas

The GUI Client SHALL present one player panel whose content, arrangement, and states follow the design canvas. The TUI layout SHALL NOT be the reference. The footer line SHALL show the prefix-armed state and the toasts of the shared Status bar projection while the Library panel does not show; while the Library panel shows, the Library Status bar shows them. A surface that the GUI Client does not yet present SHALL be absent, with no stub and no placeholder.

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

#### Scenario: The artwork has a still slot

- **WHEN** the expanded panel paints the artwork of the active item
- **THEN** the artwork paints in the canvas's fixed still slot with a caption that names the image source, and the season and episode badge sits at the bottom left as the canvas places it

#### Scenario: The footer paints the queue's source text

- **WHEN** the expanded panel paints the footer line
- **THEN** the left line shows the queue's existing source text — the playlist text the frame carries — without a leading icon, the owner marker paints the label then the dot, and the line uses the canvas footer's type and border
- **AND** the prefix, toast, autosave, and dirty states still paint in the footer line when they are active and the Library panel does not show

## ADDED Requirements

### Requirement: The GUI presents the Library panel beside the Queue panel

The GUI Client SHALL present the Library panel and the Queue panel in one window, as the TUI Client arranges them. The Library panel's content, arrangement, and states SHALL follow the Library artboard of the design canvas. Panel focus SHALL move between the two panels by the same keys and pointer presses as in the TUI Client.

#### Scenario: Both panels show

- **GIVEN** the window is wide enough for both panels
- **WHEN** the GUI Client window is open
- **THEN** the Library panel and the Queue panel both show
- **AND** exactly one of them has Panel focus

#### Scenario: The user moves Panel focus

- **WHEN** the user presses the TUI Client's Panel-focus key
- **THEN** Panel focus moves to the other panel, as in the TUI Client

#### Scenario: The user presses in the panel without focus

- **WHEN** the user presses a Library row while the Queue panel has focus
- **THEN** the Library panel takes Panel focus and the Library cursor moves to that row, as a press does in the TUI Client

### Requirement: The window arrangement follows the shared breakpoint classes

The GUI Client SHALL classify the window width into the shared breakpoint classes Mini, Narrow, and Wide, and SHALL give each class the TUI Client's layout behaviour. The width thresholds SHALL come from the canvas sizes. The Queue panel SHALL keep its own four states inside each class.

#### Scenario: The window is Mini

- **WHEN** the window width is below the Narrow threshold
- **THEN** only the panel with Panel focus shows

#### Scenario: The window is Narrow

- **WHEN** the window width is Narrow
- **THEN** both panels show, the Library panel shows its browser pane without a Hero pane, and the Hero opens as the Library Hero overlay on demand, as in the TUI Client

#### Scenario: The window is Wide

- **WHEN** the window width is Wide and the height guard holds
- **THEN** both panels show and the Hero pane sits beside the browser pane

#### Scenario: The window is resized across a threshold

- **GIVEN** the Library cursor is on an item
- **WHEN** the window width crosses a breakpoint threshold
- **THEN** the arrangement changes and the Library cursor, selection, and pill choice do not change

### Requirement: Every destination paints the Library slots

The Library panel SHALL paint the same slots for every destination: a selector row, a list, a Hero, and a Workspace inside the Hero. The Music artboard SHALL set the look of each slot. Each destination SHALL fill the slots with the content its TUI Client owner supplies: Home, Movies and other Emby libraries, Shows, Music, Audiobooks, Podcasts, and Feeds.

#### Scenario: Music paints its slots

- **WHEN** the Music destination is active and an album is selected
- **THEN** the selector row shows the collection pills, the list shows the artist and album tree, the Hero shows the album with square artwork and its actions, and the Workspace shows the tracklist

#### Scenario: A destination has a different artwork shape

- **WHEN** a destination's Hero carries landscape or portrait artwork
- **THEN** the Hero paints the artwork in that shape in the Music Hero's place

#### Scenario: A destination has no Workspace

- **WHEN** the selected item's Hero carries no Workspace
- **THEN** the Hero paints without a Workspace

#### Scenario: Shows has a season selector

- **WHEN** the Shows destination's Hero carries a Workspace with season pills
- **THEN** the Workspace shows the season pills above the episode list

#### Scenario: The list carries no media

- **WHEN** the active list carries rows that are not media, such as Shows Upcoming
- **THEN** the Library panel paints no Hero and the list takes the whole content area

#### Scenario: The list is loading or empty

- **WHEN** the active list is loading or empty
- **THEN** the list paints the loading state or the empty text that the TUI Client shows

### Requirement: Hero actions match the TUI Client

Every action that the Hero shows SHALL run the TUI Client's action for the same item, and SHALL show the default key chord of that action.

#### Scenario: Play next from the Hero

- **WHEN** the user presses the Hero's play-next control
- **THEN** the selected item is admitted after the active slot, as the TUI Client's play-next key does

#### Scenario: A row action appears on hover

- **WHEN** the pointer is over a Workspace row and the row's play-next and add controls appear
- **THEN** the same actions run from the keyboard on the Workspace cursor row

### Requirement: Library interaction state lives in the shared Client core

The Library panel's interaction state (Panel focus, held by the core focus stack; cursor, selection, viewport, tree open state, pill choice, Workspace focus, filter and Search drafts) SHALL live in the Client core that both Clients share. The TUI Client's Library behaviour, key output, and paint SHALL NOT change.

#### Scenario: The window content reloads on a Library item

- **GIVEN** the Library cursor is on an album and the Library has Panel focus
- **WHEN** the window content reloads
- **THEN** the Library has Panel focus and the cursor is on the same album

#### Scenario: The TUI Client browses a library

- **WHEN** the user browses any destination in the TUI Client
- **THEN** every key, pointer gesture, and painted frame behaves as before this change

### Requirement: Library lists keep the cursor visible

Every Library list SHALL keep the cursor row fully visible after every keyboard movement and SHALL use fixed row heights so that capacity is a whole number of rows.

#### Scenario: The cursor moves past the last visible Library row

- **WHEN** the user moves the Library cursor below the last fully visible row
- **THEN** the list scrolls so that the cursor row is fully visible

### Requirement: Collection pills are multi-select in the GUI Client

In the GUI Client a collection pill SHALL toggle on and off. A list SHALL show the de-duplicated union of the selected collections, sorted as one list. At least one pill SHALL stay selected. Latest, filter, and letter-bucket pills SHALL stay single-select. The TUI Client's pills SHALL stay single-select.

#### Scenario: A second collection is selected

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user presses the Soul pill
- **THEN** both pills are selected and the list shows the artists of both collections, each artist once, sorted by sort name

#### Scenario: The last selected pill is pressed

- **GIVEN** only one collection pill is selected
- **WHEN** the user presses that pill
- **THEN** the pill stays selected and the list does not change

#### Scenario: A pill is soloed

- **GIVEN** several collection pills are selected
- **WHEN** the user Alt-presses a pill, or presses Alt+Space on the focused pill
- **THEN** only that pill is selected

#### Scenario: The keyboard toggles a pill

- **WHEN** the user presses Space on the focused collection pill
- **THEN** the pill toggles as a press does

#### Scenario: The selection is cleared

- **GIVEN** two or more collection pills are selected
- **WHEN** the user presses the clear control
- **THEN** only the first selected pill in pill order stays selected

#### Scenario: A non-collection pill is selected

- **GIVEN** several collection pills are selected
- **WHEN** the user selects the Latest pill
- **THEN** only the Latest pill is selected

#### Scenario: Collection pills per destination

- **WHEN** a destination paints its selector row
- **THEN** the collection pills are the Music group pills, the Podcast show pills, the Feeds subscription pills, and the group pills of an Emby library in feed-group mode
- **AND** every other pill, including every Shows, Movies, Home, and Audiobooks pill, is single-select

### Requirement: The pill selection persists per destination

Each destination SHALL keep its own pill selection while the GUI Client runs. Across a restart, the GUI Client SHALL restore the persisted pill scope of the `tui-launch-state` capability; for a Grouped Music library that scope SHALL include every selected group. The TUI Client SHALL restore only the first selected group.

#### Scenario: The user changes destination and returns

- **GIVEN** Hip-Hop and Soul are selected in Music
- **WHEN** the user opens Shows and then Music again
- **THEN** Hip-Hop and Soul are still selected

#### Scenario: The GUI starts again

- **GIVEN** the GUI Client quit with Hip-Hop and Soul selected in Music
- **WHEN** the GUI Client starts again
- **THEN** Music opens with Hip-Hop and Soul selected

#### Scenario: The TUI starts after the GUI

- **GIVEN** the GUI Client quit with Hip-Hop and Soul selected in Music
- **WHEN** the TUI Client starts
- **THEN** Music opens with Hip-Hop selected

#### Scenario: A destination with a Latest pill starts again

- **GIVEN** the GUI Client quit with two Podcast show pills selected
- **WHEN** the GUI Client starts again
- **THEN** Podcasts opens on its Latest pill, as the launch-state rule requires

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, the Status bar under the Library column SHALL show the key hints of the focused Library surface's main actions, each with its configured chord, and the location: the destination, the selected pills, and the selected item. While prefix mode is armed or a toast is active, the prefix state or the toast SHALL replace the key hints.

#### Scenario: Music has an album selected

- **GIVEN** Music is active with Hip-Hop selected and an album selected
- **WHEN** the Library Status bar paints
- **THEN** it shows the filter, play, play-next, add-to-queue, Panel-focus, and tab hints with their configured chords, and the location Music, Hip-Hop, and the album

#### Scenario: A chord is rebound

- **GIVEN** the user rebinds play-next in `[keys]`
- **WHEN** the Library Status bar paints
- **THEN** the play-next hint shows the new chord

#### Scenario: A toast arrives

- **WHEN** the shared Status bar projection holds a toast while the Library panel shows
- **THEN** the toast replaces the key hints until it ends, and the location stays

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show a letter rail. A press on a letter SHALL move the cursor to the first row whose sort name starts with that letter. A letter with no row SHALL do nothing. The rail is a pointer shortcut: its keyboard path SHALL be the list's existing filter. A list sorted by date SHALL show no rail.

#### Scenario: A letter is pressed

- **WHEN** the user presses the letter B on the rail of the Music artist list
- **THEN** the cursor moves to the first artist whose sort name starts with B and the row is fully visible

#### Scenario: A letter without rows is pressed

- **WHEN** the user presses a letter that no row starts with
- **THEN** the cursor and the list do not change

#### Scenario: The list is sorted by date

- **WHEN** the Feeds destination is active
- **THEN** its list shows no letter rail

### Requirement: The top bar shows the tabs and global Search

The Library panel SHALL show a top bar with the destination tabs and a global Search field. The tabs SHALL behave as the TUI Client's tab panel. `Ctrl+K` SHALL open global Search in a GUI-only key context, and the TUI Client's Search chord SHALL also open it. Global Search SHALL behave as the TUI Client's Search sidebar.

#### Scenario: A tab is pressed

- **WHEN** the user presses the Shows tab
- **THEN** the Shows destination becomes active, as the TUI Client's tab panel does

#### Scenario: Search opens from the keyboard

- **WHEN** the user presses `Ctrl+K`
- **THEN** global Search opens with the query field focused

#### Scenario: A Search result is activated

- **WHEN** the user activates a Search result
- **THEN** the GUI Client runs the TUI Client's Search activation for that result

#### Scenario: The TUI keys are unchanged

- **WHEN** the user presses `Ctrl+K` in the TUI Client
- **THEN** the TUI Client behaves as before this change

### Requirement: Library art comes through the frame

The Library panel SHALL paint artwork only from the mbv image cache through the frame. The window content SHALL fetch no image itself.

#### Scenario: An album is selected

- **WHEN** the user selects an album whose artwork is not yet cached
- **THEN** the Hero paints the artwork placeholder until a later frame carries the cached image
- **AND** the window content makes no network request for the artwork
