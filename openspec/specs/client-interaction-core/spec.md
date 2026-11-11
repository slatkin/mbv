# client-interaction-core Specification

## Purpose

Defines the front-end-neutral Client core that the TUI Client and the GUI Client share, so that interaction behaviour lives once and the two Clients cannot drift apart.

## Requirements

### Requirement: Interaction behaviour lives once

For every surface that both Clients present (the shared surfaces: the Queue panel, the Queue playback panel, and the Status bar), interaction rules SHALL exist once, in the Client core: cursor movement, selection, marks, paging, viewport clamping, focus moves, overlay stacking, and chord resolution. For a shared surface, a Client SHALL own only three things: painting, its own geometry, and the translation of raw input into neutral events, and it SHALL NOT reimplement an interaction rule. A surface that one Client presents alone SHALL keep its interaction state and rules in that Client: the GUI Client's Library panel keeps them in its window content.

#### Scenario: A cursor rule changes

- **WHEN** a queue cursor movement rule changes in the Client core
- **THEN** the TUI Client and the GUI Client both show the new behaviour with no change to either Client's code

#### Scenario: The GUI Library panel changes a rule

- **WHEN** a GUI Library panel interaction rule changes in the GUI Client's window content
- **THEN** the TUI Client's Library behaviour does not change

### Requirement: Surface state is independent of any Client

The interaction state of each shared surface (cursor, selection, marks, viewport, local focus, drafts) SHALL be held as Surface state. Surface state SHALL NOT depend on a terminal library, a terminal UI framework, or a GUI toolkit. Its viewport SHALL be counted in items, not in cells or pixels.

#### Scenario: Surface state is built without a Client library

- **WHEN** the Surface state crate is built alone
- **THEN** its dependency graph contains no terminal library, terminal UI framework, or GUI toolkit

### Requirement: The core focus stack is the one focus authority

The Client core SHALL own which surfaces are mounted, which surface has focus, and the overlay stack. Each Client SHALL present focus from this stack. A Client's own framework SHALL follow the stack and SHALL NOT decide focus by itself.

#### Scenario: An overlay closes

- **WHEN** a blocking overlay is dismissed in either Client
- **THEN** the core focus stack returns focus to the surface that held it before the overlay opened
- **AND** the Client presents that surface as focused on the next frame

#### Scenario: Panel focus moves

- **WHEN** the user moves Panel focus from Library to Queue in the TUI Client
- **THEN** the core focus stack records Queue as focused
- **AND** the TUI framework's focused component follows that record

### Requirement: Keys and pointer events are neutral

Each Client SHALL translate the raw key and pointer events that it routes to the Client core into neutral events before routing. The one keyboard router SHALL resolve chords only from neutral key events. Two raw events that denote the same chord SHALL resolve the same way in both Clients. A key that a Client-owned surface takes, such as a GUI Library panel key, does not reach the router.

#### Scenario: The same chord in two Clients

- **WHEN** the user presses the chord bound to remove a queue slot, once in the TUI Client and once in the GUI Client
- **THEN** both resolve to the same command

### Requirement: Clients report capacity in items

Each Client SHALL report to the Client core how many whole items of each shared list are visible in the current frame. The core SHALL use that capacity as the one source of paging stride and viewport clamping.

#### Scenario: A page-down in the GUI Client

- **GIVEN** the GUI Client reports that 14 queue rows are fully visible
- **WHEN** the user pages down
- **THEN** the queue cursor moves by the stride that the core derives from 14 visible rows

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
