## ADDED Requirements

### Requirement: Interactive surfaces have one owner and one painter

Every interactive surface SHALL have exactly one component owning its
interaction state, and exactly one painter, at every reachable breakpoint.

#### Scenario: An interactive surface has one cursor owner at every breakpoint

- **WHEN** a surface is reachable at a layout breakpoint
- **THEN** exactly one mounted component owns its cursor, scroll, and keyboard
  handling at that breakpoint
- **AND** a breakpoint gate on a mount decision leaves no reachable width at
  which the surface has no owner

#### Scenario: A painted surface has exactly one painter

- **WHEN** a component and a legacy renderer are both capable of painting the
  same rect
- **THEN** exactly one of them runs for that rect at that breakpoint
- **AND** the surface ledger records the owner and the painter, per breakpoint

#### Scenario: A mounted component is placed in a non-empty rect

- **WHEN** a component is mounted and owns a surface at a breakpoint
- **THEN** the shell places its view in the rect that surface actually occupies
  at that breakpoint
- **AND** a placement rect that is empty at some breakpoint is a violation, not
  a way to disable a component

#### Scenario: Components issue no effects while painting

- **WHEN** a surface's composition moves into its owning component
- **THEN** no image fetch, content fetch, or other effect executes inside the
  component
- **AND** effects the legacy painter performed inline are relocated to the
  shell, driven by the component's projected state
