# Spec Delta

## MODIFIED Requirements

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
