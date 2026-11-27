## ADDED Requirements

### Requirement: Every GUI shape has one component

Every shape that the GUI paints more than once (a list of data, a Hero, a control, a text field, a popup, a bar, a Settings row, a panel) SHALL have one component that owns its look and its behaviour. Each surface that shows the shape SHALL use that component. A change to a shape's look or behaviour SHALL reach every surface that shows it, and no surface SHALL keep its own copy of the shape.

#### Scenario: A list look change reaches every list

- **WHEN** the look of a list row level changes in the one list component
- **THEN** the change shows in every Library list, the Playlists tree, the Queue, and every Workspace table that paints that level, with no other edit

#### Scenario: A control look change reaches every control

- **WHEN** the look of a button changes in the one control component
- **THEN** the change shows on every button in the window, in the Library, the Queue, the player panel, the menus, the dialogs, and the Settings screen

#### Scenario: Menus and Settings rows are not lists

- **WHEN** a menu entry or a Settings row paints
- **THEN** it paints through its own one component, the menu entry or the Settings row, and not through the list component

### Requirement: Destinations differ only in data

Every Library destination SHALL show its list and its Hero through the same list and Hero components, under the same interaction rules. A destination SHALL differ from another only in the rows, the Hero content, and the meaning of a row act that the Client core gives it. A fix to list or Hero behaviour SHALL reach every destination.

#### Scenario: A tree behaves the same in each destination

- **WHEN** the user presses, double-clicks, expands, collapses, hovers, or right-clicks a row in the Music tree, the Shows All tree, or the Playlists tree
- **THEN** the press, the play, the toggle, the hover controls, and the row menu behave by the same rules in all three trees
- **AND** the Client core decides what each act does for that destination's item

#### Scenario: A Hero behaves the same in each destination

- **WHEN** the Hero shows an item in any destination
- **THEN** its header, art, facts, overview, cast, action row, and list band follow the same rules, and a destination with no list band shows none

### Requirement: One drag gesture serves every drag

The Queue row drag and the scrubber seek drag SHALL behave by the same gesture rules. A drag SHALL start on pointer motion after a press, SHALL follow the pointer anywhere in the window, and SHALL end on the release. A new press while a drag is armed, or the pointer leaving the window, SHALL cancel the drag with no effect.

#### Scenario: A Queue row drag moves the slot

- **GIVEN** the Queue holds slots
- **WHEN** the user presses anywhere on a queue row, moves the pointer, and releases over another position
- **THEN** the drag ghost and the insertion line follow the pointer, and the release moves the slot to the insertion point

#### Scenario: A drag leaves the list

- **WHEN** the user drags a queue row or the scrubber and moves the pointer outside the list or the scrubber, but inside the window
- **THEN** the drag continues to follow the pointer

#### Scenario: The pointer leaves the window during a drag

- **WHEN** the pointer leaves the window while a drag is armed
- **THEN** the drag ends with no move and no seek

## MODIFIED Requirements

### Requirement: Library lists and Workspace tables paint through shared parts

The Library's pills, the Hero's fact pills, letter headings, scrollers, Workspace tables, and Hero action buttons SHALL each have one painter that every destination in scope uses. Every list of data in the GUI (each Library list, the Playlists tree, the Queue, and each Hero Workspace table, the Music artist page included) SHALL paint through the one list component, and no list SHALL paint a row of its own. A Library list row and a Playlists row SHALL have one of three levels, and every row of one level SHALL look the same in every list, with the look of that level in the Music list:

