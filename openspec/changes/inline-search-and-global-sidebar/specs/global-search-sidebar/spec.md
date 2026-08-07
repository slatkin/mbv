## Purpose

Gives server-wide Emby search a persistent side panel reachable from any tab, so the user can look up an item anywhere on the server without losing sight of the view they were in.

## ADDED Requirements

### Requirement: A dedicated key opens the global search sidebar from any tab

A single chord distinct from the inline library search key SHALL open the global search sidebar. It SHALL work from the home tab and from every library tab, and SHALL be recognised whether the terminal reports the chord as the search character with the control modifier or as the control code that terminals substitute for it.

Pressing the chord while the sidebar is already open SHALL leave it open and SHALL NOT insert a character into the query.

#### Scenario: Opening from a library tab

- **WHEN** the user presses the global search chord on a library tab
- **THEN** the global search sidebar SHALL open with an empty query
- **AND** the library list SHALL remain visible and unfiltered

#### Scenario: Opening from the home tab

- **WHEN** the user presses the global search chord on the home tab
- **THEN** the global search sidebar SHALL open

#### Scenario: Chord pressed while already open

- **WHEN** the user presses the global search chord while the sidebar is open
- **THEN** the sidebar SHALL stay open and the query SHALL be unchanged

### Requirement: The sidebar occupies the panel slot

The sidebar SHALL be drawn in the same slot and with the same panel frame, title row, and hint footer as the other side panels, so it is visually consistent with them. It SHALL NOT dim the view behind it and SHALL NOT be drawn as a centered modal.

When the layout has collapsed the panel slot to zero width, the sidebar SHALL still be reachable and SHALL render at a fixed width against the left edge of the window, matching how the other side panels behave in that layout.

The sidebar SHALL contain, in order from the top: a text input, a row of type-filter chips, and the result list.

#### Scenario: Sidebar drawn with the panel slot available

- **WHEN** the sidebar is open and the panel slot has non-zero width
- **THEN** the sidebar SHALL fill that slot with the standard panel frame, title, and hint footer

#### Scenario: Sidebar drawn with the panel slot collapsed

- **WHEN** the sidebar is open and the panel slot has zero width
- **THEN** the sidebar SHALL render at a fixed width against the left edge of the window

#### Scenario: No backdrop dimming

- **WHEN** the sidebar is open
- **THEN** the rest of the interface SHALL render at normal brightness

### Requirement: Queries are debounced and dispatched once they reach two characters

A query shorter than two characters SHALL NOT be sent to the server. Once the query reaches two characters, each change to it SHALL schedule a server query that fires only after the user stops typing for a short interval, so a burst of keystrokes produces one request rather than one per key.

Dispatch SHALL NOT block the interface. The sidebar SHALL show a loading state from the moment a query is scheduled until its results arrive.

A response SHALL be applied only if it answers the query currently in the input; a response for a superseded query SHALL be discarded without altering the results or the loading state.

#### Scenario: Query below the dispatch threshold

- **WHEN** the query holds fewer than two characters
- **THEN** no request SHALL be sent to the server

#### Scenario: Fast typing

- **WHEN** the user types several characters in quick succession
- **THEN** one request SHALL be sent after typing pauses, not one per keystroke

#### Scenario: Out-of-order response

- **WHEN** a response for an earlier query arrives after a response for a later one
- **THEN** the earlier response SHALL be discarded
- **AND** the displayed results SHALL continue to reflect the current query

#### Scenario: Request fails

- **WHEN** a server query returns an error
- **THEN** the loading state SHALL clear
- **AND** the error SHALL be surfaced to the user rather than rendered as an empty result set

### Requirement: Results render as plain single-row items

Each result SHALL occupy exactly one row. The sidebar SHALL NOT render a hero block, a second metadata row, an overview, or any image for any result, selected or otherwise, and SHALL NOT initiate an image fetch for the items it displays.

Each row SHALL carry a badge naming the result's item type, and SHALL be truncated to the sidebar width rather than wrapping.

The query SHALL be matched against item names only; badge text SHALL NOT contribute to the match.

#### Scenario: A result is selected

