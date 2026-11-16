## ADDED Requirements

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

While the Search field, the Library filter, or the Queue filter is open for typing, it SHALL take the keys wherever the pointer is. While a core overlay is open or prefix mode is armed, every key SHALL go to the Client core. Esc SHALL close the open Library row menu, the item menu, the Hero overlay, or the Search results wherever the pointer is.

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

### Requirement: One collection pill is selected at a time

One collection pill SHALL be selected at a time: one group is always selected. A press or an Alt-press on a collection pill SHALL select it and deselect the others. A press SHALL NOT deselect the selected pill. No pill SHALL hold a keyboard focus, and no key SHALL select a pill. Latest, filter, and letter-bucket pills SHALL stay single-select. The list SHALL show the selected group's rows.

#### Scenario: Collection pills per destination

- **WHEN** a destination paints its pill row
- **THEN** the collection pills are the Music group pills, the Podcast Latest pill and its show pills, the Feeds Latest pill and its subscription pills, and All and each folder pill of an Emby library in feed-group mode
- **AND** every other pill is single-select

#### Scenario: A second collection is selected

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user presses the Soul pill
- **THEN** only the Soul pill is selected and the list shows the artists of Soul, sorted as one list
- **AND** the list header reads Soul · sort name

#### Scenario: The selected pill is pressed again

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user presses the Hip-Hop pill
- **THEN** Hip-Hop stays selected and the list does not change

#### Scenario: An Alt-press selects the pill

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user Alt-presses the Soul pill
- **THEN** only the Soul pill is selected

#### Scenario: Space over the pill row plays or pauses

- **GIVEN** the pointer rests on the Soul pill
- **WHEN** the user presses Space
- **THEN** playback plays or pauses and the selected pill does not change

#### Scenario: The Podcasts Latest pill merges the shows

- **WHEN** the user presses the Latest pill of Podcasts, whose two shows have episodes
- **THEN** only Latest is selected and the list merges both shows' episodes newest first with undated episodes last
- **AND** no letter rail shows

#### Scenario: Latest is selected at the start

- **WHEN** the Podcasts or the Feeds destination opens for the first time in a launch
- **THEN** its pill row offers Latest and one pill per show or subscription, and Latest is selected

#### Scenario: A podcast show pill scopes the list

- **WHEN** the user presses a show pill of Podcasts
- **THEN** only that pill is selected and the list shows that show's episodes in date order

#### Scenario: The Feeds Latest pill merges the subscriptions

- **WHEN** the user presses the Latest pill of Feeds, whose two subscriptions have entries
- **THEN** only Latest is selected and the list merges both subscriptions' entries newest first with undated entries last
- **AND** no letter rail shows

#### Scenario: A feed subscription pill scopes the list

- **WHEN** the user presses a subscription pill of Feeds
- **THEN** only that pill is selected and the list shows that subscription's entries in date order

#### Scenario: An Emby library in feed-group mode changes pill

- **WHEN** the user presses a folder pill of an Emby library in feed-group mode
- **THEN** only that pill is selected and the list shows that folder's items in date order with no letter rail

#### Scenario: A non-collection pill is selected

- **GIVEN** the Hip-Hop collection is selected
- **WHEN** the user selects the Latest pill
- **THEN** only the Latest pill is selected

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

## MODIFIED Requirements

### Requirement: GUI keyboard interaction matches the TUI Client

For every action that the player panel and the Status bar present, each key chord SHALL have the same effect as in the TUI Client, and these surfaces SHALL use the same `[keys]` configuration and the same keyboard router. The key hints that the canvas shows for them are the default bindings. The canvas chords `z` (collapse), `q` (queue fold), and `c` (cycle subtitle) bind in a GUI-only key context; the TUI Client's default bindings SHALL NOT change. The Library panel's keys are GUI keys that the requirement "The Library keys act on the hovered row" defines; a key that the Library panel does not take SHALL reach the keyboard router.

#### Scenario: A configured binding is used in both Clients

- **GIVEN** the user rebinds a queue action in `[keys]`
- **WHEN** the user presses the new chord in the GUI Client
- **THEN** the GUI Client runs that queue action, as the TUI Client does

#### Scenario: Prefix mode

- **WHEN** the user arms prefix mode in the GUI Client
- **THEN** the footer line shows the armed state and the next chord resolves in the prefix namespace, as in the TUI Client

### Requirement: The GUI presents the Library panel beside the Queue panel

The GUI Client SHALL present the Library panel and the Queue panel in one window. The Library panel's content, arrangement, and states SHALL follow the Library artboard of the design canvas, not the TUI Client. Neither panel SHALL hold a focus: a press in either panel SHALL act at once, and keys SHALL go to the pointer target (the requirement "Keys go to the pointer target").

#### Scenario: Both panels show

- **GIVEN** the window is wide enough for both panels
- **WHEN** the GUI Client window is open
- **THEN** the Library panel and the Queue panel both show
- **AND** neither panel shows as focused

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