- A top-level row (the Music artist row's look) is every row of a flat list and the top-level row of every tree list (a Music artist, a Shows series, a Playlists playlist). It SHALL paint through the flat row, with its art thumb (`library-row-artwork`) when the item has art.
- A nested parent row (the Music album row's look) is a Music album or a Shows season. It SHALL paint through the tree row.
- A leaf row (the Music track row's look) is a Music track, a Shows episode, or a Playlists item. It SHALL paint through the tree row.

A destination SHALL supply the row's data and the meaning of its acts, and SHALL NOT supply or restate the row's look, its structure, or its handlers. The Queue and each Workspace table SHALL keep their own look through their own values of the list's look tokens, and SHALL share the list's mechanics: scrolling, the visible-row window, selection, hover, the hover controls, the row menu, and the play marks. The list rows and the Workspace table rows SHALL carry one play state, and the played title tone and the resume percent SHALL come from one derivation shared by both; no destination SHALL convert the play state into a second state of its own. A plain Emby library's row SHALL carry the play state its item carries, derived by the one shared Emby derivation the TUI's rows use, in plain mode and in feed-group mode alike. A row whose item carries a play state SHALL show its played or unplayed mark and look, as the requirement "Rows mark their played state" states; a row whose item carries no play state SHALL show neither. A Workspace table SHALL paint a header row, a number cell, a title, a duration, and a progress run. The row that plays at the playhead SHALL show as the playing row over its whole width, with its title emphasised and a playing mark in place of its number. A highlight that is not the playing row SHALL still mark focus only. An Audiobooks chapter row SHALL show its number when it is not the playing row. The Music artist page's album labels SHALL be heading rows of its Workspace table, which no press selects.

#### Scenario: A list row looks the same in every destination

- **WHEN** a Home, Emby, Music artist, Shows series, Shows New, Shows Upcoming, Audiobooks, Podcasts, Feeds, or Playlists playlist row paints
- **THEN** it paints through the flat row with the same height, title, subtitle, trailing text, and selection fill as a Music artist row

#### Scenario: Continue watching shows the time left

- **WHEN** a Home Continue watching row paints
- **THEN** its trailing text is the time left in that item

#### Scenario: A nested row keeps its look

- **WHEN** a Shows season row, a Shows episode row, or a Playlists item row paints
- **THEN** it paints through the tree row, without a thumb
- **AND** a season looks as a Music album row looks, and an episode or a Playlists item looks as a Music track row looks

#### Scenario: The Queue keeps its look on the shared list

- **WHEN** the Queue paints its rows
- **THEN** each row keeps its ordinal, its title, its duration, its progress ring, its now-playing fill, its marked fill, and its divider as before this change
- **AND** its scrolling, hover, hover Play control, and row menu follow the same rules as a Library list's

#### Scenario: A played row mutes its title

- **GIVEN** a list row's item carries a play state
- **WHEN** the item is played
- **THEN** the row shows the played mark and the played look

#### Scenario: An Emby library row carries the shared play state

- **GIVEN** a plain Emby library row whose item is played, partly played with a resumable position, or not played
- **WHEN** the row paints, in plain mode or in feed-group mode
- **THEN** a played item's row shows the played mark and the played look
- **AND** a partly played item's row shows the unplayed mark and the unplayed look
- **AND** a row of an item that is not played shows the unplayed mark
- **AND** a collection row (a folder, a boxset, or a series) carries the same state by the same derivation
- **AND** a music item's row is ordinary, whatever its stored played or resume facts

#### Scenario: A row with no play state shows no mark

- **GIVEN** a Music track row or a Playlists item row, whose item carries no play state
- **WHEN** the row paints
- **THEN** the row shows no played or unplayed mark and keeps its present look

#### Scenario: A Workspace row carries the shared play state

- **GIVEN** a Workspace destination carries a play state on its rows, as the Shows episodes Workspace and the Feeds entries Workspace do
- **WHEN** a Workspace row paints
- **THEN** the row carries the play state the list rows carry, and no second state or percent beside it
- **AND** a played row shows the played mark and look, a partly played row shows the unplayed mark and paints its progress run at its resume percent, and an ordinary row shows the unplayed mark and paints no progress run

#### Scenario: The playing Workspace row keeps its own mark

- **GIVEN** a Workspace row is the one playing at the playhead
- **WHEN** the row paints
- **THEN** the whole row shows as the playing row, its title is emphasised, and the playing mark shows in place of its number
- **AND** the row's carried play state does not change that look, and the row shows no played or unplayed mark

#### Scenario: A playing episode or entry shows the playing mark

- **GIVEN** a Shows episode or Feeds entry in a Hero list is the one playing at the playhead
- **WHEN** its Workspace row paints
- **THEN** the row shows the playing mark in place of its number

#### Scenario: A partly played row with no percent stays ordinary

- **GIVEN** a Workspace row is partly played and carries no resume percent
- **WHEN** the row paints
- **THEN** the row paints no progress run and shows the unplayed mark

#### Scenario: An artist page album label is a heading

- **WHEN** the Hero shows the Music artist page
- **THEN** each album label paints as a heading row of the artist page's Workspace table, and a press on it changes nothing

### Requirement: Scrolling follows the cursor

The queue SHALL keep the cursor row fully visible after every keyboard movement. The queue SHALL scroll as every GUI list scrolls, so a row at the viewport's edge can show in part.

#### Scenario: The cursor moves past the last visible row

- **WHEN** the user moves the queue cursor below the last fully visible row
- **THEN** the queue scrolls so that the cursor row is fully visible

#### Scenario: The wheel leaves a part row at the edge

- **WHEN** the user scrolls the queue with the wheel to a position between two rows
- **THEN** the queue keeps that position, and the row at the edge shows in part

### Requirement: GUI pointer input resolves through semantic targets

A pointer press, drag, or wheel step anywhere in the window SHALL resolve to the semantic target that the GUI painted at that point in the current frame: the component under the pointer receives it. The effect SHALL be the gesture that the design canvas defines. Where the canvas is silent and the TUI has a matching gesture, the effect SHALL be the same. Every surface (the Library panel, the Queue, the player panel, the menus, the dialogs, and the Settings screen) SHALL resolve pointer input in this one way.

#### Scenario: The slice gestures

- **WHEN** the user uses the pointer on the panel
- **THEN** the panel supports row selection, row activation, the row menu, row drag, wheel scrolling, the transport controls, the scrubber, the track chips and their popovers, the playback-target chooser, and the collapse and fold controls

#### Scenario: A queue row is clicked

- **WHEN** the user clicks a painted queue row
- **THEN** the queue cursor moves to that slot, as a click does in the TUI Client

#### Scenario: The wheel moves over the queue

- **WHEN** the user scrolls the wheel over the queue
- **THEN** the queue viewport moves as a Library list's viewport moves under the wheel, and the cursor follows the TUI wheel rules against the rows in view

#### Scenario: A control is clicked in a surface without focus

- **GIVEN** the queue has focus
- **WHEN** the user clicks the play/pause control
- **THEN** playback toggles

#### Scenario: A press lands on no target

- **WHEN** the user clicks a point with no painted target
- **THEN** nothing changes

#### Scenario: A menu covers a row

- **GIVEN** an open menu paints over a queue row or a Library row
- **WHEN** the user presses on a menu entry
- **THEN** the entry receives the press and the row beneath it does not

### Requirement: The Client core holds the player panel's interaction state

The interaction state of the player panel and the Status bar (the Queue's active row, selection, marks, prefix mode, the collapse and queue-fold toggles, and the open menu or popover) SHALL live in the Client core, not in the window content. For these surfaces the window content SHALL paint what the Client core projects and SHALL send the Client core each act the user takes on a painted target (a press, an activation, a menu request, a drag's drop, a seek) and each key with its pointer target. The Queue's viewport, hover, and drag gesture are window content state (the requirement "The window content holds the Library panel's interaction state"). The GUI Client SHALL have no Panel focus.

#### Scenario: A key arrives in the window content

- **GIVEN** the Library panel does not take the key
- **WHEN** the user presses a key while the GUI window has focus
- **THEN** the Client core resolves the chord through the one keyboard router, against the key's pointer target

#### Scenario: A queue drag drops

- **WHEN** the user drops a dragged queue row
- **THEN** the window content sends the Client core one move act that names the dragged slot and the insertion point
- **AND** the Client core sends the move request and keeps the queue's order authority

### Requirement: The window content holds the Library panel's interaction state

The Library panel's interaction state (hover, scroll, open tree nodes, filter and Search text, the open Hero overlay and item menu) SHALL live in the window content. The Queue's viewport, hover, and drag gesture SHALL also live in the window content, which reports the Queue's rows in view to the Client core. The Library panel SHALL hold no cursor and no focus. The Client core SHALL hold the data selection: the active destination, each destination's selected collection, and the Hero's item. The window content SHALL change the data selection only through the Client core.

#### Scenario: The TUI Client browses a library

- **WHEN** the user browses any destination in the TUI Client
- **THEN** every key, pointer gesture, and painted frame behaves as before this change

#### Scenario: The TUI Client queue is unchanged

- **WHEN** the user scrolls, clicks, or drags in the TUI Client's queue
- **THEN** the TUI queue behaves as before this change