- **WHEN** the user selects a result
- **THEN** the row SHALL be highlighted in place
- **AND** no hero block, extra row, or image SHALL be drawn and no image fetch SHALL start

#### Scenario: Mixed-type results

- **WHEN** the server returns results of several item types
- **THEN** each row SHALL carry a badge naming its type

#### Scenario: Long result name

- **WHEN** a result's name is wider than the sidebar
- **THEN** the row SHALL be truncated to one row rather than wrapped

### Requirement: Results can be narrowed to a single item type

The type-filter chips SHALL offer an unfiltered choice plus one chip per item type present in the current result set. The forward and backward cycle keys SHALL move between chips, wrapping at both ends. Selecting a chip SHALL restrict the visible results to that type and reset the selection to the first visible result.

Arrival of a new result set SHALL clear the active chip back to unfiltered and reset the selection to the first result.

#### Scenario: Narrowing to one type

- **WHEN** the user cycles the chips to a type chip
- **THEN** only results of that type SHALL be displayed
- **AND** the selection SHALL sit on the first of them

#### Scenario: Chips reflect the current results

- **WHEN** a result set arrives
- **THEN** the chips SHALL offer exactly the item types present in that result set, plus the unfiltered choice

#### Scenario: New results reset the filter

- **WHEN** a new result set arrives while a type chip is active
- **THEN** the chip SHALL return to unfiltered and the selection SHALL return to the first result

### Requirement: Activating a result navigates to it and closes the sidebar

Activating a result SHALL switch to the library tab containing that item and place the selection on it, using the same navigation path already used to reveal an item from elsewhere in the application, and SHALL then close the sidebar.

Results whose item type cannot be resolved to a library SHALL be excluded from the result list, so every displayed result can be activated.

When no result is selected, the activation key SHALL do nothing and the sidebar SHALL stay open with the query intact.

#### Scenario: Activating a result in another library

- **WHEN** the user activates a result belonging to a library other than the current tab
- **THEN** the application SHALL switch to that item's library tab and select the item
- **AND** the sidebar SHALL close

#### Scenario: Activation with nothing selected

- **WHEN** the user presses the activation key while results are empty or still loading
- **THEN** nothing SHALL happen and the sidebar SHALL stay open with its query intact

#### Scenario: Unnavigable types excluded

- **WHEN** the server returns results whose item type cannot be resolved to a library
- **THEN** those results SHALL NOT be displayed

#### Scenario: Every result excluded

- **WHEN** every returned result is of an unnavigable type
- **THEN** the sidebar SHALL show its empty state

### Requirement: The sidebar holds keyboard input while open

While the sidebar is open it SHALL receive every key press before the view beneath it. The library list, queue list, and home view SHALL remain visible but SHALL NOT respond to navigation, activation, or shortcut keys.

Keys the sidebar does not bind SHALL be swallowed rather than passed through to the view beneath. No key handled by the sidebar SHALL quit the application.

Selection and navigation state in the view beneath SHALL be unchanged by anything typed into the sidebar.

#### Scenario: Navigation keys while open

- **WHEN** the user presses Up or Down with the sidebar open
- **THEN** the sidebar's result selection SHALL move
- **AND** the library list selection beneath SHALL be unchanged

#### Scenario: An unbound shortcut is pressed

- **WHEN** the user presses a key the sidebar does not bind
- **THEN** it SHALL be swallowed and SHALL NOT trigger the action it would trigger in the view beneath

### Requirement: Dismissing the sidebar leaves the underlying view untouched

The dismiss key SHALL close the sidebar without navigating anywhere, returning input to whatever had it before the sidebar opened. Deleting the last character of an empty query SHALL dismiss the sidebar the same way.

Dismissal SHALL discard the query, the results, and the type filter; reopening SHALL start from an empty query.

#### Scenario: Dismissing without activating

- **WHEN** the user dismisses the sidebar without activating a result
- **THEN** the sidebar SHALL close
- **AND** the underlying view's tab, navigation position, and selection SHALL be exactly as they were before it opened

#### Scenario: Reopening after dismissal

- **WHEN** the user dismisses the sidebar and reopens it
- **THEN** the query SHALL be empty and no results SHALL be shown
