## MODIFIED Requirements

### Requirement: The right panel has exactly two arrangements

The right panel SHALL assign each screen one of two wide arrangements. Hero-on-top places the hero
above the list. Hero-on-left places the hero beside the list, with the list in a single column.
Below the shared breakpoint, every library browse screen SHALL use the standard narrow one-column
presentation: the selected item's hero SHALL be rendered inline in the scrolling list at its active
row. Narrow library presentation SHALL NOT pin the hero in a separate area above the list, and SHALL
NOT be a per-library arrangement exception. Non-library screens retain their existing narrow
presentation.

#### Scenario: A library enters the narrow presentation

- **WHEN** a library browse screen's available width falls below the shared breakpoint
- **THEN** it renders one list column
- **AND** the selected item's hero renders inline in the list at the active row
- **AND** the hero does not reserve a separate area above the list

#### Scenario: A wide hero-on-top screen crosses the breakpoint

- **WHEN** a hero-on-top library screen's available width crosses below the breakpoint
- **THEN** its wide arrangement assignment remains hero-on-top
- **AND** its narrow presentation is the shared inline-hero one-column presentation

#### Scenario: A hero-on-top screen crosses the breakpoint

- **WHEN** a hero-on-top screen's available width crosses the breakpoint
- **THEN** its wide arrangement does not change
- **AND** its narrow library presentation uses one inline-hero column when it is a library browse
  screen

#### Scenario: A wide hero-on-left screen falls below the breakpoint

- **WHEN** a hero-on-left library screen's available width falls below the breakpoint
- **THEN** its wide arrangement assignment remains hero-on-left
- **AND** it renders the shared inline-hero one-column presentation

#### Scenario: A hero-on-left screen falls below the breakpoint

- **WHEN** a hero-on-left screen's available width falls below the breakpoint
- **THEN** a library browse screen renders the shared inline-hero presentation with a single list
  column

#### Scenario: Panel mode changes

- **WHEN** the user cycles Panel mode
- **THEN** the presentation is recomputed from the width the right panel is left with
- **AND** the selected wide arrangement or standard narrow presentation is otherwise unaffected
