## ADDED Requirements

### Requirement: The root composes every visible surface from panel components

Every visible cell of a frame SHALL be painted by a mounted component composed by the root: the Tab
panel, the Library panel, the Library playback panel, the Queue panel, the Queue playback panel, the
Status bar panel, the pane boundaries, and the overlay stack. The root SHALL own the frame's split
into panels. There SHALL be no legacy base frame painted beneath the components, no shell-painted
backdrop or chrome, and no hand-ordered list of per-destination render calls in the draw path.

A migration is complete for a surface only when its composition — which rows, panes, fills, borders
and slots exist and where — is owned by a component, not only when its interaction state is.

#### Scenario: A frame is drawn
- **WHEN** any frame is drawn in any Panel mode
- **THEN** every painted cell belongs to exactly one mounted panel component's painted geometry or to
  an overlay
- **AND** no cell is first painted by a shell base frame and then overpainted by a component

#### Scenario: A destination changes
- **WHEN** the selected library tab changes
- **THEN** the same Library panel component paints the frame's library area with the new
  destination's content
- **AND** the draw path does not select a different per-destination render call

### Requirement: Panels own layout and destinations supply only typed slot content

A panel component SHALL own the geometry of everything inside it. A destination component SHALL
contribute to a panel only by producing the typed content the panel's slots define, and SHALL keep its
interaction state (cursor, scroll, focus, drafts) and its embedded media lists as today. A component's
`view()` SHALL NOT construct shell-wide geometry, SHALL NOT call a per-destination free painter that
lays out a pane, and SHALL NOT paint outside the rect its parent gives it. Hit geometry SHALL be
retained by the component that painted it; no shell-wide struct SHALL carry painted rects from paint to
input.

#### Scenario: A destination needs a new visual element
- **WHEN** a destination's content has no slot in its panel's content type
- **THEN** the destination cannot render it
- **AND** adding it requires adding the slot to the panel for every destination

#### Scenario: A click lands on a painted element
- **WHEN** the user clicks a pill, row, tab, control or boundary
- **THEN** the component that painted that element in the latest frame resolves the click from its own
  retained geometry