The GUI Client SHALL arrange the panels from the measured Library column width (the window width minus the Queue panel's current width), with thresholds from the canvas sizes: Mini below 480 px, Narrow from 480 px, and Wide from 880 px with at least 600 px of height. The Queue panel SHALL keep its own four states inside each arrangement. In Mini only the Queue panel SHALL show, and no key or press SHALL change that.

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

#### Scenario: The window is resized across a threshold

- **GIVEN** a collection pill is selected and the Hero shows an item
- **WHEN** the Library column width crosses a threshold
- **THEN** the arrangement changes and the selected pill and the Hero's item do not change

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

### Requirement: The letter rail jumps a name-sorted list

A Library list sorted by name SHALL show a letter rail from A to Z. A letter with no rows SHALL be disabled. A press on a letter SHALL scroll the list to the first row whose sort name starts with that letter; it SHALL pick and select nothing. The rail's keyboard path SHALL be the list's `/` filter. A list sorted by date SHALL show no rail. An Emby library that is not a feed-group library SHALL show the rail and no pills: the rail replaces the TUI's letter-range pill row, as it does for Shows.

#### Scenario: A plain Emby library paints the rail and no pills

- **WHEN** a Movies library paints its list
- **THEN** the list is flat and sorted by name, it shows the letter rail, and its pill row holds no pill

#### Scenario: A letter is pressed

- **WHEN** the user presses the letter B on the rail of the Music artist list
- **THEN** the list scrolls so that the first artist whose sort name starts with B is fully visible
- **AND** the Hero does not change

#### Scenario: A letter without rows is pressed

- **WHEN** the user presses a disabled letter
- **THEN** the list does not change

### Requirement: The Library Status bar shows hints and the location

While the Library panel shows, a Library Status bar SHALL show under the Library column: the key hints for the filter, play, play next, add to queue, and destination keys on the left, and the location on the right: the destination, the selected collection's name, and the picked item. The hints SHALL name no pane or focus key. While prefix mode is armed or a toast is active, that SHALL replace the key hints, and the location SHALL stay.

#### Scenario: Music has an album picked

- **GIVEN** Music is active with Hip-Hop selected and a Beastie Boys album picked
- **WHEN** the Library Status bar paints
- **THEN** it shows the key hints and the location Music, Hip-Hop, Beastie Boys

#### Scenario: A toast arrives

- **WHEN** the shared Status bar projection holds a toast while the Library panel shows
- **THEN** the toast replaces the key hints until it ends, and the location stays

### Requirement: The Client core holds the player panel's interaction state

The interaction state of the player panel and the Status bar (the Queue's active row, selection, marks, viewport, prefix mode, the collapse and queue-fold toggles, the drag, and the open menu or popover) SHALL live in the Client core, not in the window content. For these surfaces the window content SHALL paint what the Client core projects and SHALL forward key and pointer input to it, each key with its pointer target. The GUI Client SHALL have no Panel focus.

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router, against the key's pointer target

### Requirement: The window content holds the Library panel's interaction state

The Library panel's interaction state (hover, scroll, open tree nodes, filter and Search text, the open Hero overlay and item menu) SHALL live in the window content. The Library panel SHALL hold no cursor and no focus. The Client core SHALL hold the data selection: the active destination, each destination's selected collection, and the Hero's item. The window content SHALL change the data selection only through the Client core.

#### Scenario: The TUI Client browses a library

- **WHEN** the user browses any destination in the TUI Client
- **THEN** every key, pointer gesture, and painted frame behaves as before this change

## REMOVED Requirements

### Requirement: Exactly one focus is visible

**Reason**: It is the TUI focus model. The GUI Client has no Panel focus and no Library cursor, and keys go to the pointer target.

**Migration**: The requirements "Keys go to the pointer target" and "The Queue is a ledger with one active row" replace it. The Queue's active row is the one persistent row state that remains.

### Requirement: The window content takes the window keys

**Reason**: Its Tab and Shift+Tab pane walk moves a Panel focus that the GUI Client no longer has.

**Migration**: "The window content takes the destination and Search keys" keeps the digit and `Ctrl+K` keys. Tab and Shift+Tab go to the keyboard router.

### Requirement: The Library panel handles its own keys

**Reason**: It routes keys by Library Panel focus and acts on a Library cursor and a focused pill, which the GUI Client no longer has.

**Migration**: "The Library keys act on the hovered row" and "Library rows have no cursor and no selection" replace it. The keys act on the hovered row, and the arrow and Page keys scroll.

### Requirement: Collection pills are single-select

**Reason**: It selects a pill with Space on a focused pill, and no pill has a keyboard focus now.

**Migration**: "One collection pill is selected at a time" keeps every press rule and every destination's pill row. Space plays or pauses.

### Requirement: The Library list is the canvas tree

**Reason**: A track press moves a Library cursor, and the list keeps that cursor in view, and the GUI Library has no cursor now.

**Migration**: "The Music list is the canvas tree" keeps the tree, the press, and the double-click rules. A track press picks its album only.
