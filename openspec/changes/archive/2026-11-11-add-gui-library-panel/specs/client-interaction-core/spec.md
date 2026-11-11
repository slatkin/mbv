## MODIFIED Requirements

### Requirement: Interaction behaviour lives once

For every surface that both Clients present (the shared surfaces: the Queue panel, the Queue playback panel, and the Status bar), interaction rules SHALL exist once, in the Client core: cursor movement, selection, marks, paging, viewport clamping, focus moves, overlay stacking, and chord resolution. For a shared surface, a Client SHALL own only three things: painting, its own geometry, and the translation of raw input into neutral events, and it SHALL NOT reimplement an interaction rule. A surface that one Client presents alone SHALL keep its interaction state and rules in that Client: the GUI Client's Library panel keeps them in its window content.

#### Scenario: A cursor rule changes

- **WHEN** a queue cursor movement rule changes in the Client core
- **THEN** the TUI Client and the GUI Client both show the new behaviour with no change to either Client's code

#### Scenario: The GUI Library panel changes a rule

- **WHEN** a GUI Library panel interaction rule changes in the GUI Client's window content
- **THEN** the TUI Client's Library behaviour does not change

### Requirement: Keys and pointer events are neutral

Each Client SHALL translate the raw key and pointer events that it routes to the Client core into neutral events before routing. The one keyboard router SHALL resolve chords only from neutral key events. Two raw events that denote the same chord SHALL resolve the same way in both Clients. A key that a Client-owned surface takes, such as a GUI Library panel key, does not reach the router.

#### Scenario: The same chord in two Clients

- **WHEN** the user presses the chord bound to remove a queue slot, once in the TUI Client and once in the GUI Client
- **THEN** both resolve to the same command

### Requirement: Pointer targets resolve from a semantic hit map

For the shared surfaces, each Client SHALL report, for each frame, the regions it painted and the semantic target of each region. The Client core SHALL resolve a neutral pointer event against that frame's hit map. A pointer event that lands in no region SHALL resolve to no target.

#### Scenario: A press on a painted row

- **WHEN** a pointer press lands inside the region a Client painted for a queue slot
- **THEN** the core resolves it to that queue slot's stable identity

#### Scenario: A stale hit map

- **WHEN** the queue content changes and a pointer press arrives before the next frame is painted
- **THEN** the press resolves against the regions of the last painted frame and never against coordinates the core computes itself

### Requirement: Clients map their size to shared breakpoint classes

A Client whose Library or hero placement follows the Client core's layout behaviour SHALL map its own size to a shared breakpoint class, and the Client core SHALL decide the layout behaviour of each class. The GUI Client's Library panel does not follow it: the GUI Client arranges its panels from its own measured size and the canvas thresholds, and derives no breakpoint class.

#### Scenario: Two Clients in the Narrow class

- **WHEN** two Clients whose Library placement follows the Client core are both in the Narrow class
- **THEN** both apply the same Narrow layout behaviour

#### Scenario: The GUI Client presents only the canvas panel

- **WHEN** the GUI Client presents only the canvas panel
- **THEN** it picks its panel state from the container size it reports in `LayoutReport`
- **AND** it derives no breakpoint class

#### Scenario: The GUI Client presents the Library panel

- **WHEN** the GUI Client presents the Library panel beside the Queue panel
- **THEN** it picks the arrangement from the Library column width that it measures
- **AND** it derives no breakpoint class
