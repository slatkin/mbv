## MODIFIED Requirements

### Requirement: The panel changes state with its own size

The panel SHALL pick one of four states from its own size, not from the window size: expanded, compact strip, expanded without queue, or rail. The `z` key SHALL toggle the collapse and the `q` key SHALL toggle the queue fold. The Client core SHALL hold the two toggles. The collapse SHALL apply only while the Library panel shows beside the panel or the window is pinned: while the Queue column is the only column of a window that is not pinned (the Mini view's Queue, or no Library content), the panel SHALL paint as if not collapsed and the footer line SHALL show no collapse control. In a pinned window the collapse SHALL follow `gui-pin-window` "A pinned Queue view collapses to the rail". The front end SHALL pick the layout from the measured size of the panel container, the same measurement that it reports to the Client core in `LayoutReport`, so the painted layout, the list capacity, and the hit map follow one size.

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

#### Scenario: A queue-only window does not collapse

- **GIVEN** the window is not pinned and the Queue column is the only column in the window
- **WHEN** the user opens the panel, or presses `z`
- **THEN** the footer line shows no collapse control and the panel never shows the rail beside an empty window

#### Scenario: A pinned Queue view collapses

- **GIVEN** the window is pinned and shows the Queue view
- **WHEN** the user opens the panel
- **THEN** the footer line shows the collapse control
- **AND** a press on it shrinks the window to the rail

### Requirement: The top bar shows the destination tabs and Search

The window SHALL show a top bar with a Home icon and the destination tabs as its global header. Once Search is presented, the Library Status bar SHALL show a Search field with the hint `Ctrl K` at its left side. A press on a tab SHALL make its destination active. In Mini the destination tabs SHALL show as one dropdown in the tab strip's place: its trigger SHALL name the current tab (or `Libraries` while no tab is current), its rows SHALL be the tabs in their order, and a row press SHALL act as a press on that tab and close the dropdown. The Home and Playlists icons SHALL stay beside it. Esc and a press outside SHALL close the dropdown. The Search field SHALL search all libraries, show the results above the field, and activate a result by Enter or a press. In a pinned window, a destination press SHALL move between the Queue view and the Full view by `gui-pin-window` "The header moves between the views", and SHALL NOT show the Library in place of the Queue.

#### Scenario: A tab is pressed

- **WHEN** the user presses the Music tab
- **THEN** Music becomes the active destination and its tab is marked current

#### Scenario: The tabs are a dropdown in Mini

- **GIVEN** the arrangement is Mini and Shows is the active destination
- **WHEN** the window paints the top bar
- **THEN** the Home and Playlists icons show, then one dropdown named Shows, and no tab strip
- **AND** opening the dropdown lists the destination tabs in order with Shows marked

#### Scenario: A tab is pressed in Mini while the Queue shows

- **GIVEN** the window is not pinned, the arrangement is Mini, and the Queue shows
- **WHEN** the user presses a destination tab
- **THEN** that destination becomes active and the Library shows in place of the Queue

#### Scenario: The active destination's tab is pressed again in Mini while the Library shows

- **GIVEN** the window is not pinned, the arrangement is Mini, and the Library shows for an active destination
- **WHEN** the user presses that destination's tab again, or the Home icon while the Home content is present
- **THEN** the Library's press flips the view back to the Queue

#### Scenario: A different destination's tab is pressed in Mini while the Library shows

- **GIVEN** the window is not pinned, the arrangement is Mini, and the Library shows
- **WHEN** the user presses a destination tab whose destination is not active
- **THEN** that destination becomes active and the Library stays showing

#### Scenario: A tab is pressed in a pinned Queue view

- **GIVEN** the window is pinned and shows the Queue view
- **WHEN** the user presses a destination tab
- **THEN** that destination becomes active and the window opens the Full view
- **AND** the Library does not show in the Queue column's place

#### Scenario: Search opens from the keyboard

- **GIVEN** the Library panel shows
- **WHEN** the user presses `Ctrl+K`
- **THEN** the Search field in the Library Status bar has focus

#### Scenario: A Search result is activated

- **WHEN** the user activates a Search result
- **THEN** the GUI Client opens the result's destination on that item
