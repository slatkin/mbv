## MODIFIED Requirements

### Requirement: Screens do not determine their own arrangement

The responsive breakpoint SHALL be evaluated in one place, and its value SHALL be defined in one
place. An individual screen SHALL NOT test the available width to select an arrangement, a column
count, or any presentation that differs between arrangements. The arrangement SHALL own pane
placement, breakpoints, and rectangle splitting; components SHALL own painting; and screens SHALL
provide content and interaction state. Code outside a screen SHALL NOT paint part of that screen's
arrangement.

The single place that evaluates the breakpoint SHALL derive it from the current mount area and
terminal size, never from what was painted on a previous frame and never from a post-paint
readback of a mounted component's own geometry. A decision that branches on the breakpoint SHALL
be correct on the same tick a resize is delivered, including the very first tick after the
resize: it SHALL NOT act on a breakpoint value computed from the terminal size that was current
before that resize.

#### Scenario: The breakpoint value is changed

- **WHEN** the breakpoint value is changed in its single definition
- **THEN** every right-panel screen changes arrangement at the new width
- **AND** no screen requires an individual edit

#### Scenario: One arrangement's presentation is changed

- **WHEN** the presentation of one arrangement is changed
- **THEN** the other arrangement is unaffected

#### Scenario: A keyboard or activation decision runs on the resize tick

- **WHEN** a terminal resize is delivered and a keyboard or activation decision that branches on
  the breakpoint runs during that same tick, before any frame has been painted at the new size
- **THEN** the decision uses the breakpoint that matches the new terminal size
- **AND** it never uses a breakpoint left over from the previous frame's paint

#### Scenario: No decision reads a previous-frame paint signal

- **WHEN** any consumer needs to know whether the right panel is currently in the wide or narrow
  presentation
- **THEN** it reads the one paint-free breakpoint predicate derived from the current mount area
  and terminal size
- **AND** no consumer derives that decision from a component's previous-frame painted rect, a
  previous-frame painted area's nonzero width or height, or a post-paint mirror of a mounted
  component's own reported geometry

#### Scenario: A narrow-only or wide-only geometry field remains paint-produced

- **WHEN** a consumer needs the actual painted geometry of a specific pane (for example, to
  resolve a context-menu anchor or a hit target) after a frame has already painted at the current
  breakpoint
- **THEN** it MAY continue to read that pane's painted rect
- **AND** only the choice of which breakpoint branch to take is required to be paint-free, not
  every rect consumed once inside that branch
