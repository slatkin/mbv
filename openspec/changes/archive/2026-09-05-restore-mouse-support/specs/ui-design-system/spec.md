## MODIFIED Requirements

### Requirement: Screens use canonical UI ownership boundaries

The UI SHALL separate screen content, arrangement geometry, and component painting.
Screens SHALL provide semantic content and approved variants; arrangements SHALL own
shared geometry; components SHALL own their painting and styling. Screen modules
SHALL NOT call Ratatui or construct layout rectangles.

For every mouse path, hit-target ownership belongs to the interactive component
that paints the region, as defined by the `interactive-component-framework` and
`mouse-input` capabilities. Screens SHALL NOT compute or own hit geometry, and no
screen-local or arrangement-local hit map is introduced. The former global
completed-frame mouse hit map and router are removed by those capabilities;
render-only layout state MAY remain, but this capability no longer treats it as hit
resolution authority.

Classification is by signature: a function taking app state and returning a typed
content model is screen code; a function taking a typed content model, a `Rect`, and
a buffer is a component; a function placing components within a `Rect` and owning
breakpoints is an arrangement.

#### Scenario: A screen adds content without duplicating a painter
- **WHEN** a screen needs different titles, metadata, rows, or images
- **THEN** it supplies a screen model to an existing arrangement or component
- **AND** it does not copy the arrangement's geometry or painter

#### Scenario: Existing hit-target resolution is preserved
- **WHEN** a mouse event is resolved for any surface
- **THEN** the click resolves to a target computed by the interactive component
  that painted the region, from the same geometry it painted with
- **AND** it is not resolved by a global completed-frame hit map or by any
  screen-local or arrangement-local hit map

#### Scenario: Deferred mouse support is restored later
- **WHEN** mouse interaction is restored for a surface that had it deferred
- **THEN** its interactive component computes targets from the geometry it painted
- **AND** the implementation does not restore a global mouse router, global hit map,
  or duplicated coordinate path

#### Scenario: A surface is migrated
- **WHEN** a surface listed in the change ledger is brought inside the boundary
- **THEN** a characterization buffer test capturing its current default, focused,
  narrow-width, and selected output lands first, in its own commit
- **AND** the migration commit leaves that test unchanged and passing
- **AND** the ledger row is ticked in the same PR

#### Scenario: A hero uses an approved additional-content style
- **WHEN** a hero supplies the Movie overview/detail block, the TV season/pill and
  episode workspace, the Music track-list workspace, or another centrally mapped
  provider-specific style, including its preview versus focusable child state
- **THEN** the screen supplies the typed content and interaction state to the shared
  arrangement or component
- **AND** the arrangement owns pane placement, sizing, spacing, and responsive layout
- **AND** the screen does not invent another additional-content style or supply
  screen-local rectangles, row arithmetic, breakpoints, or renderer callbacks
- **AND** any approved customisation is implemented by the central owning component
  or arrangement, not by the surface

#### Scenario: An Inline hero displays an image
- **WHEN** an Inline hero has an image
- **THEN** the shared hero component places it against the top and right edges
- **AND** text reserves a one-column gutter to its left and a one-row gutter below it
- **AND** the surface may supply image dimensions but not placement or gutter geometry
